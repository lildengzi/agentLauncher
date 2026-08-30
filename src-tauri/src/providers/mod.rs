//! Providers and API keys — `~/.agentlauncher/providers.json`, mode 0600.
//!
//! This is the one launcher-owned file that holds secrets, and it exists because
//! dsh's `~/.dsh/.credentials.yaml` structurally cannot do what was asked of it:
//! it is a flat `NAME: value` map, so it holds exactly **one** key per name. Two
//! DeepSeek keys — a personal one and a team one — have nowhere to live there.
//!
//! Four rules hold this module together:
//!
//! * **Values never travel to the UI.** [`get_providers`] returns [`ProviderView`],
//!   whose keys carry a fingerprint (`sk-a…3f9c`) and nothing else. There is no
//!   command that returns a plaintext key, so the Settings dialog's eye button
//!   toggles fingerprint ↔ dots — it cannot reveal a secret it was never sent.
//! * **The frontend cannot round-trip a value it does not have.** Saving the list
//!   writes metadata only; each key's value is carried over from disk by alias
//!   (see [`save_view`]).
//! * **File permissions do the protecting.** 0600 under a 0700 root, the same
//!   treatment dsh gives its own credentials file. No OS keyring: that would add a
//!   dependency which fails on exactly the headless/SSH boxes these agents run on.
//! * **Dispatch happens at spawn, never per request.** See [`dispatch`].
//!
//! Related: [`detect`] lists models — local runtimes by port probe, cloud
//! providers by an explicit, user-triggered call to the provider's own API.
//! [`adopt`] reads the *other* installed agents' own config files, so a provider the
//! user set up once elsewhere does not have to be typed again.

pub mod adopt;
pub mod detect;
pub mod dispatch;

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::launcher_config;

fn current_format_version() -> u32 {
    1
}
fn enabled_default() -> bool {
    true
}
fn bearer() -> String {
    "bearer".into()
}

/// A stem for a provider the user only gave a name to: `My Proxy 2` → `my-proxy-2`.
/// Empty when the name has nothing usable in it, which `save` then drops.
fn slug(name: &str) -> String {
    let mut out = String::new();
    for ch in name.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

/// `openai-compatible` → `OPENAI_COMPATIBLE`. A POSIX identifier may not start with
/// a digit, so `4o-proxy` gets an underscore in front rather than being refused.
fn env_prefix(id: &str) -> String {
    let mut out = String::new();
    for ch in id.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_uppercase());
        } else {
            out.push('_');
        }
    }
    if out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

/// Fill in the two env var names so nobody has to type them.
///
/// These are the fields that made the settings page unreadable: they are mechanical
/// (`DEEPSEEK_API_KEY` for `deepseek`), the user has no way to know a good answer,
/// and a typo makes the row silently useless. So they are derived, and the form only
/// shows them under 高级 as an override for the rare engine that reads something else.
///
/// `*_API_KEY` → `*_BASE_URL` deliberately keeps the pair together: a row that
/// borrows OpenAI's key variable is an OpenAI-shaped endpoint, and gets OpenAI's
/// base-URL variable too. A **builtin**'s `base_url_env` is left exactly as shipped —
/// mostly empty — because its `base_url` is the vendor's own root, and re-injecting
/// that changes how already-working instances start (Anthropic's is worse than a
/// no-op: the `claude` CLI appends `/v1` itself). A user's own row is the opposite
/// case: they typed that URL precisely so it would be used.
fn derive_envs(p: &mut ProviderDef, builtin: bool) {
    if p.api_key_env.is_empty() && !p.id.is_empty() {
        p.api_key_env = format!("{}_API_KEY", env_prefix(&p.id));
    }
    if !builtin && p.base_url_env.is_empty() && !p.base_url.is_empty() {
        p.base_url_env = match p.api_key_env.strip_suffix("_API_KEY") {
            Some(stem) => format!("{stem}_BASE_URL"),
            None => format!("{}_BASE_URL", env_prefix(&p.id)),
        };
    }
}

