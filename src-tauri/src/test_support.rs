//! Shared test scaffolding (compiled only under `cfg(test)`).
//!
//! Several suites need to point the launcher at a throwaway home directory, which
//! means mutating process-global env vars. Those locks live here rather than in
//! each test module: two modules with their own private `HOME_LOCK` would not
//! actually exclude each other, so a temp HOME from one test could leak into
//! another running in parallel. One lock per env var, crate-wide.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Serializes every test that mutates `HOME`.
pub(crate) static HOME_LOCK: Mutex<()> = Mutex::new(());

/// Serializes every test that mutates `DSH_HOME` (read by dsh_config when
/// deciding whether a profile is web-capable).
pub(crate) static DSH_HOME_LOCK: Mutex<()> = Mutex::new(());

/// Sets an env var for as long as the guard lives, restoring the previous value
/// (or its absence) on drop — so a panicking test cannot leak it into the rest of
/// the run.
pub(crate) struct EnvGuard {
    key: &'static str,
    prev: Option<OsString>,
}

impl EnvGuard {
    pub(crate) fn set(key: &'static str, value: impl AsRef<OsStr>) -> Self {
        let prev = std::env::var_os(key);
        std::env::set_var(key, value);
        Self { key, prev }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match self.prev.take() {
            Some(v) => std::env::set_var(self.key, v),
            None => std::env::remove_var(self.key),
        }
    }
}

/// A temp directory removed when the guard drops.
pub(crate) struct TempTree(PathBuf);

impl TempTree {
    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

/// Create a uniquely named temp tree. The nanos + pid suffix keeps parallel test
/// binaries and repeated runs from colliding.
pub(crate) fn temp_tree(tag: &str) -> TempTree {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let p = std::env::temp_dir().join(format!(
        "agentlauncher-test-{tag}-{n}-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&p).unwrap();
    TempTree(p)
}
