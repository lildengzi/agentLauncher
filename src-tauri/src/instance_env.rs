//! The instance's own `.env` — the one credential tier an instance owns itself.
//!
//! `instances/<id>/.env` is a plain `KEY=value` file that the executor layers **last**,
//! so anything written here is the most specific source and wins over the launcher's
//! shared key store. That is the whole reason this module exists as its own seam: the
//! 模型 page lets a user keep a key *for this instance only*, and "this instance only"
//! means this file.
//!
//! Invariants:
//!
//! * **A value never comes back out.** [`get_instance_key`] returns the variable name
//!   and a fingerprint (`sk-p…9f2a`), the same masking `providers.json` uses. The
//!   frontend can tell whether a key is there and which one it is; it cannot read it.
//! * **Other lines survive.** [`set_instance_key`] rewrites one entry and leaves
//!   comments, settings and unrelated variables exactly where they were.
//! * **0600.** The file holds a secret, so it is written owner-only like every other
//!   credential file the launcher touches.
//! * **One definition of "looks like a key".** [`looks_like_key_var`] is used both here
//!   and by `executor::resolve_credentials` when it asks "does this instance already
//!   bring its own key?" — two answers to that question would be one bug.

use serde::Serialize;

use crate::instance_manager::instance_dir;
use crate::providers::fingerprint;
use crate::runtime::dsh_home::is_posix_identifier;

/// Parse a minimal `.env` file into (key, value) pairs. Blank lines and `#` comments
/// are skipped; a surrounding pair of double quotes is stripped from the value.
pub fn parse(path: &std::path::Path) -> Vec<(String, String)> {
    let mut out = vec![];
    if let Ok(text) = std::fs::read_to_string(path) {
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                let v = v.trim().trim_matches('"');
                out.push((k.trim().to_string(), v.to_string()));
            }
        }
    }
    out
}

/// A variable name that carries a credential rather than a setting.
///
/// A heuristic on purpose. The file is hand-written as often as not, and every engine
/// names its key differently (`DEEPSEEK_API_KEY`, `ANTHROPIC_AUTH_TOKEN`,
/// `GEMINI_API_KEY`, `OPENAI_API_KEY`), so the launcher recognises the *shape* rather
/// than keeping a per-engine table it would have to keep guessing at.
pub fn looks_like_key_var(name: &str) -> bool {
    let n = name.trim().to_ascii_uppercase();
    n.ends_with("_KEY") || n.ends_with("_TOKEN") || n.contains("APIKEY")
}

/// The first key-shaped entry with a value, if this instance brings its own.
pub fn find_key(pairs: &[(String, String)]) -> Option<(&String, &String)> {
    pairs
        .iter()
        .find(|(k, v)| looks_like_key_var(k) && !v.trim().is_empty())
        .map(|(k, v)| (k, v))
}

/// What the 模型 page shows about this instance's own key: which variable holds it and
/// a fingerprint of the value — never the value.
#[derive(Debug, Clone, Serialize)]
pub struct InstanceKeyView {
    /// Empty when the instance keeps no key of its own.
    pub var: String,
    pub fingerprint: String,
    pub has_value: bool,
}

/// Whether this instance keeps a key of its own, and which variable it lands in.
#[tauri::command]
pub fn get_instance_key(id: String) -> Result<InstanceKeyView, String> {
    let pairs = parse(&instance_dir(&id)?.join(".env"));
    Ok(match find_key(&pairs) {
        Some((var, value)) => InstanceKeyView {
            var: var.clone(),
            fingerprint: fingerprint(value),
            has_value: true,
        },
        None => InstanceKeyView {
            var: String::new(),
            fingerprint: String::new(),
            has_value: false,
        },
    })
}