/// One API key. `alias` is its identity — the name the user gave it ("personal",
/// "team-quota"), what an instance binds to, and the join key when the UI saves
/// metadata back. Two keys under one provider may not share an alias.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ProviderKey {
    pub alias: String,
    /// The secret. Never serialized toward the frontend — it only ever appears in
    /// this file and in the child process env at spawn.
    #[serde(default)]
    pub value: String,
    /// A disabled key is skipped by round-robin but kept, so pausing a key that
    /// hit its quota does not mean pasting it again next month.
    #[serde(default = "enabled_default")]
    pub enabled: bool,
}

/// One provider: where a key goes, what it is called, and which models it serves.
///
/// Every field except `id` is optional, because a user-added provider is usually
/// half-known at the time it is added. Empty means "omit", never "guess" — the
/// same contract the engine adapters follow for provider/model flags.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ProviderDef {
    /// Stable reference used by `Instance::api_key_ref`. Not shown to the user.
    pub id: String,
    #[serde(default)]
    pub label: String,
    /// The env var the engine reads the key from (`DEEPSEEK_API_KEY`). Derived from
    /// `id` on save (see [`derive_envs`]) unless the user overrode it.
    #[serde(default)]
    pub api_key_env: String,
    /// The provider's API root. Used for the model listing, and injected at spawn
    /// only when `base_url_env` names somewhere to put it.
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub base_url_env: String,
    /// How the model listing authenticates: `"bearer"`, `"x-api-key"` (Anthropic), or
    /// **empty for auto** — bearer first, retried as `x-api-key` if the provider
    /// rejects it. Affects [`detect`] only, never the launch env.
    #[serde(default)]
    pub auth_style: String,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default = "enabled_default")]
    pub enabled: bool,
    /// Shipped with the launcher: disable-able, not delete-able. Asserted by the
    /// backend on every save; the frontend does not get to claim it.
    #[serde(default)]
    pub builtin: bool,
    #[serde(default)]
    pub keys: Vec<ProviderKey>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProvidersDoc {
    #[serde(default = "current_format_version")]
    pub format_version: u32,
    #[serde(default)]
    pub providers: Vec<ProviderDef>,
}

impl Default for ProvidersDoc {
    fn default() -> Self {
        Self {
            format_version: current_format_version(),
            providers: builtins(),
        }
    }
}

// ---- the masked view the frontend sees -------------------------------------

/// A key as the UI is allowed to know it: named, toggleable, and identifiable by
/// fingerprint — but not readable.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ProviderKeyView {
    pub alias: String,
    pub enabled: bool,
    /// `sk-a…3f9c`, or dots when the value is too short for a window that small to
    /// be safe. Empty when the row has no value yet.
    pub fingerprint: String,
    /// False ⇒ this row is a placeholder waiting for a value; dispatch skips it.
    pub has_value: bool,
}

/// A provider as the UI sees it: [`ProviderDef`] with every key masked.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ProviderView {
    pub id: String,
    pub label: String,
    pub api_key_env: String,
    pub base_url: String,
    pub base_url_env: String,
    pub auth_style: String,
    pub models: Vec<String>,
    pub enabled: bool,
    pub builtin: bool,
    pub keys: Vec<ProviderKeyView>,
}

// ---- builtins --------------------------------------------------------------

