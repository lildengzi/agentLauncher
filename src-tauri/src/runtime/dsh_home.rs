//! Everything the launcher reads or writes inside `$DSH_HOME` (default `~/.dsh`).
//!
//! This is dsh's *own installation*, not a launcher-level concern — hence its home
//! under `runtime/`, beside the engine adapters. At the top level, next to
//! `launcher_config.rs`, it would read as that module's peer: one of two config
//! modules, as if every engine had one here. None of this applies to the other
//! five — they keep credentials and settings in their own homes, reached through
//! the instance `.env`.
//!
//! dsh has no `config`/`model`/`market` subcommands; its state is files:
//!   * `.credentials.yaml` — a flat `KEY: value` mapping (mode 0600) of credential
//!     references (e.g. DEEPSEEK_API_KEY) to secret values.
//!   * `settings.yaml`     — the hot-reloaded user settings document. Its
//!     `llm-pi-ai: providers:` dict is what *adds* model routes (see
//!     [`model_routes`]); the launcher only reads it.
//!   * `profiles/<name>/`   — one profile per dir; `cordis.patch.yml` is the user
//!     patch layer and `package.json` `dependencies` are the installed plugins.
//!
//! The default agent model is the `agent-default-model` plugin's `{provider, model}`
//! config, overridable per run via `--patch <file>` (written by `model.rs`). Its
//! `provider` is a dsh **route**, not a launcher provider id — [`model_routes`].
//!
//! Plugins are managed with `dsh plugin --profile <name> add|remove <pkg>`, which
//! forwards to pnpm inside the profile directory. There is no remote plugin market.
//!
//! Secret *values* stay on disk: `list_credential_keys` hands the UI names only.

use std::fs;
use std::path::PathBuf;

/// `$DSH_HOME` or `~/.dsh`. Private: outside this module the dsh home is only ever
/// reached through the functions below, never assembled by hand.
fn root() -> Result<PathBuf, String> {
    if let Ok(h) = std::env::var("DSH_HOME") {
        if !h.trim().is_empty() {
            return Ok(PathBuf::from(h));
        }
    }
    let home = dirs::home_dir().ok_or("cannot resolve home directory")?;
    Ok(home.join(".dsh"))
}

fn credentials_path() -> Result<PathBuf, String> {
    Ok(root()?.join(".credentials.yaml"))
}

pub(crate) fn is_posix_identifier(k: &str) -> bool {
    !k.is_empty()
        && k.chars()
            .next()
            .map(|c| c.is_ascii_alphabetic() || c == '_')
            .unwrap_or(false)
        && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Split a `.credentials.yaml` line into (indent-free key, value) if it is a
/// simple top-level `KEY: value` entry. Comments and blanks return None.
fn split_kv(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    // Only treat unindented entries as top-level keys.
    if line.starts_with(char::is_whitespace) {
        return None;
    }
    let (k, v) = trimmed.split_once(':')?;
    let k = k.trim();
    if !is_posix_identifier(k) {
        return None;
    }
    Some((k.to_string(), v.trim().to_string()))
}

/// Names of credentials currently stored in `.credentials.yaml` (values are never
/// returned to the UI). Missing file ⇒ empty list.
#[tauri::command]
pub fn list_credential_keys() -> Result<Vec<String>, String> {
    let path = credentials_path()?;
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => return Ok(vec![]),
    };
    let mut keys: Vec<String> = text.lines().filter_map(split_kv).map(|(k, _)| k).collect();
    keys.sort();
    keys.dedup();
    Ok(keys)
}

/// Credential `(name, value)` pairs, for backend code that must read a value.
///
/// **Not a command, and `pub(crate)` for that reason.** `list_credential_keys` is the
/// version the UI gets, and the difference between the two is the whole invariant: a
/// key value may be read here and moved to another file on disk, but it must never be
/// returned across a command boundary. The only caller is `providers::adopt`, which
/// copies a detected key into `providers.json`.
pub(crate) fn credential_pairs() -> Vec<(String, String)> {
    let Ok(path) = credentials_path() else {
        return vec![];
    };
    let Ok(text) = fs::read_to_string(&path) else {
        return vec![];
    };
    text.lines().filter_map(split_kv).collect()
}

