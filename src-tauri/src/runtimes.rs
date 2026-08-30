//! Launcher-managed agent CLIs — `~/.agentlauncher/runtimes/`.
//!
//! Prism Launcher does not ask you to install Java; it keeps its own. Same move
//! here: one-click install puts an agent CLI in a launcher-private prefix, not in
//! the user's global npm root and not inside any instance. Everything good about
//! it follows from that one choice — no administrator rights (the blocker on
//! Windows), no edit to the user's PATH, no global pollution, uninstall is one
//! `rm -r`, and the CLI is usable *without restarting the launcher*, because the
//! launcher composes the child PATH itself (see [`crate::runtime::env`]).
//!
//! Installing fetches third-party code and then runs it, so it is never
//! automatic: nothing in this module runs at startup, it happens only on an
//! explicit press, and the exact command is echoed into the log before it runs.

use serde::Serialize;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::OnceLock;

use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Mutex;

use crate::engines::{self, Install};
use crate::launcher_config;
use crate::runtime::env;

/// `~/.agentlauncher/runtimes` — the npm prefix for every launcher-installed CLI.
pub fn root() -> Option<PathBuf> {
    launcher_config::agentlauncher_root()
        .ok()
        .map(|r| r.join("runtimes"))
}

/// Where npm puts executables for a non-global `--prefix` install:
/// `<root>/node_modules/.bin`, identically on all three platforms. A global
/// (`-g --prefix`) install would land them in `<root>/bin` on unix but directly in
/// `<root>` on Windows, so the local layout is the portable one.
///
/// Returned as a plain string for PATH composition, and deliberately *not*
/// created here — this is called on every launch, and a PATH entry that does not
/// exist yet is harmless.
pub fn bin_dir() -> Option<String> {
    Some(
        root()?
            .join("node_modules")
            .join(".bin")
            .to_string_lossy()
            .into_owned(),
    )
}

/// Create the prefix (0700) and seed a private `package.json`.
///
/// npm walks *upward* from its prefix looking for a manifest to treat as the
/// project root; without one of ours it could adopt an unrelated ancestor's.
/// `private` also means npm will never offer to publish this directory.
fn ensure_prefix() -> Result<PathBuf, String> {
    let dir = root().ok_or("cannot resolve home directory")?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    }
    let manifest = dir.join("package.json");
    if !manifest.exists() {
        std::fs::write(
            &manifest,
            "{\n  \"name\": \"agentlauncher-runtimes\",\n  \"private\": true\n}\n",
        )
        .map_err(|e| format!("{}: {e}", manifest.display()))?;
    }
    Ok(dir)
}

/// What the install dialog needs before it may offer a button — mirrors
/// `RuntimesStatus` in src/types.ts.
#[derive(Debug, Clone, Serialize)]
pub struct RuntimesStatus {
    /// Absolute path of the prefix, so the user can see where things will go.
    pub dir: String,
    /// Resolved `npm`, or empty. The one hard prerequisite for every recipe.
    pub npm: String,
    /// Resolved `node`, or empty. Reported separately because "npm is missing"
    /// and "node is missing" send the user to the same download but read very
    /// differently when only one of them is true.
    pub node: String,
}

/// Probe for the toolchain an install needs. Read-only: creates nothing.
#[tauri::command]
pub async fn runtimes_status() -> RuntimesStatus {
    let path_var = env::resolve_child_path("autodetect", "")
        .await
        .unwrap_or_default();
    RuntimesStatus {
        dir: root().map(|p| p.display().to_string()).unwrap_or_default(),
        npm: engines::find_on_path("npm", &path_var).unwrap_or_default(),
        node: engines::find_on_path("node", &path_var).unwrap_or_default(),
    }
}

/// One line of installer output. `stream` is "stdout" | "stderr" | "cmd", the
/// last being the command itself, echoed before it runs.
#[derive(Clone, Serialize)]
struct InstallLog {
    engine: String,
    stream: String,
    chunk: String,
}

/// How an install turned out, once the child has exited.
#[derive(Clone, Serialize)]
struct InstallDone {
    engine: String,
    ok: bool,
    message: String,
    /// The re-probed binary path on success — proof it is really there now.
    path: String,
}