/// The providers every install starts with — the three that were hardcoded in the
/// settings dialog, plus Anthropic, which the `claude` engine needs and which the
/// old list had no room for.
///
/// `base_url` is filled in with each provider's real API root so the model listing
/// has somewhere to call. `base_url_env` is deliberately left **empty**: a base URL
/// is only injected into a launch when the user names the variable to put it in, so
/// installing this round changes nothing about how existing instances start.
///
/// **`models` is empty on purpose, for every row.** A shipped model list is wrong the
/// moment a vendor renames a model, and it is wrong in the worst way: it looks
/// authoritative, so the launch fails at the provider with an unknown-model error
/// instead of anywhere the user would think to look. `fetch_provider_models` asks the
/// provider itself, which is the only source that cannot go stale — so the cost of
/// this is one press of 拉取模型列表 per provider, and the alternative was a list
/// that quietly rots. Same "空值即省略" contract the engine adapters follow.
pub fn builtins() -> Vec<ProviderDef> {
    vec![
        ProviderDef {
            id: "deepseek".into(),
            label: "DeepSeek".into(),
            api_key_env: "DEEPSEEK_API_KEY".into(),
            base_url: "https://api.deepseek.com/v1".into(),
            base_url_env: String::new(),
            auth_style: bearer(),
            models: Vec::new(),
            enabled: true,
            builtin: true,
            keys: Vec::new(),
        },
        ProviderDef {
            id: "openai".into(),
            label: "OpenAI".into(),
            api_key_env: "OPENAI_API_KEY".into(),
            base_url: "https://api.openai.com/v1".into(),
            base_url_env: String::new(),
            auth_style: bearer(),
            models: Vec::new(),
            enabled: true,
            builtin: true,
            keys: Vec::new(),
        },
        ProviderDef {
            id: "anthropic".into(),
            label: "Anthropic".into(),
            // What the `claude` engine reads; see runtime::model's claude adapter.
            api_key_env: "ANTHROPIC_API_KEY".into(),
            base_url: "https://api.anthropic.com/v1".into(),
            base_url_env: String::new(),
            // Anthropic authenticates with `x-api-key`, not a bearer token.
            auth_style: "x-api-key".into(),
            models: Vec::new(),
            enabled: true,
            builtin: true,
            keys: Vec::new(),
        },
        ProviderDef {
            id: "openai-compatible".into(),
            label: "OpenAI-Compatible".into(),
            api_key_env: "OPENAI_API_KEY".into(),
            // No default: this row exists for an endpoint only the user knows.
            base_url: String::new(),
            base_url_env: "OPENAI_BASE_URL".into(),
            auth_style: bearer(),
            models: Vec::new(),
            enabled: true,
            builtin: true,
            keys: Vec::new(),
        },
    ]
}

// ---- io --------------------------------------------------------------------

