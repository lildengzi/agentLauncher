//! Engine registry + live detection — agent-agnostic host capability.
//!
//! The launcher wraps several agent CLIs ("frameworks"); `known_engines` is the
//! static catalog and `detect_engines` probes which are actually installed on
//! the host right now. Detection is done fresh on demand and never cached to
//! disk: a persisted `engines.json` would go stale and point at a since-removed
//! binary — the same foot-gun the PATH resolver avoids (see runtime::env). The
//! UI uses the result to populate the engine picker and mark missing CLIs.

use serde::Serialize;

use crate::runtime::env;

/// A known agent CLI the launcher can drive.
pub struct EngineSpec {
    /// Stable id stored in `instance.runtime.engine` (matches AgentRuntime::id).
    pub id: &'static str,
    /// Human-readable name for the picker.
    pub display: &'static str,
    /// Default binary name looked up on PATH when `custom_bin` is empty.
    pub default_bin: &'static str,
    /// Whether this engine has a launcher-wired web/serve mode (only dsh today).
    pub web: bool,
    /// Whether the LLM provider reaches this engine as a launch argument. False
    /// for claude, whose provider / base URL / key come only from `ANTHROPIC_*`
    /// in the instance `.env` — so the UI hides the field rather than collecting
    /// a value the launcher would silently drop.
    pub takes_provider: bool,
}

/// The static catalog of engines the launcher knows how to assemble commands for.
pub fn known_engines() -> &'static [EngineSpec] {
    &[
        EngineSpec {
            id: "dsh",
            display: "dsh (DeepSeek Harness)",
            default_bin: "dsh",
            takes_provider: true,
            web: true,
        },
        EngineSpec {
            id: "pi",
            display: "pi (pi-coding-agent)",
            default_bin: "pi",
            takes_provider: true,
            web: false,
        },
        EngineSpec {
            id: "omp",
            display: "omp (oh-my-pi)",
            default_bin: "omp",
            takes_provider: true,
            web: false,
        },
        EngineSpec {
            id: "claude",
            display: "claude (Claude Code)",
            default_bin: "claude",
            takes_provider: false,
            web: false,
        },
        EngineSpec {
            id: "codex",
            display: "codex",
            default_bin: "codex",
            takes_provider: true,
            web: false,
        },
        EngineSpec {
            id: "opencode",
            display: "opencode",
            default_bin: "opencode",
            takes_provider: true,
            web: false,
        },
    ]
}

/// One engine's install status for the UI — mirrors `EngineInfo` in src/types.ts.
#[derive(Debug, Clone, Serialize)]
pub struct EngineInfo {
    pub id: String,
    pub display: String,
    pub web: bool,
    /// Mirrors `EngineSpec::takes_provider` — the UI hides the provider field for
    /// engines that would drop it.
    pub takes_provider: bool,
    pub installed: bool,
    /// Absolute path of the resolved binary, or empty when not found.
    pub path: String,
}

/// Find `bin` on a colon/semicolon-separated PATH, returning the first match.
fn find_on_path(bin: &str, path_var: &str) -> Option<String> {
    let sep = if cfg!(windows) { ';' } else { ':' };
    for dir in path_var.split(sep) {
        if dir.is_empty() {
            continue;
        }
        let candidate = std::path::Path::new(dir).join(bin);
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }
    None
}

/// Probe the host for each known engine's binary, using the same enriched PATH
/// the launcher would give a child (login-shell PATH ∪ process PATH). Fresh each
/// call — never cached.
#[tauri::command]
pub async fn detect_engines() -> Vec<EngineInfo> {
    // Reuse the autodetect PATH resolution so detection matches what a launched
    // child would actually see (fixes the GUI thin-PATH case).
    let path_var = env::resolve_child_path("autodetect", "")
        .await
        .unwrap_or_default();
    known_engines()
        .iter()
        .map(|e| {
            let found = find_on_path(e.default_bin, &path_var);
            EngineInfo {
                id: e.id.to_string(),
                display: e.display.to_string(),
                web: e.web,
                takes_provider: e.takes_provider,
                installed: found.is_some(),
                path: found.unwrap_or_default(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_all_six_and_only_dsh_is_web() {
        let ids: Vec<&str> = known_engines().iter().map(|e| e.id).collect();
        assert_eq!(ids, ["dsh", "pi", "omp", "claude", "codex", "opencode"]);
        for e in known_engines() {
            assert_eq!(e.web, e.id == "dsh", "only dsh should be web-capable");
            // claude is the one engine with no provider flag (ANTHROPIC_* env only);
            // `model_test::argv_matrix` is the other half of this claim — it asserts
            // no provider ever reaches claude's argv.
            assert_eq!(
                e.takes_provider,
                e.id != "claude",
                "{}: takes_provider disagrees with the argv matrix",
                e.id
            );
        }
    }

    #[test]
    fn find_on_path_locates_a_file() {
        let dir =
            std::env::temp_dir().join(format!("agentlauncher-engines-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let bin = dir.join("fake-engine");
        std::fs::write(&bin, b"#!/bin/sh\n").unwrap();
        let path_var = format!(
            "/nonexistent-xyz{}{}",
            if cfg!(windows) { ';' } else { ':' },
            dir.display()
        );
        assert_eq!(
            find_on_path("fake-engine", &path_var).as_deref(),
            Some(bin.to_string_lossy().as_ref())
        );
        assert_eq!(find_on_path("not-there", &path_var), None);
        std::fs::remove_dir_all(&dir).ok();
    }
}