/// Upsert (non-empty value) or remove (empty value) a credential, preserving all
/// other lines and comments. Writes the document owner-only (0600) under a 0700 home.
#[tauri::command]
pub fn set_credential(key: String, value: String) -> Result<(), String> {
    let key = key.trim().to_string();
    if !is_posix_identifier(&key) {
        return Err(format!("凭据名必须是 POSIX 标识符: {key}"));
    }
    let value = value.trim().to_string();

    let home = root()?;
    let path = home.join(".credentials.yaml");
    let existing = fs::read_to_string(&path).unwrap_or_default();

    let mut out_lines: Vec<String> = Vec::new();
    let mut replaced = false;
    for line in existing.lines() {
        match split_kv(line) {
            Some((k, _)) if k == key => {
                if !value.is_empty() {
                    out_lines.push(format!("{key}: {value}"));
                }
                replaced = true; // dropping the line handles the unset case
            }
            _ => out_lines.push(line.to_string()),
        }
    }
    if !replaced && !value.is_empty() {
        out_lines.push(format!("{key}: {value}"));
    }
    let mut body = out_lines.join("\n");
    if !body.is_empty() && !body.ends_with('\n') {
        body.push('\n');
    }

    write_owner_only(&home, &path, &body)
}

/// Create $DSH_HOME 0700 if needed and write `body` to `path` with mode 0600.
fn write_owner_only(home: &PathBuf, path: &PathBuf, body: &str) -> Result<(), String> {
    fs::create_dir_all(home).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(home, fs::Permissions::from_mode(0o700));
    }
    fs::write(path, body).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// One dsh profile as the picker needs it — mirrors `DshProfile` in src/types.ts.
///
/// `web` is the launcher's real judgement (`profile_is_web_capable`), not a guess
/// from the name: web-ness comes from the profile's bundled packages, so a profile
/// called `daily` can be interactive and one called `web` need not be.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DshProfile {
    pub name: String,
    pub web: bool,
}

/// Profiles under `$DSH_HOME/profiles` (excludes `node_modules`), each with its
/// web capability resolved.
#[tauri::command]
pub fn list_dsh_profiles() -> Result<Vec<DshProfile>, String> {
    let dir = root()?.join("profiles");
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut out: Vec<String> = vec![];
    for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.path().is_dir() {
            continue;
        }
        if let Some(name) = entry.file_name().to_str() {
            if name == "node_modules" || name.starts_with('.') {
                continue;
            }
            out.push(name.to_string());
        }
    }
    out.sort();
    Ok(out
        .into_iter()
        .map(|name| DshProfile {
            web: profile_is_web_capable(&name),
            name,
        })
        .collect())
}

// ---- model routes ----------------------------------------------------------

/// The one provider route dsh ships wired: `@deepseek-ai/dsh-llm-deepseek` owns it
/// and `dsh-base` mounts that adapter in every profile, so it is available even in a
/// dsh home with no settings document at all.
pub(crate) const NATIVE_ROUTE: &str = "deepseek-official";

fn settings_path() -> Result<PathBuf, String> {
    Ok(root()?.join("settings.yaml"))
}

/// Provider **routes** a dsh run can resolve a model on.
///
/// This is not the launcher's provider list and must not be confused with it — the
/// two are different namespaces. `providers.json` holds *the launcher's* ids
/// (`deepseek`, `free-api`, `mcapple`), which is where a key is stored and which env
/// var it lands in. A dsh route is a name registered on `ctx.llm` inside the running
/// harness, and only two things register one:
///
/// * the native DeepSeek adapter, which owns exactly [`NATIVE_ROUTE`]; and
/// * `@deepseek-ai/dsh-llm-pi-ai`, which is mounted dormant and registers one route
///   per key of the `llm-pi-ai: providers:` dict in `$DSH_HOME/settings.yaml` — the
///   section dsh's own web Models page writes.
///
/// So this reads that dict. A route dsh does not have makes
/// `agent-default-model` unresolvable, which is what the user sees as 模型不可用 —
/// hence [`crate::runtime::model`] refuses to write one rather than letting the
/// failure happen inside the agent.
pub(crate) fn model_routes() -> Vec<String> {
    let mut out = vec![NATIVE_ROUTE.to_string()];
    let Ok(path) = settings_path() else {
        return out;
    };
    let Ok(text) = fs::read_to_string(&path) else {
        return out; // no settings document ⇒ the native route is all there is
    };
    let Ok(docs) = yaml_rust2::YamlLoader::load_from_str(&text) else {
        return out;
    };
    for doc in &docs {
        if let Some(map) = doc["llm-pi-ai"]["providers"].as_hash() {
            for (k, _) in map {
                if let Some(route) = k.as_str() {
                    let route = route.trim();
                    if !route.is_empty() && !out.iter().any(|r| r == route) {
                        out.push(route.to_string());
                    }
                }
            }
        }
    }
    out
}