fn providers_path() -> Result<PathBuf, String> {
    Ok(launcher_config::agentlauncher_root()?.join("providers.json"))
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// Write the document as owner-only JSON: 0600 file under a 0700 root.
///
/// The mode is set on the temp file *before* the rename, so the finished
/// `providers.json` never exists — not even for an instant — at the 0644 a fresh
/// write would give it. The root is tightened too: it already holds every
/// instance's `.env`, which is where the other engines' keys live.
fn write_secret_json(path: &Path, doc: &ProvidersDoc) -> Result<(), String> {
    let root = path
        .parent()
        .ok_or("providers.json has no parent directory")?;
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    let _ = set_mode(root, 0o700);

    let text = serde_json::to_string_pretty(doc).map_err(|e| e.to_string())? + "\n";
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    fs::write(&tmp, &text).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    set_mode(&tmp, 0o600)?;
    if fs::rename(&tmp, path).is_err() {
        // e.g. a filesystem where rename onto an existing file fails.
        fs::write(path, &text).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        set_mode(path, 0o600)?;
        let _ = fs::remove_file(&tmp);
    }
    Ok(())
}

/// The document with values. Built-ins missing from the file are appended, and
/// `builtin` is re-asserted, so "I deleted DeepSeek and now there is no DeepSeek
/// ever again" is not a reachable state.
pub fn load() -> Result<ProvidersDoc, String> {
    let mut doc: ProvidersDoc = launcher_config::read_or_default(&providers_path()?);
    doc.format_version = current_format_version();
    for b in builtins() {
        if !doc.providers.iter().any(|p| p.id == b.id) {
            doc.providers.push(b);
        }
    }
    for p in &mut doc.providers {
        p.builtin = builtins().iter().any(|b| b.id == p.id);
    }
    Ok(doc)
}

/// Normalise then write. Ids and aliases are trimmed, blanks dropped and
/// duplicates removed first-wins; a dropped built-in comes back disabled rather
/// than re-enabled behind the user's back.
///
/// This is also where the fields the form no longer asks for get their values: a row
/// that arrives with only a name and a Base URL leaves with an id and both env var
/// names filled in ([`derive_envs`]).
pub fn save(mut doc: ProvidersDoc) -> Result<(), String> {
    doc.format_version = current_format_version();
    let builtin_ids: Vec<String> = builtins().into_iter().map(|b| b.id).collect();
    let mut seen: Vec<String> = Vec::new();
    for p in &mut doc.providers {
        p.id = p.id.trim().to_string();
        p.label = p.label.trim().to_string();
        // The form shows one name field, so a new row arrives with no id at all.
        if p.id.is_empty() {
            p.id = slug(&p.label);
        }
        p.api_key_env = p.api_key_env.trim().to_string();
        p.base_url_env = p.base_url_env.trim().to_string();
        p.base_url = p.base_url.trim().to_string();
        if p.label.is_empty() {
            p.label = p.id.clone();
        }
        // Empty is a real value here now: it means "work the auth scheme out".
        p.auth_style = p.auth_style.trim().to_string();
        derive_envs(p, builtin_ids.contains(&p.id));
        // An env var name is the one field a typo makes silently useless, so it is
        // checked here rather than discovered at spawn. Derived names pass by
        // construction; this catches the 高级 override.
        for env in [&p.api_key_env, &p.base_url_env] {
            if !env.is_empty() && !crate::runtime::dsh_home::is_posix_identifier(env) {
                return Err(format!("环境变量名必须是 POSIX 标识符: {env}"));
            }
        }
        p.models = p
            .models
            .iter()
            .map(|m| m.trim().to_string())
            .filter(|m| !m.is_empty())
            .collect();
        let mut aliases: Vec<String> = Vec::new();
        p.keys.retain_mut(|k| {
            k.alias = k.alias.trim().to_string();
            let keep = !k.alias.is_empty() && !aliases.contains(&k.alias);
            if keep {
                aliases.push(k.alias.clone());
            }
            keep
        });
    }
    doc.providers.retain(|p| {
        let keep = !p.id.is_empty() && !seen.contains(&p.id);
        if keep {
            seen.push(p.id.clone());
        }
        keep
    });
    for b in builtins() {
        if !doc.providers.iter().any(|p| p.id == b.id) {
            doc.providers.push(ProviderDef {
                enabled: false,
                ..b
            });
        }
    }
    for p in &mut doc.providers {
        p.builtin = builtins().iter().any(|b| b.id == p.id);
    }
    write_secret_json(&providers_path()?, &doc)
}

// ---- masking ---------------------------------------------------------------

/// A hint that lets a user tell two of their own keys apart without showing either.
///
/// Four leading and four trailing characters is enough for that (`sk-p…9f2a`), and
/// on a real key it is a negligible fraction of the secret. On a *short* value that
/// same window would be most of it, so anything under 12 characters is replaced by
/// dots outright rather than partially leaked.
pub fn fingerprint(value: &str) -> String {
    let n = value.chars().count();
    if n == 0 {
        return String::new();
    }
    if n < 12 {
        return "•".repeat(8);
    }
    let head: String = value.chars().take(4).collect();
    let tail: String = value.chars().skip(n - 4).collect();
    format!("{head}…{tail}")
}

impl ProviderDef {
    /// The masked projection. This is the only shape that crosses to the frontend.
    pub fn view(&self) -> ProviderView {
        ProviderView {
            id: self.id.clone(),
            label: self.label.clone(),
            api_key_env: self.api_key_env.clone(),
            base_url: self.base_url.clone(),
            base_url_env: self.base_url_env.clone(),
            auth_style: self.auth_style.clone(),
            models: self.models.clone(),
            enabled: self.enabled,
            builtin: self.builtin,
            keys: self
                .keys
                .iter()
                .map(|k| ProviderKeyView {
                    alias: k.alias.clone(),
                    enabled: k.enabled,
                    fingerprint: fingerprint(&k.value),
                    has_value: !k.value.is_empty(),
                })
                .collect(),
        }
    }
}

/// Write metadata from the UI, keeping every secret the UI never received.
///
/// Values are carried over from disk by `(provider id, key alias)`. One consequence
/// worth knowing: **renaming an alias drops that key's value**, because the rename
/// leaves nothing on disk to join against. The row then shows as valueless and asks
/// for a new key — which is a visible outcome, unlike silently reattaching a secret
/// to a name the user has changed.
pub fn save_view(view: Vec<ProviderView>) -> Result<(), String> {
    let disk = load()?;
    let providers = view
        .into_iter()
        .map(|v| {
            let old = disk.providers.iter().find(|p| p.id == v.id);
            let keys = v
                .keys
                .iter()
                .map(|k| ProviderKey {
                    alias: k.alias.clone(),
                    value: old
                        .and_then(|p| p.keys.iter().find(|d| d.alias == k.alias))
                        .map(|d| d.value.clone())
                        .unwrap_or_default(),
                    enabled: k.enabled,
                })
                .collect();
            ProviderDef {
                id: v.id,
                label: v.label,
                api_key_env: v.api_key_env,
                base_url: v.base_url,
                base_url_env: v.base_url_env,
                auth_style: v.auth_style,
                models: v.models,
                enabled: v.enabled,
                // Ours to assert (in `save`), not the caller's to claim.
                builtin: false,
                keys,
            }
        })
        .collect();
    save(ProvidersDoc {
        format_version: current_format_version(),
        providers,
    })
}

/// Set or clear one key's value. The only path by which a secret enters the file.
///
/// An empty `value` deletes the row outright rather than blanking it: a key the user
/// cleared should not linger as a disabled placeholder they have to clean up twice.
/// The value is trimmed — a pasted key routinely arrives with a trailing newline,
/// and a key whose real value has surrounding whitespace does not exist.
pub fn set_key(provider_id: &str, alias: &str, value: &str) -> Result<(), String> {
    let alias = alias.trim().to_string();
    if alias.is_empty() {
        return Err("API Key 需要一个名称".into());
    }
    let value = value.trim().to_string();
    let mut doc = load()?;
    let p = doc
        .providers
        .iter_mut()
        .find(|p| p.id == provider_id)
        .ok_or_else(|| format!("没有这个 provider: {provider_id}"))?;
    if value.is_empty() {
        p.keys.retain(|k| k.alias != alias);
    } else {
        match p.keys.iter_mut().find(|k| k.alias == alias) {
            Some(k) => k.value = value,
            None => p.keys.push(ProviderKey {
                alias,
                value,
                enabled: true,
            }),
        }
    }
    save(doc)
}

// ---- commands --------------------------------------------------------------

/// Every provider, with keys masked. Never returns a secret.
#[tauri::command]
pub fn get_providers() -> Result<Vec<ProviderView>, String> {
    Ok(load()?.providers.iter().map(|p| p.view()).collect())
}

#[tauri::command]
pub fn set_providers(providers: Vec<ProviderView>) -> Result<(), String> {
    save_view(providers)
}

#[tauri::command]
pub fn set_provider_key(provider: String, alias: String, value: String) -> Result<(), String> {
    set_key(&provider, &alias, &value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{temp_tree, EnvGuard, HOME_LOCK};

    #[test]
    fn missing_file_yields_the_builtins_with_no_keys() {
        let _g = HOME_LOCK.lock().unwrap();
        let tree = temp_tree("providers-empty");
        let _home = EnvGuard::set("HOME", tree.path());

        let doc = load().unwrap();
        assert_eq!(doc.format_version, 1);
        assert!(doc.providers.iter().all(|p| p.builtin && p.enabled));
        assert!(doc.providers.iter().all(|p| p.keys.is_empty()));
        // Anthropic is present because the `claude` engine reads ANTHROPIC_API_KEY.
        let a = doc.providers.iter().find(|p| p.id == "anthropic").unwrap();
        assert_eq!(a.api_key_env, "ANTHROPIC_API_KEY");
        assert_eq!(a.auth_style, "x-api-key");
        // No base URL is injected by default, so this round cannot re-point a launch.
        assert!(doc
            .providers
            .iter()
            .all(|p| p.base_url_env.is_empty() || p.base_url.is_empty()));
        // No model list ships with a builtin: a hardcoded one goes stale silently and
        // then fails at the provider. `fetch_provider_models` is the source of truth,
        // which is why every builtin that has a real API root carries it.
        assert!(
            doc.providers.iter().all(|p| p.models.is_empty()),
            "builtins must ship no model list — pull it from the provider instead"
        );
        assert!(doc
            .providers
            .iter()
            .filter(|p| p.id != "openai-compatible")
            .all(|p| p.base_url.starts_with("https://")));
    }

    #[test]
    fn a_stored_key_is_never_returned_to_the_frontend() {
        let _g = HOME_LOCK.lock().unwrap();
        let tree = temp_tree("providers-mask");
        let _home = EnvGuard::set("HOME", tree.path());

        set_key("deepseek", "personal", "sk-abcdefghijklmnop9f2a").unwrap();
        let view = get_providers().unwrap();
        let p = view.iter().find(|p| p.id == "deepseek").unwrap();
        let k = &p.keys[0];
        assert_eq!(k.alias, "personal");
        assert!(k.has_value && k.enabled);
        assert_eq!(k.fingerprint, "sk-a…9f2a");
        // The whole serialized view, not just the field we looked at, is free of it.
        let json = serde_json::to_string(&view).unwrap();
        assert!(!json.contains("sk-abcdefghijklmnop9f2a"));
        assert!(!json.contains("abcdefghijklmnop"));

        // A short value is not partially leaked.
        assert_eq!(fingerprint("sk-short"), "••••••••");
        assert_eq!(fingerprint(""), "");
    }

    #[test]
    fn the_file_is_owner_only() {
        let _g = HOME_LOCK.lock().unwrap();
        let tree = temp_tree("providers-perm");
        let _home = EnvGuard::set("HOME", tree.path());

        set_key("openai", "work", "sk-0123456789abcdef").unwrap();
        let path = providers_path().unwrap();
        assert!(path.exists());
        // No temp file left beside the secret.
        assert!(!path.with_file_name("providers.json.tmp").exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "providers.json must not be readable by others");
            let dir = fs::metadata(path.parent().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(dir, 0o700);
        }
    }

    #[test]
    fn saving_metadata_keeps_values_and_a_rename_drops_one() {
        let _g = HOME_LOCK.lock().unwrap();
        let tree = temp_tree("providers-merge");
        let _home = EnvGuard::set("HOME", tree.path());

        set_key("deepseek", "personal", "sk-personal-000000001").unwrap();
        set_key("deepseek", "team", "sk-team-0000000000002").unwrap();

        // The UI sends back what it was given — masked rows, one of them disabled —
        // and both secrets have to survive the round trip.
        let mut view = get_providers().unwrap();
        {
            let p = view.iter_mut().find(|p| p.id == "deepseek").unwrap();
            p.keys[1].enabled = false;
            p.models = vec!["deepseek-chat".into()];
        }
        set_providers(view).unwrap();

        let doc = load().unwrap();
        let p = doc.providers.iter().find(|p| p.id == "deepseek").unwrap();
        assert_eq!(p.models, vec!["deepseek-chat"]);
        assert_eq!(p.keys[0].value, "sk-personal-000000001");
        assert_eq!(p.keys[1].value, "sk-team-0000000000002");
        assert!(!p.keys[1].enabled);
        assert!(p.builtin, "builtin is asserted by the backend");

        // Renaming an alias has nothing to join against, so the row loses its value
        // and asks for a new one rather than inheriting the old secret.
        let mut view = get_providers().unwrap();
        {
            let p = view.iter_mut().find(|p| p.id == "deepseek").unwrap();
            p.keys[0].alias = "personal-2".into();
        }
        set_providers(view).unwrap();
        let doc = load().unwrap();
        let p = doc.providers.iter().find(|p| p.id == "deepseek").unwrap();
        let renamed = p.keys.iter().find(|k| k.alias == "personal-2").unwrap();
        assert!(renamed.value.is_empty());
    }

    #[test]
    fn user_rows_are_validated_and_builtins_cannot_be_deleted() {
        let _g = HOME_LOCK.lock().unwrap();
        let tree = temp_tree("providers-save");
        let _home = EnvGuard::set("HOME", tree.path());

        // A user replaces the whole list with one row of their own, tries to claim it
        // is shipped, and leaves a blank alias and a duplicate in the key list.
        set_providers(vec![ProviderView {
            id: " local-vllm ".into(),
            label: String::new(),
            api_key_env: "VLLM_API_KEY".into(),
            base_url: "http://127.0.0.1:8000/v1".into(),
            base_url_env: "OPENAI_BASE_URL".into(),
            auth_style: String::new(),
            models: vec![" qwen3 ".into(), "".into()],
            enabled: true,
            builtin: true,
            keys: vec![
                ProviderKeyView {
                    alias: "a".into(),
                    enabled: true,
                    ..Default::default()
                },
                ProviderKeyView {
                    alias: " ".into(),
                    enabled: true,
                    ..Default::default()
                },
                ProviderKeyView {
                    alias: "a".into(),
                    enabled: false,
                    ..Default::default()
                },
            ],
        }])
        .unwrap();

        let doc = load().unwrap();
        let mine = doc.providers.iter().find(|p| p.id == "local-vllm").unwrap();
        assert!(!mine.builtin, "a user row cannot claim to be builtin");
        assert_eq!(
            mine.label, "local-vllm",
            "a blank label falls back to the id"
        );
        assert_eq!(mine.auth_style, "", "empty means auto-detect, not bearer");
        assert_eq!(mine.models, vec!["qwen3"]);
        assert_eq!(
            mine.keys.len(),
            1,
            "blank and duplicate aliases are dropped"
        );
        // Restored, and not silently re-enabled.
        let ds = doc.providers.iter().find(|p| p.id == "deepseek").unwrap();
        assert!(ds.builtin && !ds.enabled);

        // An env var name that no shell could export is refused at the seam.
        let mut view = get_providers().unwrap();
        view[0].api_key_env = "not a var".into();
        assert!(set_providers(view).is_err());
    }

    #[test]
    fn a_row_with_only_a_name_and_a_url_gets_the_rest_derived() {
        let _g = HOME_LOCK.lock().unwrap();
        let tree = temp_tree("providers-derive");
        let _home = EnvGuard::set("HOME", tree.path());

        // Exactly what the settings form now sends: a name, a Base URL, nothing else.
        let mut view = get_providers().unwrap();
        view.push(ProviderView {
            label: "My Proxy 2".into(),
            base_url: "https://proxy.example.com/v1".into(),
            enabled: true,
            ..Default::default()
        });
        set_providers(view).unwrap();

        let doc = load().unwrap();
        let mine = doc.providers.iter().find(|p| p.id == "my-proxy-2").unwrap();
        assert_eq!(mine.label, "My Proxy 2", "the name is kept as written");
        assert_eq!(mine.api_key_env, "MY_PROXY_2_API_KEY");
        // Derived from the key variable, so a row borrowing OpenAI's key variable
        // would get OPENAI_BASE_URL rather than a name nothing reads.
        assert_eq!(mine.base_url_env, "MY_PROXY_2_BASE_URL");

        // A builtin keeps the base-URL variable it shipped with — injecting a vendor's
        // own root into an already-working instance is at best a no-op and at worst
        // (Anthropic, which appends /v1 itself) a broken endpoint.
        let a = doc.providers.iter().find(|p| p.id == "anthropic").unwrap();
        assert!(a.base_url_env.is_empty());
        assert_eq!(a.api_key_env, "ANTHROPIC_API_KEY");

        assert_eq!(slug("  DeepSeek 官方 "), "deepseek");
        assert_eq!(env_prefix("4o-proxy"), "_4O_PROXY");
    }

    #[test]
    fn clearing_a_value_removes_the_row_and_unknown_providers_are_refused() {
        let _g = HOME_LOCK.lock().unwrap();
        let tree = temp_tree("providers-clear");
        let _home = EnvGuard::set("HOME", tree.path());

        // A pasted key arrives with a trailing newline; the stored value has none.
        set_key("openai", "work", "  sk-0123456789abcdef\n").unwrap();
        let doc = load().unwrap();
        let p = doc.providers.iter().find(|p| p.id == "openai").unwrap();
        assert_eq!(p.keys[0].value, "sk-0123456789abcdef");

        set_key("openai", "work", "").unwrap();
        let doc = load().unwrap();
        let p = doc.providers.iter().find(|p| p.id == "openai").unwrap();
        assert!(
            p.keys.is_empty(),
            "clearing a key deletes it, secret and all"
        );

        assert!(set_key("nope", "work", "sk-1").is_err());
        assert!(set_key("openai", "  ", "sk-1").is_err());
    }
}