/// Upsert (non-empty value) or remove (empty value) one variable in this instance's
/// `.env`, preserving every other line and comment.
///
/// The `var` name is the caller's: which variable an engine reads its key from is the
/// engine's business, and the 模型 page derives it from the chosen provider row. It must
/// be a POSIX identifier, and the value may not contain a newline — a line break here
/// would silently become a second variable.
#[tauri::command]
pub fn set_instance_key(id: String, var: String, value: String) -> Result<(), String> {
    let var = var.trim().to_string();
    if !is_posix_identifier(&var) {
        return Err(format!("环境变量名必须是 POSIX 标识符: {var}"));
    }
    let value = value.trim().to_string();
    if value.contains('\n') || value.contains('\r') {
        return Err("密钥里不能有换行".into());
    }

    let dir = instance_dir(&id)?;
    if !dir.exists() {
        return Err(format!("实例不存在: {id}"));
    }
    let path = dir.join(".env");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();

    let mut out: Vec<String> = Vec::new();
    let mut replaced = false;
    for line in existing.lines() {
        let is_target = line
            .split_once('=')
            .map(|(k, _)| k.trim() == var && !line.trim_start().starts_with('#'))
            .unwrap_or(false);
        if is_target {
            if !value.is_empty() {
                out.push(format!("{var}={value}"));
            }
            replaced = true; // dropping the line handles the unset case
        } else {
            out.push(line.to_string());
        }
    }
    if !replaced && !value.is_empty() {
        out.push(format!("{var}={value}"));
    }
    let mut body = out.join("\n");
    if !body.is_empty() && !body.ends_with('\n') {
        body.push('\n');
    }

    std::fs::write(&path, &body).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{temp_tree, EnvGuard, HOME_LOCK};

    #[test]
    fn key_shaped_variable_names() {
        // The names the six engines actually read.
        for k in [
            "DEEPSEEK_API_KEY",
            "ANTHROPIC_API_KEY",
            "ANTHROPIC_AUTH_TOKEN",
            "GEMINI_API_KEY",
            "openai_api_key",
        ] {
            assert!(looks_like_key_var(k), "{k} 应被认成密钥");
        }
        // Settings that ride in the same file must not be mistaken for one — the whole
        // question this answers is "did the instance bring its own key?".
        for k in ["ANTHROPIC_BASE_URL", "OPENAI_BASE_URL", "PATH", "DSH_HOME"] {
            assert!(!looks_like_key_var(k), "{k} 不是密钥");
        }
    }

    #[test]
    fn a_key_is_written_read_back_masked_and_removed_without_touching_the_rest() {
        let _g = HOME_LOCK.lock().unwrap();
        let tree = temp_tree("inst-env");
        let _home = EnvGuard::set("HOME", tree.path());

        let dir = instance_dir("i1").unwrap();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(".env"),
            "# 手写的注释\nANTHROPIC_BASE_URL=https://example.test/v1\n",
        )
        .unwrap();

        set_instance_key(
            "i1".into(),
            "ANTHROPIC_API_KEY".into(),
            "sk-abcdefgh1234wxyz".into(),
        )
        .unwrap();
        let view = get_instance_key("i1".into()).unwrap();
        assert_eq!(view.var, "ANTHROPIC_API_KEY");
        assert!(view.has_value);
        // Masked, and the mask is not the value.
        assert_eq!(view.fingerprint, "sk-a…wxyz");
        assert!(!view.fingerprint.contains("efgh1234"));

        let text = std::fs::read_to_string(dir.join(".env")).unwrap();
        assert!(text.contains("# 手写的注释"), "注释必须留着: {text}");
        assert!(
            text.contains("ANTHROPIC_BASE_URL=https://example.test/v1"),
            "{text}"
        );

        // Rewriting replaces the one line rather than appending a second.
        set_instance_key(
            "i1".into(),
            "ANTHROPIC_API_KEY".into(),
            "sk-zzzzzzzz9999yyyy".into(),
        )
        .unwrap();
        let text = std::fs::read_to_string(dir.join(".env")).unwrap();
        assert_eq!(text.matches("ANTHROPIC_API_KEY=").count(), 1, "{text}");

        // An empty value is "remove it", and what is left is the file minus that line.
        set_instance_key("i1".into(), "ANTHROPIC_API_KEY".into(), String::new()).unwrap();
        assert!(!get_instance_key("i1".into()).unwrap().has_value);
        let text = std::fs::read_to_string(dir.join(".env")).unwrap();
        assert!(!text.contains("ANTHROPIC_API_KEY"), "{text}");
        assert!(text.contains("ANTHROPIC_BASE_URL"), "{text}");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.join(".env"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600, "实例 .env 里有密钥，不能让别人读");
        }
    }

    #[test]
    fn a_bad_variable_name_or_a_multiline_value_is_refused() {
        let _g = HOME_LOCK.lock().unwrap();
        let tree = temp_tree("inst-env-bad");
        let _home = EnvGuard::set("HOME", tree.path());
        let dir = instance_dir("i1").unwrap();
        std::fs::create_dir_all(&dir).unwrap();

        assert!(set_instance_key("i1".into(), "not a name".into(), "x".into()).is_err());
        // A newline would quietly become a second variable in the child's environment.
        assert!(set_instance_key("i1".into(), "K_API_KEY".into(), "a\nB=c".into()).is_err());
        assert!(set_instance_key("ghost".into(), "K_API_KEY".into(), "sk-1".into()).is_err());
    }
}