/// One install at a time: they share a single npm prefix, and two npm processes
/// writing one `node_modules` is how you corrupt it.
fn install_lock() -> &'static Mutex<()> {
    static L: OnceLock<Mutex<()>> = OnceLock::new();
    L.get_or_init(Mutex::default)
}

fn log(app: &AppHandle, engine: &str, stream: &str, chunk: String) {
    let _ = app.emit(
        "install-log",
        InstallLog {
            engine: engine.to_string(),
            stream: stream.to_string(),
            chunk,
        },
    );
}

/// Forward one of the child's streams to the frontend, line by line.
fn pump<R>(
    app: AppHandle,
    engine: String,
    stream: &'static str,
    reader: R,
) -> tokio::task::JoinHandle<()>
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            log(&app, &engine, stream, format!("{line}\n"));
        }
    })
}

/// Raise npm's V8 heap unless the user has an opinion of their own.
///
/// Measured, not precautionary: resolving `@deepseek-ai/dsh@latest` with node's
/// default limit (2144 MB here) dies with `Ineffective mark-compacts near heap
/// limit` before it downloads anything. dsh depends on ~40 sibling packages, each
/// pinned with a caret against a *prerelease* version, and the candidate space
/// that produces is what exhausts the resolver — nothing about it gets smaller
/// with flags, so headroom is the only lever.
///
/// A `--max-old-space-size` already in `NODE_OPTIONS` is left alone: a user who
/// set one is more likely to be working around a small machine than to want ours.
/// The value is a ceiling, not a reservation, so raising it costs nothing when the
/// resolve happens to be small.
fn node_options() -> String {
    let existing = std::env::var("NODE_OPTIONS").unwrap_or_default();
    if existing.contains("max-old-space-size") {
        return existing;
    }
    let ours = "--max-old-space-size=4096";
    if existing.trim().is_empty() {
        ours.to_string()
    } else {
        format!("{} {ours}", existing.trim())
    }
}

