//! Key dispatch — which stored key a launch actually uses.
//!
//! The launcher can only choose a credential at **one** moment: the instant it
//! builds the child's environment in `executor::start`. It never sees a request or
//! a response, so "429 → retry with the next key" is not something it can do
//! without becoming a proxy in front of the provider, which it is not.
//!
//! What it can do, and does:
//!
//! * **Binding** — an instance names a key (`api_key_ref = "deepseek/team"`) and
//!   gets that one, every launch.
//! * **Round-robin** — an instance names only a provider (`"deepseek"`), or nothing
//!   at all, and successive launches walk that provider's enabled keys in turn.
//!
//! The two failure modes are deliberately different. An **explicit** binding that
//! cannot be honoured fails the launch loudly: the user asked for a specific key,
//! and quietly running on a different one is a billing and quota decision that is
//! not ours to make. The **implicit** fallback — matching `instance.provider`
//! against a provider id, for the common case where the two happen to coincide —
//! never fails a launch: it injects nothing and the engine falls back to whatever
//! it already had, which is exactly how every instance behaved before this file
//! existed.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use super::{load, ProviderDef, ProviderKey};
use crate::instance_manager::Instance;

/// Split `api_key_ref` into `(provider, alias)`. `"deepseek/team"` binds one key,
/// `"deepseek"` names a provider to round-robin, `""` means "figure it out".
fn parse_ref(raw: &str) -> (String, String) {
    let raw = raw.trim();
    match raw.split_once('/') {
        Some((p, a)) => (p.trim().to_string(), a.trim().to_string()),
        None => (raw.to_string(), String::new()),
    }
}

/// Round-robin position per provider, in memory only.
///
/// Not persisted, on purpose. The obvious home would be `providers.json`, but that
/// would rewrite a file full of secrets on every single launch — a much worse trade
/// than restarting the rotation after an app restart. `config.json` is not an option
/// either: the frontend owns it and rewrites it whole, so a backend field there
/// would be clobbered by the next autosave.
fn next_index(provider_id: &str, len: usize) -> usize {
    static CURSOR: OnceLock<Mutex<HashMap<String, usize>>> = OnceLock::new();
    let cursor = CURSOR.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = cursor.lock().unwrap();
    let slot = guard.entry(provider_id.to_string()).or_insert(0);
    // Modulo on read, so a key list that shrank between launches cannot index past
    // its end.
    let i = *slot % len;
    *slot = i + 1;
    i
}

fn usable(p: &ProviderDef) -> Vec<&ProviderKey> {
    p.keys
        .iter()
        .filter(|k| k.enabled && !k.value.is_empty())
        .collect()
}

