//! Providers detected from the agents that are already installed.
//!
//! The launcher is not the first program on this machine to have been told about a
//! provider: whoever has `omp`, `pi`, `opencode`, `codex` or `dsh` on their PATH has
//! already typed a base URL and pasted a key into at least one of them. Asking for
//! all of it a second time is the launcher's problem, not the user's — so this module
//! reads those agents' own config files and offers what it finds.
//!
//! Same rule as [`crate::engines::detect_engines`]: **no disk scan.** Each source is
//! one known path belonging to one agent, and it is only read when that agent's
//! binary is actually on PATH. Nothing here executes an agent, and nothing here makes
//! a network request.
//!
//! `claude` is deliberately absent. It has no provider table — its endpoint and
//! credential come from `ANTHROPIC_*` (or an `apiKeyHelper` script), which is why
//! `EngineSpec::takes_provider` is false for it. There is nothing to import.
//!
//! ## Where a key value may go
//!
//! [`detect_agent_providers`] returns **no secrets**: a source that holds a key is
//! reported as `has_key: true` and nothing more, so the same invariant the rest of
//! this module keeps ("values never travel to the UI") holds here too. Copying a key
//! is a separate, explicitly requested act — [`import_agent_provider_keys`] — and it
//! moves the value disk-to-disk inside this process, from the agent's config into
//! `providers.json` via [`super::set_key`]. The frontend asks for it by provider id
//! and is told how many landed; it never sees a character of any key.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use serde::Serialize;

use crate::engines;

/// One provider another agent already knows about, as the UI is allowed to see it.
#[derive(Serialize, Clone, Debug, Default)]
pub struct DetectedProvider {
    /// Slug, and the id it would take in `providers.json` — so a source naming
    /// `deepseek` folds into the built-in DeepSeek row instead of shadowing it.
    pub id: String,
    pub label: String,
    pub base_url: String,
    /// Only when the source names one (codex's `env_key`, dsh's credential name).
    pub api_key_env: String,
    pub models: Vec<String>,
    /// One of the sources holds a usable-looking key. **Never the key itself.**
    pub has_key: bool,
    /// Which agents reported this provider, in the order they were read.
    pub sources: Vec<String>,
}

/// A reader's result: the provider, plus the key its source happens to hold.
///
/// Deliberately **not** `Serialize`. This type is what keeps the two halves of the
/// module honest — a key value exists only inside it, and `DetectedProvider` (the
/// thing that crosses to the frontend) has no field that could carry one.
struct Found {
    p: DetectedProvider,
    key: String,
}

/// One source: the agent whose config is being read, and the reader for it. Named
/// because the pair is what [`SOURCES`] is a table of — a reader is selected by
/// whether that agent is installed, so the id travels with the function.
type Source = (&'static str, fn() -> Vec<Found>);

/// Sources in read order, which is also merge precedence for `base_url`.
///
/// `dsh` is last on purpose: it contributes credential *names* and no endpoint, so a
/// provider another agent described in full should not have its base URL displaced.
const SOURCES: &[Source] = &[
    ("omp", from_omp),
    ("pi", from_pi),
    ("opencode", from_opencode),
    ("codex", from_codex),
    ("dsh", from_dsh),
];

fn home() -> Option<PathBuf> {
    dirs::home_dir()
}

/// Reject what is stored in a key field but is plainly not a key.
///
/// This is not paranoia about shapes — it is one real case: `omp`'s bundled
/// `free-api` provider puts the URL of the project's README where the key goes. A
/// value with whitespace, or one that is a URL, would be imported as a credential
/// and then fail at the provider with an authentication error pointing nowhere.
fn looks_like_key(v: &str) -> bool {
    let v = v.trim();
    !v.is_empty()
        && !v.contains(char::is_whitespace)
        && !v.starts_with("http://")
        && !v.starts_with("https://")
}

/// `My Proxy 2` → `my-proxy-2`; mirrors `providers::slug`, which is private.
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

fn read(path: PathBuf) -> Option<String> {
    fs::read_to_string(path).ok()
}

/// Strip `//` and `/* */` comments so `serde_json` can read a `.jsonc`.
///
/// String-aware, because a base URL is full of slashes and an escaped quote inside
/// one would otherwise end the string early and eat the rest of the file.
fn strip_jsonc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => {
                in_string = true;
                out.push(c);
            }
            '/' if chars.peek() == Some(&'/') => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut prev = '\0';
                for c in chars.by_ref() {
                    if prev == '*' && c == '/' {
                        break;
                    }
                    prev = c;
                }
                out.push(' ');
            }
            _ => out.push(c),
        }
    }
    out
}

fn parse_json(text: &str) -> Option<serde_json::Value> {
    serde_json::from_str(text)
        .ok()
        .or_else(|| serde_json::from_str(&strip_jsonc(text)).ok())
}

