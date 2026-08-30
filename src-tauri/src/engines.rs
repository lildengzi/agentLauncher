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

/// How the launcher can install an engine into its own runtimes dir (see
/// [`crate::runtimes`]).
///
/// A recipe, not a command string, because the install methods are genuinely
/// heterogeneous: on one Arch box the six engines came from five different
/// sources. Every `Npm` package name below was verified against the running
/// binary's version, not guessed — the obvious short names on npm (`pi`, `omp`,
/// `pi-coding-agent`, `oh-my-pi`) are *other people's* packages, stale by whole
/// major versions, and installing one of those would be a supply-chain hazard
/// dressed up as a convenience.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Install {
    /// One npm package whose published `bin` name equals `default_bin`.
    Npm(&'static str),
    /// No automated path we can vouch for — the UI offers the docs link and a
    /// command to copy instead of a button that must fail.
    Manual,
}

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
    /// How one-click install gets this engine, if it can.
    pub install: Install,
    /// Where a user goes to install or read about it by hand. Shown for every
    /// engine, and the only thing offered for `Install::Manual`.
    pub docs: &'static str,
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
            install: Install::Npm("@deepseek-ai/dsh"),
            docs: "https://www.npmjs.com/package/@deepseek-ai/dsh",
        },
        EngineSpec {
            id: "pi",
            display: "pi (pi-coding-agent)",
            default_bin: "pi",
            takes_provider: true,
            web: false,
            // Not `pi-coding-agent` (a stranger's 0.0.1). The real name comes from
            // the installer manifest pi ships in its own GitHub release.
            install: Install::Npm("@earendil-works/pi-coding-agent"),
            docs: "https://github.com/earendil-works/pi",
        },
        EngineSpec {
            id: "omp",
            display: "omp (oh-my-pi)",
            default_bin: "omp",
            takes_provider: true,
            web: false,
            // Built from source by its packagers; no npm artifact we can identify.
            install: Install::Manual,
            docs: "https://omp.sh/",
        },
        EngineSpec {
            id: "claude",
            display: "claude (Claude Code)",
            default_bin: "claude",
            takes_provider: false,
            web: false,
            install: Install::Npm("@anthropic-ai/claude-code"),
            docs: "https://github.com/anthropics/claude-code",
        },
        EngineSpec {
            id: "codex",
            display: "codex",
            default_bin: "codex",
            takes_provider: true,
            web: false,
            install: Install::Npm("@openai/codex"),
            docs: "https://github.com/openai/codex",
        },
        EngineSpec {
            id: "opencode",
            display: "opencode",
            default_bin: "opencode",
            takes_provider: true,
            web: false,
            install: Install::Npm("opencode-ai"),
            docs: "https://github.com/anomalyco/opencode",
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
    /// `"npm"` or `"manual"` — which install affordance the UI may offer.
    pub install: String,
    /// The npm package name for `"npm"`, empty for `"manual"`. Shown verbatim in
    /// the install dialog, because the user is entitled to see what would be
    /// fetched before agreeing to fetch it.
    pub package: String,
    pub docs: String,
    /// Whether the resolved binary is the copy the launcher installed, rather than
    /// one the user already had on PATH.
    pub managed: bool,
}

impl EngineInfo {
    /// Present one spec together with the result of probing for its binary.
    fn from_probe(e: &EngineSpec, found: Option<String>, managed_prefix: Option<&str>) -> Self {
        let (install, package) = match e.install {
            Install::Npm(pkg) => ("npm", pkg),
            Install::Manual => ("manual", ""),
        };
        let managed = match (&found, managed_prefix) {
            (Some(p), Some(prefix)) => p.starts_with(prefix),
            _ => false,
        };
        EngineInfo {
            id: e.id.to_string(),
            display: e.display.to_string(),
            web: e.web,
            takes_provider: e.takes_provider,
            installed: found.is_some(),
            path: found.unwrap_or_default(),
            install: install.to_string(),
            package: package.to_string(),
            docs: e.docs.to_string(),
            managed,
        }
    }
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
/// the launcher would give a child (login-shell PATH ∪ process PATH, with the
/// launcher's own runtimes dir in front). Fresh each call — never cached.
#[tauri::command]
pub async fn detect_engines() -> Vec<EngineInfo> {
    // Reuse the autodetect PATH resolution so detection matches what a launched
    // child would actually see (fixes the GUI thin-PATH case, and means a CLI the
    // launcher just installed is found without a restart).
    let path_var = env::resolve_child_path("autodetect", "")
        .await
        .unwrap_or_default();
    let managed = crate::runtimes::bin_dir();
    known_engines()
        .iter()
        .map(|e| {
            let found = find_on_path(e.default_bin, &path_var);
            EngineInfo::from_probe(e, found, managed.as_deref())
        })
        .collect()
}

/// Re-probe one engine, for the installer to report what it actually produced.
/// Same PATH rules as [`detect_engines`]; `None` for an unknown id.
pub(crate) async fn probe_one(id: &str) -> Option<EngineInfo> {
    let spec = known_engines().iter().find(|e| e.id == id)?;
    let path_var = env::resolve_child_path("autodetect", "")
        .await
        .unwrap_or_default();
    let found = find_on_path(spec.default_bin, &path_var);
    Some(EngineInfo::from_probe(
        spec,
        found,
        crate::runtimes::bin_dir().as_deref(),
    ))
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

    /// Every install recipe must be actionable: an `Npm` row needs a package name
    /// for the dialog to show before the user agrees to fetch it, and every row
    /// needs a docs URL, because `Manual` has nothing else to offer.
    #[test]
    fn every_recipe_is_actionable() {
        for e in known_engines() {
            assert!(e.docs.starts_with("https://"), "{}: docs url", e.id);
            if let Install::Npm(pkg) = e.install {
                assert!(!pkg.is_empty(), "{}: empty package name", e.id);
                // The short, obvious names on npm are other people's packages,
                // stale by whole major versions; installing one would be a
                // supply-chain hazard dressed up as a convenience. Every name in
                // the catalog was matched against the running binary's version.
                assert_ne!(pkg, e.default_bin, "{}: suspiciously short name", e.id);
            }
        }
        // omp is the one engine built from git source by its packagers.
        let manual: Vec<&str> = known_engines()
            .iter()
            .filter(|e| e.install == Install::Manual)
            .map(|e| e.id)
            .collect();
        assert_eq!(manual, ["omp"]);
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