/// The routes above, for the UI: an instance's dsh 服务商 field is a route name, so
/// the editor can offer the real set instead of a text box that accepts anything.
#[tauri::command]
pub fn list_dsh_model_routes() -> Result<Vec<String>, String> {
    Ok(model_routes())
}

fn profile_dir(profile: &str) -> Result<PathBuf, String> {
    if profile.is_empty()
        || profile.contains('/')
        || profile.contains('\\')
        || profile.contains("..")
    {
        return Err(format!("非法 profile 名: {profile}"));
    }
    Ok(root()?.join("profiles").join(profile))
}

/// True when the profile boots dsh's browser-UI server rather than answering a
/// one-shot task — i.e. its `dsh.profile.bundles` include the web-app plugin.
/// The launcher uses this to decide the run shape (serve vs. headless task);
/// missing/unreadable profile ⇒ false (treat as a plain task profile).
pub fn profile_is_web_capable(profile: &str) -> bool {
    let Ok(dir) = profile_dir(profile) else {
        return false;
    };
    let text = match fs::read_to_string(dir.join("package.json")) {
        Ok(t) => t,
        Err(_) => return false,
    };
    let json: serde_json::Value = match serde_json::from_str(&text) {
        Ok(j) => j,
        Err(_) => return false,
    };
    json.get("dsh")
        .and_then(|d| d.get("profile"))
        .and_then(|p| p.get("bundles"))
        .and_then(|b| b.as_array())
        .map(|arr| {
            arr.iter()
                .any(|v| v.as_str() == Some("@deepseek-ai/dsh-web-app"))
        })
        .unwrap_or(false)
}

/// Package names in a profile's `package.json` `dependencies` — the real
/// installed-plugin set for that profile.
#[tauri::command]
pub fn list_installed_plugins(profile: String) -> Result<Vec<String>, String> {
    let pkg = profile_dir(&profile)?.join("package.json");
    let text = match fs::read_to_string(&pkg) {
        Ok(t) => t,
        Err(_) => return Ok(vec![]),
    };
    let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let mut out = vec![];
    if let Some(deps) = json.get("dependencies").and_then(|d| d.as_object()) {
        out.extend(deps.keys().cloned());
    }
    out.sort();
    Ok(out)
}

/// Run `dsh plugin --profile <profile> add <pkg>` (pnpm passthrough). Returns the
/// combined stdout/stderr on success; errors carry the tail of the output.
#[tauri::command]
pub async fn plugin_add(profile: String, pkg: String) -> Result<String, String> {
    run_plugin(&profile, "add", &pkg).await
}

/// Run `dsh plugin --profile <profile> remove <pkg>`.
#[tauri::command]
pub async fn plugin_remove(profile: String, pkg: String) -> Result<String, String> {
    run_plugin(&profile, "remove", &pkg).await
}

async fn run_plugin(profile: &str, verb: &str, pkg: &str) -> Result<String, String> {
    // Validate the profile path even though the CLI resolves it, to reject traversal.
    profile_dir(profile)?;
    if pkg.trim().is_empty() {
        return Err("插件包名不能为空".into());
    }
    let output = tokio::process::Command::new("dsh")
        .arg("plugin")
        .arg("--profile")
        .arg(profile)
        .arg(verb)
        .arg(pkg)
        .output()
        .await
        .map_err(|e| format!("无法执行 dsh plugin: {e}"))?;

    let mut combined = String::from_utf8_lossy(&output.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&output.stderr));
    if output.status.success() {
        Ok(combined)
    } else {
        let lines: Vec<&str> = combined.lines().collect();
        let tail = lines[lines.len().saturating_sub(12)..].join("\n");
        Err(format!(
            "dsh plugin {verb} 失败 (code {:?}):\n{tail}",
            output.status.code()
        ))
    }
}