// ---- omp: ~/.omp/agent/models.yml -----------------------------------------
// providers:
//   <id>:
//     baseUrl: https://…/v1
//     apiKey: sk-…
//     models:
//       - id: vendor/model-name

fn from_omp() -> Vec<Found> {
    let Some(text) = home().and_then(|h| read(h.join(".omp/agent/models.yml"))) else {
        return vec![];
    };
    let Ok(docs) = yaml_rust2::YamlLoader::load_from_str(&text) else {
        return vec![];
    };
    let mut out = Vec::new();
    for doc in &docs {
        let Some(map) = doc["providers"].as_hash() else {
            continue;
        };
        for (k, v) in map {
            let Some(id) = k.as_str() else { continue };
            let models = v["models"]
                .as_vec()
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|m| m["id"].as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            let key = v["apiKey"].as_str().unwrap_or_default().to_string();
            out.push(Found {
                p: DetectedProvider {
                    id: slug(id),
                    label: id.to_string(),
                    base_url: v["baseUrl"].as_str().unwrap_or_default().to_string(),
                    models,
                    has_key: looks_like_key(&key),
                    ..Default::default()
                },
                key,
            });
        }
    }
    out
}

// ---- pi: ~/.pi/agent/models-store.json + auth.json ------------------------
// { "<provider>": { "models": [ { "id": …, "baseUrl": … } ] } }
// { "<provider>": { "type": "api", "key": "…" } }

