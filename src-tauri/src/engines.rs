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

/// Executable suffixes tried on Windows, in this order, before the bare name.
///
/// Deliberately *not* read from `PATHEXT`: that variable answers "what would a
/// shell run", while the question here is "what can the launcher spawn". Rust's
/// `Command` reaches `.exe`/`.com` through CreateProcess and `.cmd`/`.bat`
/// through cmd.exe — but nothing can spawn the `.ps1` that npm writes next to
/// every `.cmd`, so accepting it would only trade a false "not installed" for a
/// launch that fails later.
const WIN_EXTS: [&str; 4] = [".exe", ".cmd", ".bat", ".com"];

/// The file names to try for `bin` inside one PATH directory.
///
/// On Windows an agent CLI installed by npm/pnpm lands as `claude.cmd`, never as
/// a bare `claude`, so looking up the bare name alone finds nothing — which is
/// why every engine used to report "not installed" there. The bare name is still
/// tried, but *last*, so a spawnable shim always wins over the extension-less sh
/// script npm drops beside it for Git Bash.
fn win_name_candidates(bin: &str) -> Vec<String> {
    let lower = bin.to_ascii_lowercase();
    if WIN_EXTS.iter().any(|e| lower.ends_with(e)) {
        // Already explicit — a `custom_bin` pointing straight at the executable.
        return vec![bin.to_string()];
    }
    let mut out: Vec<String> = WIN_EXTS.iter().map(|e| format!("{bin}{e}")).collect();
    out.push(bin.to_string());
    out
}

/// Find `bin` on a colon/semicolon-separated PATH, returning the first match.
///
/// Directory-major, suffix-minor: every candidate name is tried in one directory
/// before moving to the next, the same precedence `cmd.exe` gives PATHEXT.
///
/// `pub(crate)` because terminal discovery ([`crate::runtime::term`]) must follow
/// exactly this rule — PATH lookup, no disk scan, never executing the candidate.
/// One implementation, so the two can't drift apart.
pub(crate) fn find_on_path(bin: &str, path_var: &str) -> Option<String> {
    let sep = if cfg!(windows) { ';' } else { ':' };
    let names = if cfg!(windows) {
        win_name_candidates(bin)
    } else {
        vec![bin.to_string()]
    };
    for dir in path_var.split(sep) {
        // Windows PATH entries are sometimes quoted; a literal quote is never
        // part of a directory name on either platform.
        let dir = dir.trim().trim_matches('"');
        if dir.is_empty() {
            continue;
        }
        for name in &names {
            let candidate = std::path::Path::new(dir).join(name);
            if candidate.is_file() {
                return Some(candidate.to_string_lossy().into_owned());
            }
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

    // Windows name expansion is unit-tested on every platform (the lookup itself
    // is `cfg!(windows)`-gated, so a Linux CI run can only reach it here).
    #[test]
    fn win_candidates_try_spawnable_shims_before_the_bare_name() {
        assert_eq!(
            win_name_candidates("claude"),
            [
                "claude.exe",
                "claude.cmd",
                "claude.bat",
                "claude.com",
                "claude"
            ]
        );
        // npm writes `claude.ps1` beside `claude.cmd`; nothing can spawn it, so
        // it must never be what detection reports.
        assert!(!win_name_candidates("claude")
            .iter()
            .any(|c| c.ends_with(".ps1")));
    }

    #[test]
    fn win_candidates_leave_an_explicit_executable_alone() {
        for bin in [r"C:\tools\dsh.exe", r"C:\tools\DSH.EXE", "pi.cmd"] {
            assert_eq!(win_name_candidates(bin), [bin.to_string()], "{bin}");
        }
    }

    #[test]
    fn find_on_path_ignores_quotes_and_padding_around_a_dir() {
        let dir = std::env::temp_dir().join(format!("agentlauncher-quoted-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let bin = dir.join("fake-engine");
        std::fs::write(&bin, b"#!/bin/sh\n").unwrap();
        let path_var = format!(" \"{}\" ", dir.display());
        assert_eq!(
            find_on_path("fake-engine", &path_var).as_deref(),
            Some(bin.to_string_lossy().as_ref())
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