/// Install one engine into the runtimes prefix with npm.
///
/// `@latest` is deliberate: these CLIs ship weekly, so a pinned version would
/// make "one-click" mean "install something stale" within a month.
///
/// What npm's integrity actually buys, stated straight: the SRI hash and the
/// registry signature both arrive in the same packument, over the same TLS, from
/// the same host as the tarball. They catch a corrupted download and a tarball
/// swapped underneath a published version — not a compromised registry and not a
/// compromised publisher account. Only sigstore provenance ties a tarball back to
/// a build in a public repo, and as of writing just two of the five (`codex`,
/// `pi`) publish one, which plain `npm install` does not check anyway. So npm is
/// the recipe because it needs no privileges, lands inside our own prefix, and is
/// *one* mechanism for five engines — not because its supply chain is stronger
/// than a vendor's own signed release would be.
///
/// `Err` means the install could not be *started* (unknown engine, no automated
/// source, no npm, one already running) and nothing was emitted. Once it starts,
/// the outcome arrives as an `install-done` event instead, so a failing npm
/// reports through the same channel as its own output.
#[tauri::command]
pub async fn install_engine(app: AppHandle, id: String) -> Result<(), String> {
    let spec = engines::known_engines()
        .iter()
        .find(|e| e.id == id)
        .ok_or_else(|| format!("未知引擎: {id}"))?;
    let pkg = match spec.install {
        Install::Npm(p) => p,
        Install::Manual => {
            return Err(format!(
                "{} 没有可信的自动安装来源，请照文档手动安装",
                spec.display
            ))
        }
    };
    let _guard = install_lock()
        .try_lock()
        .map_err(|_| "已有一个安装正在进行，等它结束再来".to_string())?;

    let dir = ensure_prefix()?;
    let path_var = env::resolve_child_path("autodetect", "")
        .await
        .unwrap_or_default();
    let npm = engines::find_on_path("npm", &path_var)
        .ok_or("PATH 上找不到 npm —— 先安装 Node（≥ 22），再回到这里")?;

    let target = format!("{pkg}@latest");
    let dir_arg = dir.display().to_string();
    let args = [
        "install",
        "--prefix",
        &dir_arg,
        "--no-audit",
        "--no-fund",
        // Every registry fetch prints a line, so a 40 MB package is not four
        // silent minutes.
        "--loglevel=http",
        &target,
    ];
    // Echoed before anything runs, and echoed as *actually invoked* — heap override
    // included, so the logged line is one a user could rerun by hand.
    let node_opts = node_options();
    log(
        &app,
        &id,
        "cmd",
        format!(
            "$ NODE_OPTIONS=\"{node_opts}\" {} {}\n",
            npm,
            args.join(" ")
        ),
    );

    let mut child = tokio::process::Command::new(&npm)
        .args(args)
        .env("PATH", &path_var)
        .env("NODE_OPTIONS", &node_opts)
        .current_dir(&dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("无法执行 npm: {e}"))?;

    let mut pumps = Vec::new();
    if let Some(out) = child.stdout.take() {
        pumps.push(pump(app.clone(), id.clone(), "stdout", out));
    }
    if let Some(err) = child.stderr.take() {
        pumps.push(pump(app.clone(), id.clone(), "stderr", err));
    }
    let status = child.wait().await;
    for p in pumps {
        let _ = p.await;
    }

    let done = match status {
        Ok(s) if s.success() => match engines::probe_one(&id).await {
            Some(info) if info.installed => InstallDone {
                engine: id.clone(),
                ok: true,
                message: format!("已安装到 {}", dir.display()),
                path: info.path,
            },
            // npm succeeded but the expected bin is not there: the package's bin
            // name is not what the catalog assumes. Say exactly that instead of a
            // green check the user cannot act on.
            _ => InstallDone {
                engine: id.clone(),
                ok: false,
                message: format!(
                    "npm 装完了，但 {} 下没有出现 {}",
                    dir.display(),
                    spec.default_bin
                ),
                path: String::new(),
            },
        },
        Ok(s) => InstallDone {
            engine: id.clone(),
            ok: false,
            message: format!("npm 退出码 {}", s.code().unwrap_or(-1)),
            path: String::new(),
        },
        Err(e) => InstallDone {
            engine: id.clone(),
            ok: false,
            message: format!("等待 npm 结束时出错: {e}"),
            path: String::new(),
        },
    };
    let _ = app.emit("install-done", done);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{temp_tree, EnvGuard, HOME_LOCK};

    /// The prefix is owner-only and carries a manifest of its own, so npm never
    /// walks up into an unrelated ancestor's `package.json`, and `bin_dir` points
    /// at the local (non-global) layout npm actually writes.
    #[test]
    fn the_prefix_is_owner_only_and_seeded() {
        let _lock = HOME_LOCK.lock().unwrap();
        let home = temp_tree("runtimes");
        let _guard = EnvGuard::set("HOME", home.path());

        let dir = ensure_prefix().unwrap();
        assert_eq!(dir, home.path().join(".agentlauncher").join("runtimes"));
        let manifest = std::fs::read_to_string(dir.join("package.json")).unwrap();
        assert!(manifest.contains("\"private\": true"), "{manifest}");
        assert!(bin_dir().unwrap().ends_with("runtimes/node_modules/.bin"));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&dir).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o700, "prefix must be owner-only");
        }

        // Idempotent, and a hand-edited manifest is left alone.
        std::fs::write(dir.join("package.json"), "{\"mine\": true}").unwrap();
        ensure_prefix().unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("package.json")).unwrap(),
            "{\"mine\": true}"
        );
    }

    /// The heap bump defers to a user who already set one, and never mangles the
    /// rest of their `NODE_OPTIONS`.
    #[test]
    fn node_options_defers_to_an_existing_limit() {
        static NODE_OPTIONS_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _lock = NODE_OPTIONS_LOCK.lock().unwrap();

        let _g = EnvGuard::set("NODE_OPTIONS", "");
        assert_eq!(node_options(), "--max-old-space-size=4096");

        let _g = EnvGuard::set("NODE_OPTIONS", "--enable-source-maps");
        assert_eq!(
            node_options(),
            "--enable-source-maps --max-old-space-size=4096"
        );

        let _g = EnvGuard::set("NODE_OPTIONS", "--max-old-space-size=512");
        assert_eq!(node_options(), "--max-old-space-size=512");
    }
}