fn from_pi() -> Vec<Found> {
    let Some(h) = home() else { return vec![] };
    let store = read(h.join(".pi/agent/models-store.json")).and_then(|t| parse_json(&t));
    let auth = read(h.join(".pi/agent/auth.json")).and_then(|t| parse_json(&t));
    let Some(store) = store.as_ref().and_then(|v| v.as_object()) else {
        return vec![];
    };
    let mut out = Vec::new();
    for (id, v) in store {
        let entries = v.get("models").and_then(|m| m.as_array());
        let models: Vec<String> = entries
            .map(|items| {
                items
                    .iter()
                    .filter_map(|m| m.get("id").and_then(|i| i.as_str()).map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        // pi records the endpoint per model rather than per provider; they agree in
        // practice, so the first one that has it speaks for the provider.
        let base_url = entries
            .and_then(|items| {
                items
                    .iter()
                    .filter_map(|m| m.get("baseUrl").and_then(|b| b.as_str()))
                    .find(|b| !b.is_empty())
            })
            .unwrap_or_default()
            .to_string();
        let key = auth
            .as_ref()
            .and_then(|a| a.get(id))
            .and_then(|e| e.get("key"))
            .and_then(|k| k.as_str())
            .unwrap_or_default()
            .to_string();
        out.push(Found {
            p: DetectedProvider {
                id: slug(id),
                label: id.clone(),
                base_url,
                models,
                has_key: looks_like_key(&key),
                ..Default::default()
            },
            key,
        });
    }
    out
}

// ---- opencode: $XDG_CONFIG_HOME/opencode/opencode.json{,c} + auth store ---
// { "provider": { "<id>": { "name": …, "options": { "baseURL": … },
//                           "models": { "<model>": {…} } } } }
// Keys live apart, in $XDG_DATA_HOME/opencode/auth.json.
//
// The auth store routinely names providers the config file does not (opencode ships
// the well-known ones), so an entry that exists only there still becomes a candidate:
// it has no endpoint to offer, but it does have the key for a provider the launcher
// may already know by that id.

fn from_opencode() -> Vec<Found> {
    let cfg = dirs::config_dir().and_then(|c| {
        read(c.join("opencode/opencode.jsonc")).or_else(|| read(c.join("opencode/opencode.json")))
    });
    let cfg = cfg.as_deref().and_then(parse_json);
    let auth = dirs::data_dir()
        .and_then(|d| read(d.join("opencode/auth.json")))
        .and_then(|t| parse_json(&t));

    let key_of = |id: &str| -> String {
        auth.as_ref()
            .and_then(|a| a.get(id))
            .and_then(|e| e.get("key"))
            .and_then(|k| k.as_str())
            .unwrap_or_default()
            .to_string()
    };

    let mut out: Vec<Found> = Vec::new();
    if let Some(map) = cfg
        .as_ref()
        .and_then(|v| v.get("provider"))
        .and_then(|p| p.as_object())
    {
        for (id, v) in map {
            let models = v
                .get("models")
                .and_then(|m| m.as_object())
                .map(|m| m.keys().cloned().collect())
                .unwrap_or_default();
            let base_url = v
                .get("options")
                .and_then(|o| o.get("baseURL"))
                .and_then(|b| b.as_str())
                .unwrap_or_default()
                .to_string();
            // An inline key in the config beats the auth store; either is the one the
            // agent itself would use.
            let inline = v
                .get("options")
                .and_then(|o| o.get("apiKey"))
                .and_then(|k| k.as_str())
                .unwrap_or_default()
                .to_string();
            let key = if looks_like_key(&inline) {
                inline
            } else {
                key_of(id)
            };
            let label = v
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or(id)
                .to_string();
            out.push(Found {
                p: DetectedProvider {
                    id: slug(id),
                    label,
                    base_url,
                    models,
                    has_key: looks_like_key(&key),
                    ..Default::default()
                },
                key,
            });
        }
    }
    if let Some(map) = auth.as_ref().and_then(|a| a.as_object()) {
        for id in map.keys() {
            if out.iter().any(|f| f.p.id == slug(id)) {
                continue;
            }
            let key = key_of(id);
            out.push(Found {
                p: DetectedProvider {
                    id: slug(id),
                    label: id.clone(),
                    has_key: looks_like_key(&key),
                    ..Default::default()
                },
                key,
            });
        }
    }
    out
}

// ---- codex: ~/.codex/config.toml ------------------------------------------
// [model_providers.<id>]
// name = "…"          base_url = "https://…/v1"
// env_key = "FOO_API_KEY"   wire_api = "chat"
//
// codex keeps no key of its own — `env_key` names the variable it reads. So the key
// this source can offer is whatever that variable holds in the launcher's own
// environment, and nothing when it is unset.

fn from_codex() -> Vec<Found> {
    let Some(text) = home().and_then(|h| read(h.join(".codex/config.toml"))) else {
        return vec![];
    };
    let Ok(doc) = text.parse::<toml::Table>() else {
        return vec![];
    };
    let Some(map) = doc.get("model_providers").and_then(|v| v.as_table()) else {
        return vec![];
    };
    let mut out = Vec::new();
    for (id, v) in map {
        let s = |field: &str| -> String {
            v.get(field)
                .and_then(|x| x.as_str())
                .unwrap_or_default()
                .to_string()
        };
        let env_key = s("env_key");
        let key = if env_key.is_empty() {
            String::new()
        } else {
            std::env::var(&env_key).unwrap_or_default()
        };
        let name = s("name");
        out.push(Found {
            p: DetectedProvider {
                id: slug(id),
                label: if name.is_empty() { id.clone() } else { name },
                base_url: s("base_url"),
                api_key_env: env_key,
                has_key: looks_like_key(&key),
                ..Default::default()
            },
            key,
        });
    }
    out
}

// ---- dsh: ~/.dsh/.credentials.yaml ----------------------------------------
// A flat `NAME: value` map. It names no endpoints, so what it contributes is the
// credential itself, attached to the provider its variable name implies:
// `DEEPSEEK_API_KEY` → `deepseek`, which is a built-in and therefore arrives with the
// vendor's own base URL already filled in.

fn from_dsh() -> Vec<Found> {
    let builtins = super::builtins();
    crate::runtime::dsh_home::credential_pairs()
        .into_iter()
        .filter_map(|(name, value)| {
            let stem = name.strip_suffix("_API_KEY")?;
            let id = slug(stem);
            if id.is_empty() {
                return None;
            }
            let b = builtins.iter().find(|b| b.id == id);
            Some(Found {
                p: DetectedProvider {
                    id: id.clone(),
                    label: b.map(|b| b.label.clone()).unwrap_or(id),
                    base_url: b.map(|b| b.base_url.clone()).unwrap_or_default(),
                    api_key_env: name,
                    has_key: looks_like_key(&value),
                    ..Default::default()
                },
                key: value,
            })
        })
        .collect()
}

// ---- merge + commands ------------------------------------------------------

/// Read every source whose agent is installed, in [`SOURCES`] order.
async fn scan() -> Vec<Found> {
    let installed: Vec<String> = engines::detect_engines()
        .await
        .into_iter()
        .filter(|e| e.installed)
        .map(|e| e.id)
        .collect();
    SOURCES
        .iter()
        .filter(|(id, _)| installed.iter().any(|i| i == id))
        .flat_map(|(id, read)| {
            read()
                .into_iter()
                .filter(|f| !f.p.id.is_empty())
                .map(move |mut f| {
                    f.p.sources = vec![id.to_string()];
                    f
                })
        })
        .collect()
}

/// What the installed agents know, one row per provider id, secrets dropped.
///
/// Two agents pointed at the same provider produce one row: the first source with a
/// base URL wins it, model lists are unioned, and `has_key` is true if any of them
/// holds one. Fresh on every call — a provider added to another agent a minute ago
/// should show up, and a cached answer would be the same stale-detection foot-gun
/// `detect_engines` avoids.
#[tauri::command]
pub async fn detect_agent_providers() -> Result<Vec<DetectedProvider>, String> {
    let mut merged: BTreeMap<String, DetectedProvider> = BTreeMap::new();
    for f in scan().await {
        let e = merged
            .entry(f.p.id.clone())
            .or_insert_with(|| DetectedProvider {
                id: f.p.id.clone(),
                label: f.p.label.clone(),
                ..Default::default()
            });
        if e.base_url.is_empty() {
            e.base_url = f.p.base_url;
        }
        if e.api_key_env.is_empty() {
            e.api_key_env = f.p.api_key_env;
        }
        e.models.extend(f.p.models);
        e.has_key |= f.p.has_key;
        e.sources.extend(f.p.sources);
    }
    let mut out: Vec<DetectedProvider> = merged.into_values().collect();
    for p in &mut out {
        p.models.sort();
        p.models.dedup();
    }
    Ok(out)
}

/// Copy the detected key for each named provider into `providers.json`.
///
/// The value is read from the agent's config and handed straight to
/// [`super::set_key`]; it does not pass through the frontend in either direction. The
/// key is stored under the source's own name as its alias (`omp`, `opencode`), so the
/// Settings list says where it came from and a second import updates that same row
/// instead of stacking duplicates.
///
/// Providers with no key anywhere are skipped, not failed — a batch is allowed to be
/// partly fruitful. The count is how many landed. A provider that is not in
/// `providers.json` yet *is* an error, and it comes from `set_key`: adopt and save the
/// row first, which is what the Settings page does before calling this.
#[tauri::command]
pub async fn import_agent_provider_keys(providers: Vec<String>) -> Result<usize, String> {
    let found = scan().await;
    let mut n = 0usize;
    for id in providers {
        let Some(f) = found
            .iter()
            .find(|f| f.p.id == id && looks_like_key(&f.key))
        else {
            continue;
        };
        let alias =
            f.p.sources
                .first()
                .cloned()
                .unwrap_or_else(|| "import".into());
        super::set_key(&id, &alias, &f.key)?;
        n += 1;
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_readme_url_in_the_key_field_is_not_a_key() {
        // omp ships `free-api` with the project's README where the key goes.
        assert!(!looks_like_key("https://github.com/smanx/free-api"));
        assert!(!looks_like_key(""));
        assert!(!looks_like_key("   "));
        assert!(!looks_like_key("two words"));
        assert!(looks_like_key("sk-abc123"));
    }

    #[test]
    fn jsonc_comments_come_out_and_urls_survive() {
        let text = r#"{
          // a line comment
          "provider": { "p": { "options": { "baseURL": "https://x.example/v1" } } },
          /* a block
             comment */
          "n": 1
        }"#;
        let v = parse_json(text).expect("jsonc should parse");
        assert_eq!(
            v["provider"]["p"]["options"]["baseURL"],
            "https://x.example/v1"
        );
        assert_eq!(v["n"], 1);
        // A slash inside a string is not a comment.
        let v = parse_json(r#"{"u": "https://a/b//c"}"#).unwrap();
        assert_eq!(v["u"], "https://a/b//c");
    }

    #[test]
    fn omp_yaml_yields_providers_with_models_and_key_status() {
        let tree = crate::test_support::temp_tree("adopt-omp");
        let _g = crate::test_support::HOME_LOCK.lock().unwrap();
        let _home = crate::test_support::EnvGuard::set("HOME", tree.path());
        let dir = tree.path().join(".omp/agent");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("models.yml"),
            "providers:\n  acme:\n    baseUrl: https://acme.test/v1\n    apiKey: sk-real\n    \
             models:\n      - id: acme/one\n      - id: acme/two\n  free:\n    baseUrl: \
             https://free.test/v1\n    apiKey: https://example.com/readme\n",
        )
        .unwrap();

        let found = from_omp();
        let acme = found.iter().find(|f| f.p.id == "acme").unwrap();
        assert_eq!(acme.p.base_url, "https://acme.test/v1");
        assert_eq!(acme.p.models, ["acme/one", "acme/two"]);
        assert!(acme.p.has_key);
        let free = found.iter().find(|f| f.p.id == "free").unwrap();
        assert!(!free.p.has_key, "a README URL is not a credential");
    }

    #[test]
    fn a_detected_provider_carries_no_key_field_at_all() {
        // The type-level half of the no-secrets rule: whatever `Found` holds, what
        // serializes toward the UI cannot name it.
        let json = serde_json::to_string(&DetectedProvider {
            id: "x".into(),
            has_key: true,
            ..Default::default()
        })
        .unwrap();
        assert!(json.contains("has_key"));
        assert!(!json.contains("\"key\""));
    }
}
