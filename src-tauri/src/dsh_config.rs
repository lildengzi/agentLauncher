// Real dsh (DeepSeek Harness) configuration wiring.
//
// dsh has no `config`/`model`/`market` subcommands; its configuration lives in
// files under $DSH_HOME (default ~/.dsh):
//   * `.credentials.yaml` — a flat `KEY: value` mapping (mode 0600) of credential
//     references (e.g. DEEPSEEK_API_KEY) to secret values.
//   * `profiles/<name>/`   — one profile per dir; `cordis.patch.yml` is the user
//     patch layer and `package.json` `dependencies` are the installed plugins.
// The default agent model is the `agent-default-model` plugin's `{provider, model}`
// config, overridable per run via `--patch <file>` (see dsh_runner).
//
// Plugins are managed with `dsh plugin --profile <name> add|remove <pkg>`, which
// forwards to pnpm inside the profile directory. There is no remote plugin market.

use std::fs;
use std::path::PathBuf;

/// `$DSH_HOME` or `~/.dsh`.
pub fn dsh_home() -> Result<PathBuf, String> {
    if let Ok(h) = std::env::var("DSH_HOME") {
        if !h.trim().is_empty() {
            return Ok(PathBuf::from(h));
        }
    }
    let home = dirs::home_dir().ok_or("cannot resolve home directory")?;
    Ok(home.join(".dsh"))
}

fn credentials_path() -> Result<PathBuf, String> {
    Ok(dsh_home()?.join(".credentials.yaml"))
}

fn is_posix_identifier(k: &str) -> bool {
    !k.is_empty()
        && k
            .chars()
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

/// Upsert (non-empty value) or remove (empty value) a credential, preserving all
/// other lines and comments. Writes the document owner-only (0600) under a 0700 home.
#[tauri::command]
pub fn set_credential(key: String, value: String) -> Result<(), String> {
    let key = key.trim().to_string();
    if !is_posix_identifier(&key) {
        return Err(format!("凭据名必须是 POSIX 标识符: {key}"));
    }
    let value = value.trim().to_string();

    let home = dsh_home()?;
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

/// Profile names under `$DSH_HOME/profiles` (excludes `node_modules`).
#[tauri::command]
pub fn list_dsh_profiles() -> Result<Vec<String>, String> {
    let dir = dsh_home()?.join("profiles");
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut out = vec![];
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
    Ok(out)
}

fn profile_dir(profile: &str) -> Result<PathBuf, String> {
    if profile.is_empty()
        || profile.contains('/')
        || profile.contains('\\')
        || profile.contains("..")
    {
        return Err(format!("非法 profile 名: {profile}"));
    }
    Ok(dsh_home()?.join("profiles").join(profile))
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
