//! Child-process PATH resolution — agent-agnostic host adaptation.
//!
//! A launcher started from a desktop environment (icon / dock) inherits a
//! *stripped* PATH, so an agent CLI installed under nvm / `~/.local/bin` /
//! Homebrew resolves in the user's terminal but not from the GUI. This module
//! computes the PATH the executor sets on the child before spawning, per the
//! instance's `runtime.env_policy`:
//!   * `autodetect` — enrich with the login shell's PATH (probed fresh each
//!     launch, never cached to disk) so the child sees the same toolchain the
//!     terminal does.
//!   * `isolated`   — a minimal, deterministic PATH that does not leak the
//!     host toolchain; only system dirs plus any `custom_bin` directory.
//!
//! A non-empty `custom_bin` always contributes its own directory to the front,
//! followed by the launcher's own runtimes bin dir ([`crate::runtimes`]) — in
//! *both* policies, because a CLI the launcher installed is not host toolchain
//! leakage; it is the launcher's own equipment, as deterministic as `/usr/bin`.
//! Prepending it here is also what makes a freshly installed CLI launchable with
//! no restart and no edit to the user's PATH.

use std::path::Path;
use std::time::Duration;

/// Platform PATH separator.
fn sep() -> char {
    if cfg!(windows) {
        ';'
    } else {
        ':'
    }
}

/// The launcher process's own PATH (what a naive spawn would inherit).
fn process_path() -> String {
    std::env::var("PATH").unwrap_or_default()
}

/// Minimal deterministic system PATH for `isolated`.
fn system_path() -> String {
    if cfg!(windows) {
        // Reasonable Windows baseline; the process PATH's system entries usually
        // already cover this, but be explicit for reproducibility.
        std::env::var("PATH").unwrap_or_default()
    } else {
        "/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin".to_string()
    }
}

/// The directory holding `custom_bin`, if it has one.
fn custom_bin_dir(custom_bin: &str) -> Option<String> {
    let b = custom_bin.trim();
    if b.is_empty() {
        return None;
    }
    Path::new(b)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| p.to_string_lossy().into_owned())
}

/// Join path fragments (each may itself be a separator-joined list), dropping
/// empties and duplicates while preserving first-seen order.
fn join_dedup(fragments: &[&str]) -> String {
    let s = sep();
    let mut seen: Vec<String> = Vec::new();
    for frag in fragments {
        for part in frag.split(s) {
            let part = part.trim();
            if part.is_empty() || seen.iter().any(|e| e == part) {
                continue;
            }
            seen.push(part.to_string());
        }
    }
    seen.join(&s.to_string())
}

/// Probe the user's login shell for its exported PATH. `printenv PATH` yields
/// the colon-joined value identically across bash/zsh/fish (it reads the
/// exported env var, not shell-specific list syntax). Non-interactive (`-c`)
/// with a timeout so a slow rc file can never hang a launch.
#[cfg(unix)]
async fn login_shell_path() -> Option<String> {
    let shell = std::env::var("SHELL")
        .ok()
        .filter(|s| !s.trim().is_empty())?;
    let mut cmd = tokio::process::Command::new(&shell);
    cmd.arg("-l")
        .arg("-c")
        .arg("printenv PATH")
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    let fut = cmd.output();
    let out = tokio::time::timeout(Duration::from_secs(3), fut)
        .await
        .ok()?
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if path.is_empty() {
        None
    } else {
        Some(path)
    }
}

#[cfg(not(unix))]
async fn login_shell_path() -> Option<String> {
    None
}

/// Resolve the PATH to set on the child process for this instance, or `None`
/// to leave the inherited PATH untouched (only when policy is unrecognized).
///
/// Precedence is custom_bin > launcher-managed > host. An explicit `custom_bin`
/// is the user naming a binary, so nothing may shadow it; the managed dir comes
/// next so one-click install wins over a stale copy elsewhere on the host, which
/// is the only ordering where "the launcher installed it for you" is a true
/// statement about what then runs.
pub async fn resolve_child_path(env_policy: &str, custom_bin: &str) -> Option<String> {
    let dir = custom_bin_dir(custom_bin);
    let dir_ref = dir.as_deref().unwrap_or("");
    let managed = crate::runtimes::bin_dir().unwrap_or_default();
    match env_policy {
        "isolated" => Some(join_dedup(&[dir_ref, &managed, &system_path()])),
        // "autodetect" (and any unknown value) enrich the host PATH; treating an
        // unknown policy as autodetect is the safe, launch-succeeds default.
        _ => {
            let shell = login_shell_path().await.unwrap_or_default();
            Some(join_dedup(&[dir_ref, &managed, &shell, &process_path()]))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_dedup_preserves_order_and_drops_dupes() {
        assert_eq!(join_dedup(&["/a:/b", "/b:/c", ""]), "/a:/b:/c");
        assert_eq!(join_dedup(&["", "/x"]), "/x");
    }

    #[test]
    fn isolated_includes_custom_bin_dir_and_system_only() {
        let p = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(resolve_child_path("isolated", "/opt/tools/dsh/bin/dsh"))
            .unwrap();
        let entries: Vec<&str> = p.split(':').collect();
        assert!(
            entries.contains(&"/opt/tools/dsh/bin"),
            "custom_bin dir present: {p}"
        );
        assert!(entries.contains(&"/usr/bin"), "system dir present: {p}");
        // isolated must not leak an arbitrary host-only dir.
        assert!(
            !entries.contains(&"/home/someone/.nvm/x/bin"),
            "no host leak: {p}"
        );
    }

    #[test]
    fn custom_bin_dir_of_empty_is_none() {
        assert_eq!(custom_bin_dir(""), None);
        assert_eq!(custom_bin_dir("   "), None);
    }

    /// The launcher's own runtimes bin dir is on the child PATH under *both*
    /// policies, after an explicit `custom_bin` and before anything from the host.
    /// This ordering is the whole reason one-click install needs no restart and no
    /// edit to the user's PATH.
    #[test]
    fn managed_runtimes_dir_sits_between_custom_bin_and_the_host() {
        let _lock = crate::test_support::HOME_LOCK.lock().unwrap();
        let home = crate::test_support::temp_tree("env-managed");
        let _guard = crate::test_support::EnvGuard::set("HOME", home.path());
        let managed = crate::runtimes::bin_dir().unwrap();

        for policy in ["isolated", "autodetect"] {
            let p = tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(resolve_child_path(policy, "/opt/tools/dsh/bin/dsh"))
                .unwrap();
            let entries: Vec<&str> = p.split(':').collect();
            let at = |d: &str| entries.iter().position(|e| *e == d);
            let (custom, mgd) = (at("/opt/tools/dsh/bin"), at(&managed));
            assert!(mgd.is_some(), "{policy}: managed dir missing from {p}");
            assert!(custom < mgd, "{policy}: custom_bin must win: {p}");
            if let Some(sys) = at("/usr/bin") {
                assert!(mgd < Some(sys), "{policy}: managed before host: {p}");
            }
        }
    }
}