/// The env pairs to inject for this launch, or an empty vec when the launcher has
/// nothing to say about this instance's credentials.
///
/// Layering matters at the call site: these go on **before** the instance `.env`,
/// so a key written by hand into `instances/<id>/.env` still wins as the most
/// specific source — the same rule PATH already follows.
pub fn env_for_instance(inst: &Instance) -> Result<Vec<(String, String)>, String> {
    let (mut pid, alias) = parse_ref(&inst.api_key_ref);
    // Explicit means the user typed a reference; implicit means we are guessing from
    // the engine-facing provider string, and a guess may never break a launch.
    let explicit = !pid.is_empty();
    if !explicit {
        pid = inst.provider.trim().to_string();
    }
    if pid.is_empty() {
        return Ok(Vec::new());
    }

    let doc = load()?;
    let Some(p) = doc.providers.iter().find(|p| p.id == pid) else {
        return if explicit {
            Err(format!("实例绑定的 provider「{pid}」不存在"))
        } else {
            Ok(Vec::new())
        };
    };
    if !p.enabled {
        return if explicit {
            Err(format!("实例绑定的 provider「{pid}」已停用"))
        } else {
            Ok(Vec::new())
        };
    }
    if p.api_key_env.is_empty() {
        return if explicit {
            Err(format!("provider「{pid}」没有设置 API Key 环境变量名"))
        } else {
            Ok(Vec::new())
        };
    }

    let chosen: &ProviderKey = if alias.is_empty() {
        let pool = usable(p);
        if pool.is_empty() {
            return if explicit {
                Err(format!("provider「{pid}」下没有可用的 API Key"))
            } else {
                Ok(Vec::new())
            };
        }
        pool[next_index(&pid, pool.len())]
    } else {
        let k = p
            .keys
            .iter()
            .find(|k| k.alias == alias)
            .ok_or_else(|| format!("provider「{pid}」下没有名为「{alias}」的 API Key"))?;
        if !k.enabled {
            return Err(format!("API Key「{alias}」已停用"));
        }
        if k.value.is_empty() {
            return Err(format!("API Key「{alias}」还没有填入值"));
        }
        k
    };

    let mut env = vec![(p.api_key_env.clone(), chosen.value.clone())];
    // Only when the user named a variable for it: injecting a base URL nobody asked
    // for is how a working instance starts pointing somewhere else.
    if !p.base_url_env.is_empty() && !p.base_url.is_empty() {
        env.push((p.base_url_env.clone(), p.base_url.clone()));
    }
    Ok(env)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::{get_providers, set_key, set_providers};
    use crate::test_support::{temp_tree, EnvGuard, HOME_LOCK};

    fn inst(provider: &str, key_ref: &str) -> Instance {
        Instance {
            schema_version: 1,
            id: "i1".into(),
            name: "i1".into(),
            icon: String::new(),
            group: String::new(),
            description: String::new(),
            profile: "default".into(),
            provider: provider.into(),
            model: String::new(),
            api_key_ref: key_ref.into(),
            default_task: String::new(),
            runtime: Default::default(),
            created_at: String::new(),
        }
    }

    #[test]
    fn parsing_a_reference() {
        assert_eq!(parse_ref(""), (String::new(), String::new()));
        assert_eq!(parse_ref(" deepseek "), ("deepseek".into(), String::new()));
        assert_eq!(parse_ref("deepseek/team"), ("deepseek".into(), "team".into()));
    }

    #[test]
    fn an_unpinned_instance_walks_the_enabled_keys_in_turn() {
        let _g = HOME_LOCK.lock().unwrap();
        let tree = temp_tree("dispatch-rr");
        let _home = EnvGuard::set("HOME", tree.path());

        set_key("deepseek", "a", "sk-aaaaaaaaaaaaaaaa").unwrap();
        set_key("deepseek", "b", "sk-bbbbbbbbbbbbbbbb").unwrap();
        set_key("deepseek", "c", "sk-cccccccccccccccc").unwrap();
        // A disabled key is skipped, not consumed.
        let mut view = get_providers().unwrap();
        {
            let p = view.iter_mut().find(|p| p.id == "deepseek").unwrap();
            p.keys.iter_mut().find(|k| k.alias == "b").unwrap().enabled = false;
        }
        set_providers(view).unwrap();

        let i = inst("", "deepseek");
        let seen: Vec<String> = (0..4)
            .map(|_| env_for_instance(&i).unwrap()[0].1.clone())
            .collect();
        // Two enabled keys, alternating, with `b` never appearing. *Where* the cycle
        // starts is not asserted: the cursor is process-global and counts every launch
        // this provider has already served, which is exactly what it is for.
        assert_eq!(seen[0], seen[2]);
        assert_eq!(seen[1], seen[3]);
        assert_ne!(seen[0], seen[1]);
        let mut pool = vec![seen[0].clone(), seen[1].clone()];
        pool.sort();
        assert_eq!(
            pool,
            vec!["sk-aaaaaaaaaaaaaaaa", "sk-cccccccccccccccc"],
            "rotation covers exactly the enabled keys"
        );
        // The variable it lands in is the provider's, and nothing else is injected.
        let env = env_for_instance(&i).unwrap();
        assert_eq!(env.len(), 1);
        assert_eq!(env[0].0, "DEEPSEEK_API_KEY");
    }

    #[test]
    fn a_pinned_key_is_the_one_that_is_used_and_a_broken_pin_fails_the_launch() {
        let _g = HOME_LOCK.lock().unwrap();
        let tree = temp_tree("dispatch-pin");
        let _home = EnvGuard::set("HOME", tree.path());

        set_key("openai", "team", "sk-teamteamteamteam").unwrap();
        set_key("openai", "spare", "sk-sparesparespare1").unwrap();

        let pinned = inst("", "openai/team");
        for _ in 0..3 {
            let env = env_for_instance(&pinned).unwrap();
            assert_eq!(env[0].1, "sk-teamteamteamteam", "a pin does not rotate");
        }

        // The user asked for a specific key; running on a different one silently would
        // be a billing decision the launcher does not get to make.
        assert!(env_for_instance(&inst("", "openai/ghost")).is_err());
        assert!(env_for_instance(&inst("", "ghost/team")).is_err());

        let mut view = get_providers().unwrap();
        {
            let p = view.iter_mut().find(|p| p.id == "openai").unwrap();
            p.keys.iter_mut().find(|k| k.alias == "team").unwrap().enabled = false;
        }
        set_providers(view).unwrap();
        assert!(
            env_for_instance(&pinned).is_err(),
            "a pinned but disabled key must not fall through to another key"
        );
    }

    #[test]
    fn a_guess_from_the_engine_provider_string_never_breaks_a_launch() {
        let _g = HOME_LOCK.lock().unwrap();
        let tree = temp_tree("dispatch-implicit");
        let _home = EnvGuard::set("HOME", tree.path());

        // No reference at all, and a provider string in the engine's own namespace:
        // nothing matches, so nothing is injected and the engine keeps its own setup.
        assert!(env_for_instance(&inst("deepseek-official", ""))
            .unwrap()
            .is_empty());
        assert!(env_for_instance(&inst("", "")).unwrap().is_empty());
        // A provider with no usable key is also silent when it was only a guess…
        assert!(env_for_instance(&inst("deepseek", "")).unwrap().is_empty());
        // …but loud when the instance actually named it.
        assert!(env_for_instance(&inst("", "deepseek")).is_err());

        // Once the names do coincide, the convenience match works with no extra setup.
        set_key("deepseek", "only", "sk-onlyonlyonlyonly").unwrap();
        let env = env_for_instance(&inst("deepseek", "")).unwrap();
        assert_eq!(env[0], ("DEEPSEEK_API_KEY".into(), "sk-onlyonlyonlyonly".into()));
    }

    #[test]
    fn a_base_url_rides_along_only_when_a_variable_was_named_for_it() {
        let _g = HOME_LOCK.lock().unwrap();
        let tree = temp_tree("dispatch-baseurl");
        let _home = EnvGuard::set("HOME", tree.path());

        set_key("openai-compatible", "k", "sk-compatcompatcompat").unwrap();
        // The builtin ships `base_url_env` set but `base_url` empty: nothing to inject.
        let env = env_for_instance(&inst("", "openai-compatible")).unwrap();
        assert_eq!(env.len(), 1);

        let mut view = get_providers().unwrap();
        {
            let p = view.iter_mut().find(|p| p.id == "openai-compatible").unwrap();
            p.base_url = "http://127.0.0.1:8000/v1".into();
        }
        set_providers(view).unwrap();
        let env = env_for_instance(&inst("", "openai-compatible")).unwrap();
        assert_eq!(
            env[1],
            ("OPENAI_BASE_URL".into(), "http://127.0.0.1:8000/v1".into())
        );
    }
}
