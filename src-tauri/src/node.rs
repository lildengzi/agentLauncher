//! Launcher-managed Node — `~/.agentlauncher/runtimes/node/`.
//!
//! Prism Launcher does not ask you to install Java; it downloads one. Node is this
//! launcher's Java: every automated install recipe we have is npm
//! ([`crate::runtimes`]), MCP servers and dsh plugins are npm packages too, so
//! "no Node" is not one engine's missing dependency — it is the whole feature
//! being unreachable. On a fresh machine the symptom was six grey engine rows and
//! five disabled install buttons, with nothing on screen explaining why.
//!
//! So this module downloads an official Node build into the launcher's own prefix.
//! Everything good about that comes from the prefix being private: no
//! administrator rights (the blocker on Windows), no edit to the user's PATH, no
//! global npm root touched, uninstall is one `rm -r`, and the child PATH is
//! composed by us ([`crate::runtime::env`]) so a Node installed here is usable
//! with no restart.
//!
//! Downloading and then *running* a 58 MB binary is the most consequential thing
//! the launcher does on the user's behalf, so, in order:
//!
//!   * nothing here runs at startup — an install needs an explicit press, or the
//!     `auto_download` checkbox plus a press on an engine that cannot work
//!     without it;
//!   * every URL is echoed into the log before it is fetched, the same rule the
//!     npm recipe follows for its command line;
//!   * the host is a compile-time constant. The dist index supplies a version
//!     string and nothing else — no host, no path, no file name — and even that
//!     string is validated against `v<digits>.<digits>.<digits>` before it is
//!     interpolated into a URL;
//!   * the archive is checked against the sha256 the same host publishes. Read
//!     [`verify`] for what that does and does not buy; it is less than it looks.
//!
//! Nothing is written where a half-finished install could be mistaken for a
//! finished one: the archive lands under a `.part` name, extraction goes to a
//! `.node-staging-<pid>/` directory, and `node/` appears by rename once
//! `bin/node` has been seen inside it.

use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::download;
use crate::launcher_config::{self, NodeSettings};
use crate::market::http;

/// Where official builds come from. A constant, compared literally, never
/// assembled from anything a response said — the same rule
/// `market::http::github_readme_url` follows for GitHub.
const DIST_HOST: &str = "nodejs.org";
const DIST_BASE: &str = "https://nodejs.org/dist";

/// The dist index is 330 KB today and grows by a few hundred bytes per release.
const MAX_INDEX_BYTES: usize = 2 * 1024 * 1024;
/// `SHASUMS256.txt` is a few KB; this is only here so a redirect to something
/// enormous is refused rather than buffered.
const MAX_SHASUMS_BYTES: usize = 256 * 1024;
/// A Node tarball is ~58 MB; the Windows zip and the darwin builds are smaller.
const MAX_ARCHIVE_BYTES: u64 = 128 * 1024 * 1024;

/// The minimum Node the launcher will accept without complaint, and the version
/// component of every "your Node is too old" message.
///
/// It is the highest floor any bundled engine declares, read from the registry
/// rather than assumed: `pi` needs `>=22.19.0`, `claude` `>=22.0.0`, `codex`
/// `>=16`, and `opencode`/`dsh` declare none. So it is deliberately too strict for
/// most of them — which is exactly why `skip_version_check` exists.
pub const FLOOR: Version = Version {
    major: 22,
    minor: 19,
    patch: 0,
};

// ---- version --------------------------------------------------------------

/// A three-part Node version, ordered numerically.
///
/// Three `u32`s rather than a semver crate: Node's own versions are always
/// `major.minor.patch` with no prerelease tags, and the one thing that has to be
/// right — `22.19.0` is newer than `22.9.0`, which string comparison gets
/// backwards — is what the derived `Ord` gives us for free.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Parse `v24.20.0`, `24.20.0`, or either with trailing whitespace — the three
/// shapes this actually sees: the dist index, our own `.version` file, and the
/// output of `node --version`.
///
/// Strict about the rest, because the same function guards what gets interpolated
/// into a download URL: exactly three parts, digits only, nothing trailing.
pub fn parse_version(raw: &str) -> Option<Version> {
    let s = raw.trim().trim_start_matches('v');
    let mut it = s.split('.');
    let mut next = || -> Option<u32> {
        let part = it.next()?;
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        part.parse().ok()
    };
    let v = Version {
        major: next()?,
        minor: next()?,
        patch: next()?,
    };
    if it.next().is_some() {
        return None;
    }
    Some(v)
}

// ---- layout ---------------------------------------------------------------

/// `~/.agentlauncher/runtimes/node` — the whole extracted tree.
///
/// Inside the npm prefix rather than beside it: that directory is already 0700,
/// uninstall is still one `rm -r`, and npm cannot reach it — our seeded
/// `package.json` declares no `workspaces`, and `npm install --prefix` writes
/// `node_modules/` and nothing else.
pub fn dir() -> Option<PathBuf> {
    crate::runtimes::root().map(|r| r.join("node"))
}

/// The directory holding `node`/`npm`, as a real path.
///
/// Unix archives put them in `bin/`; the Windows zip puts `node.exe` and `npm.cmd`
/// at the root of the extracted directory, with no `bin/` at all. Not created
/// here — this is called on every launch, and a PATH entry that does not exist yet
/// is harmless ([`crate::runtime::env`] drops the empty ones).
pub fn bin_path() -> Option<PathBuf> {
    let d = dir()?;
    Some(if cfg!(windows) { d } else { d.join("bin") })
}

/// [`bin_path`] as a PATH segment.
///
/// Lossy on purpose and only here, at the boundary where a PATH string is what the
/// caller needs. Everything that touches the filesystem goes through `bin_path`
/// instead: a `$HOME` with a non-UTF-8 byte (legal on Linux) would come back with
/// U+FFFD substituted, and an installed Node would then be invisible to
/// `managed_exe()` forever while `install` kept writing to the real path.
pub fn bin_dir() -> Option<String> {
    Some(bin_path()?.to_string_lossy().into_owned())
}

/// The `node` executable inside the managed tree, whether or not it exists.
fn managed_exe() -> Option<PathBuf> {
    let name = if cfg!(windows) { "node.exe" } else { "node" };
    Some(bin_path()?.join(name))
}

/// `runtimes/node/.version` — written at install, read at probe.
///
/// The point of the file is that reporting the managed version costs no process
/// spawn at all: engine detection is forbidden from executing candidates, and this
/// keeps the managed row honest without asking for an exception.
fn version_file() -> Option<PathBuf> {
    dir().map(|d| d.join(".version"))
}

// ---- platform → asset -----------------------------------------------------

/// Which official build belongs on this machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Platform {
    /// The tag inside the archive's file name (`node-v24.20.0-linux-x64.tar.gz`).
    pub asset: &'static str,
    /// The tag the dist index uses in its `files` array — a *different*
    /// vocabulary for the same build: `osx-arm64-tar` is the entry for the file
    /// called `node-v24.20.0-darwin-arm64.tar.gz`. Both are needed, because the
    /// index is what tells us a release really shipped our build.
    pub file_tag: &'static str,
    pub ext: &'static str,
}

/// Resolve this build's target to a Node asset, or explain why there isn't one.
///
/// musl is decided at *compile* time. Detecting it at runtime would mean running
/// something (`ldd --version`) during what is meant to be a probe, and it would
/// answer a question we already know: a glibc-linked launcher does not start on a
/// pure musl system, so the libc this binary was built against is the libc it is
/// running on.
///
/// Known gap: there is no `linux-arm64-musl` build, so Alpine on arm64 has no
/// automatic answer. That is what the `Node 可执行文件` setting is for, and the
/// error says so rather than pretending a download would work.
///
/// The catch-all arms say the *launcher* has no download for this target, not that
/// no build exists: nodejs.org does publish `linux-armv7l`, `linux-armv6l`,
/// `linux-ppc64le` and `linux-s390x`, and telling someone on a Raspberry Pi that
/// their architecture is unsupported by Node would be a plain lie. Adding one is a
/// row in this match, which is the point of naming the reason accurately.
pub fn platform() -> Result<Platform, String> {
    let musl = cfg!(target_env = "musl");
    let p = if cfg!(target_os = "linux") {
        match (cfg!(target_arch = "x86_64"), cfg!(target_arch = "aarch64")) {
            (true, _) if musl => Platform {
                asset: "linux-x64-musl",
                file_tag: "linux-x64-musl",
                ext: ".tar.gz",
            },
            (true, _) => Platform {
                asset: "linux-x64",
                file_tag: "linux-x64",
                ext: ".tar.gz",
            },
            (_, true) if musl => return Err(unsupported("nodejs.org 不发布 linux-arm64-musl")),
            (_, true) => Platform {
                asset: "linux-arm64",
                file_tag: "linux-arm64",
                ext: ".tar.gz",
            },
            _ => return Err(unsupported("启动器没有为这个 CPU 架构准备下载")),
        }
    } else if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            Platform {
                asset: "darwin-arm64",
                file_tag: "osx-arm64-tar",
                ext: ".tar.gz",
            }
        } else if cfg!(target_arch = "x86_64") {
            Platform {
                asset: "darwin-x64",
                file_tag: "osx-x64-tar",
                ext: ".tar.gz",
            }
        } else {
            return Err(unsupported("启动器没有为这个 CPU 架构准备下载"));
        }
    } else if cfg!(target_os = "windows") {
        if cfg!(target_arch = "aarch64") {
            Platform {
                asset: "win-arm64",
                file_tag: "win-arm64-zip",
                ext: ".zip",
            }
        } else if cfg!(target_arch = "x86_64") {
            Platform {
                asset: "win-x64",
                file_tag: "win-x64-zip",
                ext: ".zip",
            }
        } else {
            return Err(unsupported("启动器没有为这个 CPU 架构准备下载"));
        }
    } else {
        return Err(unsupported("启动器没有为这个操作系统准备下载"));
    };
    Ok(p)
}

/// Say which machine we are, and where to go instead. A user on an unsupported
/// target needs the triple (to search with) and the download page (to act on) —
/// "unsupported platform" alone is a dead end.
fn unsupported(why: &str) -> String {
    format!(
        "{why}（{}-{}-{}）。请手动安装 Node 并在「设置 ▸ Node」里指定它，或见 https://nodejs.org/en/download",
        std::env::consts::OS,
        std::env::consts::ARCH,
        if cfg!(target_env = "musl") { "musl" } else { "gnu" }
    )
}

/// `node-v24.20.0-linux-x64.tar.gz` — the file name, which is also the key into
/// `SHASUMS256.txt`.
pub fn asset_name(v: Version, p: Platform) -> String {
    format!("node-v{v}-{}{}", p.asset, p.ext)
}

// ---- probe ----------------------------------------------------------------

/// What the Node settings page and the install buttons both read — mirrors
/// `NodeStatus` in src/types.ts.
#[derive(Debug, Clone, Serialize)]
pub struct NodeStatus {
    /// Absolute path of the `node` that would be used, or empty.
    pub path: String,
    /// `"custom"` | `"managed"` | `"host"` | `""` — which of the three sources
    /// won, in the same precedence an engine's binary follows.
    pub source: String,
    /// `"24.20.0"`, or empty when unknown. Empty is a real state, not an error:
    /// with `auto_detect_version` off we deliberately do not ask a host Node.
    pub version: String,
    /// Whether `version` is at or above [`FLOOR`], or the check is switched off.
    /// `false` with an empty `version` means "not installed", not "too old" — the
    /// UI needs `path` to tell those apart.
    pub ok: bool,
    /// [`FLOOR`] as a string, so no dictionary or component hardcodes it.
    pub floor: String,
    /// The npm that comes with the resolved Node, or empty. Reported separately
    /// because every install recipe runs npm, not node.
    pub npm: String,
    /// The directory the managed tree lives in, shown before anything downloads.
    pub dir: String,
    /// The asset that would be fetched on this machine, or the reason there is
    /// none — the "unsupported platform" message goes here so the page can show it
    /// where the download button would be.
    pub asset: String,
    pub unsupported: String,
}

/// Resolve which `node` wins, and what is known about it.
///
/// Precedence is `settings.exe` > managed > host, the same rule
/// `custom_bin > launcher-managed > host` that [`crate::runtime::env`] applies to
/// engines — one rule, so a user who understands one understands both.
///
/// The managed copy reports its version from `.version` and spawns nothing. A host
/// copy can only be asked, so it is asked *only* when `auto_detect_version` is on:
/// that checkbox is the standing permission, and with it off the row honestly says
/// "found, version unknown" rather than executing a binary behind the user's back.
pub async fn probe(settings: &NodeSettings, path_var: &str) -> NodeStatus {
    let platform = platform();
    let mut st = NodeStatus {
        path: String::new(),
        source: String::new(),
        version: String::new(),
        ok: false,
        floor: FLOOR.to_string(),
        npm: String::new(),
        dir: dir().map(|d| d.display().to_string()).unwrap_or_default(),
        asset: String::new(),
        unsupported: platform.as_ref().err().cloned().unwrap_or_default(),
    };

    let custom = settings.exe.trim();
    // A managed tree counts only while the npm that shipped with it is still there.
    // A `remove_dir_all` that failed halfway — Windows with `node.exe` held open by
    // a running agent, or an AV quarantine — leaves the executable and `.version`
    // behind with no `lib/node_modules/npm`, and that must not shadow a working host
    // Node behind a green row whose install buttons are all dead.
    let managed = managed_exe()
        .filter(|p| p.is_file())
        .filter(|_| managed_npm().is_some());
    if !custom.is_empty() && Path::new(custom).is_file() {
        st.path = custom.to_string();
        st.source = "custom".into();
    } else if let Some(m) = managed {
        st.path = m.display().to_string();
        st.source = "managed".into();
        st.version = read_managed_version().unwrap_or_default();
    } else if let Some(found) = crate::engines::find_on_path("node", path_var) {
        st.path = found;
        st.source = "host".into();
    }

    // A custom or host Node can only be asked; the managed one already answered.
    if st.version.is_empty() && !st.path.is_empty() && settings.auto_detect_version {
        st.version = ask_version(&st.path, path_var).await.unwrap_or_default();
    }
    if !st.path.is_empty() {
        st.npm = crate::engines::find_on_path("npm", &npm_search_path(&st.path, path_var))
            .unwrap_or_default();
    }
    st.ok = match parse_version(&st.version) {
        Some(v) => settings.skip_version_check || v >= FLOOR,
        // Found, but no version to judge. Two ways to get here — detection is off, or
        // it ran and the answer was unusable (a 3 s deadline hit on a cold cache, a
        // version manager's shim printing something else) — and they must agree, or
        // the same Node reads as broken until the user unticks a checkbox. "We could
        // not determine it" is not evidence of being too old, and refusing then is a
        // dead end with no recourse, while letting it through costs at most one npm
        // error that names the version it wanted.
        None => !st.path.is_empty(),
    };
    if let Ok(p) = platform {
        st.asset = format!("node-v<版本>-{}{}", p.asset, p.ext);
    }
    st
}

/// Where to look for the `npm` that belongs to a given `node`.
///
/// Its own directory first, and only then the inherited PATH: a managed Node ships
/// its own npm, and pairing `runtimes/node/bin/node` with a host `/usr/bin/npm`
/// would be the one combination guaranteed not to work — npm is a JS file behind a
/// `#!/usr/bin/env node` shim, so whichever node comes first on PATH is the one
/// that runs it.
pub(crate) fn npm_search_path(node_exe: &str, path_var: &str) -> String {
    let sep = if cfg!(windows) { ';' } else { ':' };
    match Path::new(node_exe).parent() {
        Some(d) if !d.as_os_str().is_empty() => format!("{}{sep}{path_var}", d.display()),
        _ => path_var.to_string(),
    }
}

/// The managed tree's recorded version, if it looks like one.
fn read_managed_version() -> Option<String> {
    let raw = std::fs::read_to_string(version_file()?).ok()?;
    parse_version(&raw).map(|v| v.to_string())
}

/// The `npm` that shipped inside the managed tree, if it is still there.
///
/// Looked up with the same PATH rules as everything else — and only in the managed
/// bin directory, so a host `npm` on the real PATH can never stand in for one that
/// was deleted out of our own tree.
fn managed_npm() -> Option<String> {
    crate::engines::find_on_path("npm", &bin_dir()?)
}

/// Run `<exe> --version` and hand back what it printed, verbatim.
///
/// The one place the launcher executes a candidate to learn about it, and it is
/// fenced accordingly: only reached with `auto_detect_version` on (see [`probe`]) or
/// from an explicit press, stdin closed, and a 3-second deadline so a wrapper script
/// that waits on something cannot stall the settings page.
///
/// `PATH` is set rather than inherited because `npm` is a JS file behind a
/// `#!/usr/bin/env node` shim — asking npm its version *is* running node, and which
/// node that is has to be the one we resolved.
async fn run_version(exe: &str, path_var: &str) -> Result<String, String> {
    let run = tokio::process::Command::new(exe)
        .arg("--version")
        .env("PATH", path_var)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output();
    let out = tokio::time::timeout(std::time::Duration::from_secs(3), run)
        .await
        .map_err(|_| format!("{exe} --version 超过 3 秒没有返回"))?
        .map_err(|e| format!("无法执行 {exe}: {e}"))?;
    if !out.status.success() {
        let why = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(format!(
            "{exe} --version 退出码 {}{}",
            out.status.code().unwrap_or(-1),
            if why.is_empty() {
                String::new()
            } else {
                format!("：{why}")
            }
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// The same call, reduced to a version we could compare. A timeout, a non-zero
/// exit, or output that is not a version is `None` — "unknown", never a guess.
async fn ask_version(exe: &str, path_var: &str) -> Option<String> {
    let raw = run_version(exe, path_var).await.ok()?;
    parse_version(&raw).map(|v| v.to_string())
}

// ---- dist index -----------------------------------------------------------

/// One release in `https://nodejs.org/dist/index.json`.
///
/// Three fields out of a dozen, and only one of them ever reaches a URL. `lts` is
/// `false` on a current release and the codename string (`"Krypton"`) on an LTS
/// one, which is why it is a `Value` rather than a `bool`: a stricter type would
/// make the whole 863-entry index fail to parse the day nodejs.org adds a variant.
#[derive(Debug, Deserialize)]
struct Release {
    version: String,
    /// The index's own vocabulary for which builds shipped — `Platform::file_tag`,
    /// not the asset file name. Checked so a release that exists but has no build
    /// for this machine is reported as such instead of as a 404 later.
    #[serde(default)]
    files: Vec<String>,
    #[serde(default)]
    lts: serde_json::Value,
    /// The npm bundled with this release, shown alongside the version so "what am
    /// I about to get" is answerable before the download starts.
    #[serde(default)]
    npm: String,
}

impl Release {
    fn is_lts(&self) -> bool {
        self.lts.as_str().is_some_and(|s| !s.is_empty())
    }
}

/// A release, once it has been reduced to the two strings we will actually use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pick {
    pub version: Version,
    pub npm: String,
}

/// The newest LTS in the index that also ships a build for `p`.
///
/// The index is newest-first, so this is the first LTS entry — but the `files`
/// check is not a formality: `linux-x64-musl` appears only on recent releases, and
/// an older machine's tag could be absent from an entry that is otherwise fine.
///
/// Everything a response says is discarded except `version`, which must parse as
/// `v<digits>.<digits>.<digits>` before it may be interpolated into a URL, and
/// `npm`, which is only ever displayed.
pub fn pick_lts(index_json: &[u8], p: Platform) -> Result<Pick, String> {
    let releases: Vec<Release> =
        serde_json::from_slice(index_json).map_err(|e| format!("无法解析 index.json: {e}"))?;
    let hit = releases
        .iter()
        .filter(|r| r.is_lts())
        .find(|r| r.files.iter().any(|f| f == p.file_tag) && parse_version(&r.version).is_some())
        .ok_or_else(|| format!("index.json 里没有提供 {} 构建的 LTS 版本", p.file_tag))?;
    Ok(Pick {
        version: parse_version(&hit.version).expect("filtered above"),
        npm: hit.npm.clone(),
    })
}

/// Fetch the index and choose. The URL is logged before it is fetched, the same
/// rule the npm recipe follows for its command line.
async fn resolve_latest_lts(app: &AppHandle, p: Platform) -> Result<Pick, String> {
    let url = http::parse_http_url(&format!("{DIST_BASE}/index.json"))?;
    require_dist_host(&url)?;
    log_cmd(app, format!("$ GET {url}\n"));
    let body = http::get_capped_pinned(&url, MAX_INDEX_BYTES).await?;
    let pick = pick_lts(&body, p)?;
    log(
        app,
        format!("最新 LTS: v{} （自带 npm {}）\n", pick.version, pick.npm),
    );
    Ok(pick)
}

/// Refuse any URL that is not literally on [`DIST_HOST`].
///
/// Every URL here is assembled from a constant, so this can only fire if that
/// assembly is changed later — which is exactly the mistake worth a guard rather
/// than a comment.
///
/// Only half the pin, and the easy half: this checks the URL we are about to ask
/// for. Keeping it true for the URL actually *served* is
/// [`crate::download::same_host_redirect_policy`]'s job, which is why both the
/// archive and the two metadata fetches here go through clients that hold the host
/// across every hop. Checked once and then followed anywhere would leave the claim
/// in this module's header false.
fn require_dist_host(url: &reqwest::Url) -> Result<(), String> {
    if url.host_str() != Some(DIST_HOST) {
        return Err(format!("拒绝从 {:?} 下载 Node", url.host_str()));
    }
    Ok(())
}

// ---- progress and logging -------------------------------------------------

/// How far the archive has got. Its own event because the CLI installs have
/// nothing to report it with — npm prints lines, not byte counts — so the two
/// paths share `install-log` and diverge here.
#[derive(Clone, Serialize)]
struct InstallProgress {
    engine: String,
    received: u64,
    total: Option<u64>,
}

/// `"node"` in every install event, so the frontend's existing per-engine log
/// window needs no new plumbing to show this.
const CHANNEL: &str = "node";

fn log(app: &AppHandle, chunk: String) {
    crate::runtimes::log(app, CHANNEL, "stdout", chunk);
}

/// A line the user could rerun by hand — every URL goes through here before it is
/// fetched.
fn log_cmd(app: &AppHandle, chunk: String) {
    crate::runtimes::log(app, CHANNEL, "cmd", chunk);
}

fn progress(app: &AppHandle, received: u64, total: Option<u64>) {
    let _ = tauri::Emitter::emit(
        app,
        "install-progress",
        InstallProgress {
            engine: CHANNEL.to_string(),
            received,
            total,
        },
    );
}

// ---- integrity ------------------------------------------------------------

/// The digest `nodejs.org` publishes for `asset`, out of that release's
/// `SHASUMS256.txt`.
///
/// What this buys, stated straight so no later reader over-reads it: it catches a
/// corrupted or truncated transfer, a cache or mirror serving a stale file, and an
/// attacker who can replace one archive but not the manifest. It does **not** catch
/// a compromised nodejs.org — the manifest and the archive travel the same TLS
/// connection to the same host, so the only trust anchor is the certificate chain.
/// Catching that needs `SHASUMS256.txt.sig` verified against the Node release
/// team's keys, which is a separate change and a separate dependency.
async fn expected_sha256(app: &AppHandle, v: Version, asset: &str) -> Result<String, String> {
    let url = http::parse_http_url(&format!("{DIST_BASE}/v{v}/SHASUMS256.txt"))?;
    require_dist_host(&url)?;
    log_cmd(app, format!("$ GET {url}\n"));
    let body = http::get_capped_pinned(&url, MAX_SHASUMS_BYTES).await?;
    let text = String::from_utf8_lossy(&body);
    download::sha256_from_manifest(&text, asset)
}

// ---- archive safety -------------------------------------------------------

/// Accept an archive member's path, or say why not.
///
/// The `tar` and `zip` crates both offer a safe unpack of their own, and this runs
/// anyway: the check is cheap, the failure mode is writing outside the launcher's
/// own directory, and "the library handles it" is not a property this file can
/// assert about a version it will be compiled against later.
///
/// Only `Normal` components survive, so an absolute path, a Windows drive prefix, a
/// `..` anywhere, and a bare `.` are all refused. The first component is returned
/// alongside so the caller can insist every member shares one top-level directory.
fn safe_member(raw: &Path) -> Result<PathBuf, String> {
    let mut out = PathBuf::new();
    for c in raw.components() {
        match c {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            _ => return Err(format!("归档里有可疑路径: {}", raw.display())),
        }
    }
    if out.as_os_str().is_empty() {
        return Err(format!("归档里有空路径: {}", raw.display()));
    }
    Ok(out)
}

/// Whether a link inside the archive still points inside the tree that will be
/// installed.
///
/// Node's own tarball contains symlinks and they are legitimate: `bin/npm` points
/// at `../lib/node_modules/npm/bin/npm-cli.js`. So the rule cannot be "no links",
/// it has to be "no link that escapes" — resolved by counting depth rather than by
/// touching the filesystem, because the target usually does not exist yet when the
/// link is created.
///
/// `base_depth` is how deep the link's own directory sits: for a symlink that is
/// the number of components in its path minus one (targets are relative to the link),
/// and for a hard link it is `0` (tar resolves those against the archive root).
///
/// The floor is depth **1**, not 0, and that is the whole subtlety. Depth 0 is the
/// staging root, but staging holds one directory and *that* directory is what
/// becomes `runtimes/node` — so a target resolving to depth 0 lands on the npm
/// prefix itself after the rename, which is on every child PATH. Everything the
/// archive legitimately contains lives at depth ≥ 1.
fn link_stays_inside(base_depth: usize, target: &Path) -> bool {
    if target.is_absolute() {
        return false;
    }
    let mut depth = base_depth as i64;
    for c in target.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                depth -= 1;
                if depth < 1 {
                    return false;
                }
            }
            Component::Normal(_) => depth += 1,
            // A prefix or a root component means it was not relative after all.
            _ => return false,
        }
    }
    true
}

/// A pax *global* header is a real archive member as far as `tar` is concerned: the
/// crate consumes GNU long names and pax per-file extensions itself, but hands this
/// one to the caller as an ordinary entry named `pax_global_header`. It carries no
/// file, `unpack_in` skips it, and only the top-level-directory bookkeeping would
/// trip over it — so it is filtered there, before it can look like a second top
/// level and fail the whole install.
///
/// Today's nodejs.org tarballs are GNU format and contain no such member. This is
/// here so that switching to `tar --format=posix` upstream is not an outage.
const PAX_GLOBAL: &str = "pax_global_header";

/// Insist every member shares one top-level directory, and remember which.
///
/// Both official formats are a single directory (`node-v24.20.0-linux-x64/…`), and
/// that directory is what gets renamed into place — so "how many top levels are
/// there" is not trivia, it is the difference between moving the tree and moving
/// one arbitrary child of it.
fn note_top(top: &mut Option<String>, rel: &Path) -> Result<(), String> {
    let first = rel
        .components()
        .next()
        .ok_or("归档里有空路径")?
        .as_os_str()
        .to_string_lossy()
        .into_owned();
    if first == PAX_GLOBAL {
        return Ok(());
    }
    match top {
        Some(t) if *t != first => Err(format!("归档里有多个顶层目录: {t} 和 {first}")),
        Some(_) => Ok(()),
        None => {
            *top = Some(first);
            Ok(())
        }
    }
}

/// Cap what may be *written*, as opposed to what may be downloaded.
///
/// [`MAX_ARCHIVE_BYTES`] bounds the transfer; it says nothing about the expansion
/// ratio, and a crafted gzip well inside 128 MB can reach hundreds of gigabytes.
/// The real archive unpacks to about 206 MB, so 1 GiB is generous headroom for a
/// Node that grows and still a bound.
const MAX_UNPACKED_BYTES: u64 = 1024 * 1024 * 1024;

/// Add `n` to the running total, refusing to go past [`MAX_UNPACKED_BYTES`].
fn bump_unpacked(total: u64, n: u64) -> Result<u64, String> {
    let next = total.saturating_add(n);
    if next > MAX_UNPACKED_BYTES {
        return Err(format!(
            "归档解出来超过 {} MiB，已中止",
            MAX_UNPACKED_BYTES / 1024 / 1024
        ));
    }
    Ok(next)
}

/// Unpack a `.tar.gz` into `root`, returning its single top-level directory name.
///
/// One streaming pass, so each member is validated immediately before it is
/// written rather than in a survey pass that a gzip stream could not repeat.
fn extract_tar_gz(archive: &Path, root: &Path) -> Result<String, String> {
    let file = std::fs::File::open(archive).map_err(|e| format!("{}: {e}", archive.display()))?;
    let mut ar = tar::Archive::new(flate2::read::GzDecoder::new(file));
    // The executables have to stay executable; the explicit chmod after extraction
    // covers the case of an archive whose modes are wrong.
    ar.set_preserve_permissions(cfg!(unix));
    ar.set_overwrite(true);
    let mut top: Option<String> = None;
    let mut unpacked: u64 = 0;
    for entry in ar.entries().map_err(|e| format!("无法读取归档: {e}"))? {
        let mut entry = entry.map_err(|e| format!("无法读取归档条目: {e}"))?;
        let raw = entry
            .path()
            .map_err(|e| format!("归档条目路径无效: {e}"))?
            .into_owned();
        let rel = safe_member(&raw)?;
        note_top(&mut top, &rel)?;

        let kind = entry.header().entry_type();
        if kind.is_symlink() || kind.is_hard_link() {
            let target = entry
                .link_name()
                .map_err(|e| format!("链接目标无效: {e}"))?
                .ok_or_else(|| format!("{} 是链接但没有目标", rel.display()))?
                .into_owned();
            // A symlink's target resolves against its own directory; tar resolves a
            // hard link's against the archive root.
            let base = if kind.is_symlink() {
                rel.components().count().saturating_sub(1)
            } else {
                0
            };
            if !link_stays_inside(base, &target) {
                return Err(format!(
                    "{} 指向归档外面: {}",
                    rel.display(),
                    target.display()
                ));
            }
        }
        // tar reads exactly the declared size for a member, so the header is a
        // faithful count here — unlike a zip's central directory, where the actual
        // bytes copied are what get counted.
        unpacked = bump_unpacked(unpacked, entry.header().size().unwrap_or(0))?;
        entry
            .unpack_in(root)
            .map_err(|e| format!("解压 {} 失败: {e}", rel.display()))?;
    }
    top.ok_or_else(|| "归档是空的".to_string())
}

/// Unpack a `.zip` into `root`, returning its single top-level directory name.
///
/// Compiled on every platform even though only the Windows asset is a zip: a
/// `cfg`-gated branch would never be type-checked by a Linux CI run, and the
/// Windows path is already the least-exercised one in this repository.
fn extract_zip(archive: &Path, root: &Path) -> Result<String, String> {
    use std::io::Write;

    let file = std::fs::File::open(archive).map_err(|e| format!("{}: {e}", archive.display()))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("无法读取归档: {e}"))?;
    let mut top: Option<String> = None;
    let mut unpacked: u64 = 0;
    for i in 0..zip.len() {
        let mut member = zip
            .by_index(i)
            .map_err(|e| format!("无法读取归档条目 {i}: {e}"))?;
        // `enclosed_name` is the crate's own refusal to leave the target directory;
        // `safe_member` is ours, and disagreeing with it would be the interesting case.
        let raw = member
            .enclosed_name()
            .ok_or_else(|| format!("归档里有可疑路径: {}", member.name()))?;
        let rel = safe_member(&raw)?;
        note_top(&mut top, &rel)?;

        // Node's zip has no symlinks; one that did would be a change of shape worth
        // stopping for rather than resolving.
        if member.is_symlink() {
            return Err(format!("zip 归档里出现了链接: {}", rel.display()));
        }
        let dest = root.join(&rel);
        if member.is_dir() {
            std::fs::create_dir_all(&dest).map_err(|e| format!("{}: {e}", dest.display()))?;
            continue;
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        let mut out =
            std::fs::File::create(&dest).map_err(|e| format!("{}: {e}", dest.display()))?;
        let n = std::io::copy(&mut member, &mut out)
            .map_err(|e| format!("写入 {} 失败: {e}", dest.display()))?;
        unpacked = bump_unpacked(unpacked, n)?;
        out.flush()
            .map_err(|e| format!("{}: {e}", dest.display()))?;
    }
    top.ok_or_else(|| "归档是空的".to_string())
}

/// Dispatch on the asset's extension rather than on `cfg!`, so both readers are
/// compiled and type-checked everywhere.
fn extract(archive: &Path, root: &Path, ext: &str) -> Result<String, String> {
    if ext == ".zip" {
        extract_zip(archive, root)
    } else {
        extract_tar_gz(archive, root)
    }
}

// ---- install --------------------------------------------------------------

/// Download the latest LTS Node and put it at [`dir`], returning what landed.
///
/// **Emits no `install-done`.** That is not an oversight and must not be "fixed":
/// when this runs as the first half of a chained engine install, the frontend's
/// `install-done` handler clears its `busy` flag, and the buttons would light up
/// again halfway through the chain. Only the *outermost* command reports an
/// outcome; this one reports progress and log lines and returns a `Result` to its
/// caller.
///
/// Nothing observable changes until the very end: the archive lands under a `.part`
/// name inside [`crate::runtimes::root`], extraction goes to `.node-staging-<pid>/`,
/// and `node/` appears by a single rename once `bin/node` has been seen inside it.
/// A crash therefore leaves at most one dot-prefixed directory, which the next
/// install removes.
pub async fn install(app: &AppHandle) -> Result<Version, String> {
    let p = platform()?;
    let prefix = crate::runtimes::ensure_prefix()?;
    let target = dir().ok_or("无法定位主目录")?;
    sweep_leftovers(&prefix);

    let pick = resolve_latest_lts(app, p).await?;
    let asset = asset_name(pick.version, p);
    let url = http::parse_http_url(&format!("{DIST_BASE}/v{}/{asset}", pick.version))?;
    require_dist_host(&url)?;

    let want = expected_sha256(app, pick.version, &asset).await?;
    log(app, format!("SHASUMS256.txt 记录的 sha256: {want}\n"));

    let archive = prefix.join(&asset);
    log_cmd(app, format!("$ GET {url}\n"));
    let progress_app = app.clone();
    let got =
        download::download_to_file(&url, &archive, MAX_ARCHIVE_BYTES, move |received, total| {
            progress(&progress_app, received, total);
        })
        .await?;
    if !got.sha256.eq_ignore_ascii_case(&want) {
        let _ = std::fs::remove_file(&archive);
        return Err(format!(
            "sha256 不匹配：期望 {want}，实际 {}。已删除下载的文件",
            got.sha256
        ));
    }
    log(app, format!("sha256 一致（{} 字节）\n", got.bytes));

    let staging = prefix.join(format!(".node-staging-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|e| format!("{}: {e}", staging.display()))?;
    let outcome = unpack_and_swap(app, &staging, &archive, &target, p, pick.version);

    // The archive is 58 MB and the staging tree a second copy of the install; both
    // are worthless once the rename has happened, and just as worthless if it did not.
    let _ = std::fs::remove_dir_all(&staging);
    let _ = std::fs::remove_file(&archive);
    outcome.map(|()| pick.version)
}

/// Remove any `.part` file or `.node-staging-*` / `.node-old-*` directory a
/// previous run died inside. Only dot-prefixed names, and only directly under the
/// prefix, so nothing a user put there is in scope.
///
/// Age-gated, because the pid in those names is not decoration: nothing stops a
/// second launcher process from existing (no single-instance guard is registered),
/// and [`crate::runtimes::install_lock`] only excludes installs inside one process.
/// An unqualified sweep would let a starting install delete the staging tree — or,
/// worse, the mid-swap rollback copy — of one already running next to it. A stale
/// leftover is by definition not being written to, so anything touched recently is
/// somebody else's and gets left alone.
const LEFTOVER_MIN_AGE: std::time::Duration = std::time::Duration::from_secs(60 * 60);

fn sweep_leftovers(prefix: &Path) {
    let Ok(entries) = std::fs::read_dir(prefix) else {
        return;
    };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let ours = name.starts_with(".node-staging-") || name.starts_with(".node-old-");
        let part = name.starts_with("node-v") && name.ends_with(".part");
        if !ours && !part {
            continue;
        }
        let recent = e
            .metadata()
            .and_then(|m| m.modified())
            .and_then(|t| t.elapsed().map_err(std::io::Error::other))
            .map(|age| age < LEFTOVER_MIN_AGE)
            // An unreadable or future mtime is treated as "recent" — refusing to
            // delete is the safe way to be wrong here.
            .unwrap_or(true);
        if recent {
            continue;
        }
        if ours {
            let _ = std::fs::remove_dir_all(e.path());
        } else {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// Extract into `staging`, prove the result is a Node, then make it `node/` in one
/// rename.
///
/// The old tree is moved aside rather than deleted first, so a failing rename leaves
/// the previous install in place instead of nothing at all.
fn unpack_and_swap(
    app: &AppHandle,
    staging: &Path,
    archive: &Path,
    target: &Path,
    p: Platform,
    v: Version,
) -> Result<(), String> {
    log(app, "解压…\n".to_string());
    let top = extract(archive, staging, p.ext)?;
    let unpacked = staging.join(&top);

    // The one property worth checking before anything is moved: the thing we are
    // about to call `node/` actually contains a node.
    let exe = if cfg!(windows) {
        unpacked.join("node.exe")
    } else {
        unpacked.join("bin").join("node")
    };
    if !exe.is_file() {
        return Err(format!("解压后没有找到 {}", exe.display()));
    }

    let aside = target.with_file_name(format!(".node-old-{}", std::process::id()));
    let had_old = target.exists();
    if had_old {
        std::fs::rename(target, &aside)
            .map_err(|e| format!("无法移开旧的 {}: {e}", target.display()))?;
    }
    if let Err(e) = std::fs::rename(&unpacked, target) {
        // Say where the previous tree went if putting it back also failed. Without
        // this the message claims only that the install failed, while the only copy
        // of the old Node sits under a dot-name the next sweep deletes — recoverable
        // by re-downloading, but the user has no way to know that is what happened.
        if had_old {
            if let Err(back) = std::fs::rename(&aside, target) {
                return Err(format!(
                    "无法安装到 {}: {e}；而且旧的那份也没能放回去 ({back})，它现在在 {}",
                    target.display(),
                    aside.display()
                ));
            }
        }
        return Err(format!("无法安装到 {}: {e}", target.display()));
    }
    let _ = std::fs::remove_dir_all(&aside);

    // Before `.version` exists, `probe` finds an executable with no recorded
    // version — and with 自动检测 on it would spawn the freshly unpacked binary to
    // ask, which is the one thing the managed row promises not to do. So this is not
    // bookkeeping-after-the-fact; the tree is only fully installed once it is here.
    write_version(v)?;
    make_executable(target);
    log(app, format!("已安装 Node v{v} 到 {}\n", target.display()));
    Ok(())
}

/// Record the version so [`probe`] can report it without spawning anything.
fn write_version(v: Version) -> Result<(), String> {
    let path = version_file().ok_or("无法定位主目录")?;
    std::fs::write(&path, format!("{v}\n")).map_err(|e| format!("{}: {e}", path.display()))
}

/// Set the executable bit on the launchers we are going to spawn, instead of
/// trusting the archive's own modes.
///
/// `npm` and `npx` are relative symlinks into `lib/node_modules/npm/`, so this
/// follows them to the scripts they point at — which is where the bit has to be for
/// a `#!/usr/bin/env node` shim to run at all.
fn make_executable(root: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for name in ["node", "npm", "npx", "corepack"] {
            let p = root.join("bin").join(name);
            if p.exists() {
                let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755));
            }
        }
    }
    #[cfg(not(unix))]
    let _ = root;
}

/// Delete the managed tree. One `rm -r`, which is the whole point of keeping it in
/// a private prefix.
pub fn uninstall() -> Result<(), String> {
    let d = dir().ok_or("无法定位主目录")?;
    if !d.exists() {
        return Ok(());
    }
    std::fs::remove_dir_all(&d).map_err(|e| format!("{}: {e}", d.display()))
}

// ---- commands -------------------------------------------------------------

/// The launcher's own view of Node: saved settings plus the PATH a child would get.
///
/// One resolution shared by the settings page, the install pane and the installer
/// itself, so "which node" can never be answered two different ways.
pub(crate) async fn status() -> NodeStatus {
    let path_var = crate::runtime::env::resolve_child_path("autodetect", "")
        .await
        .unwrap_or_default();
    probe(&launcher_config::node_settings(), &path_var).await
}

#[tauri::command]
pub async fn node_status() -> NodeStatus {
    status().await
}

/// Remove the managed tree. Not the user's own Node — [`dir`] is inside the
/// launcher's prefix and nothing else is ever touched.
///
/// Takes the install lock, and not for tidiness: without it, 卸载 pressed during an
/// install can land between the rename that makes the tree live and the
/// `.version` write that finishes it, after which the install still reports success
/// over a directory that is no longer there.
#[tauri::command]
pub async fn uninstall_node() -> Result<(), String> {
    let _guard = crate::runtimes::install_lock()
        .try_lock()
        .map_err(|_| "正在安装，等它结束再卸载".to_string())?;
    uninstall()
}

/// What 测试设置 reports: both binaries actually run, and what they said.
#[derive(Debug, Clone, Serialize)]
pub struct NodeTest {
    pub node_path: String,
    pub node_output: String,
    pub npm_path: String,
    pub npm_output: String,
}

/// Prism's 测试设置, and the honest version of it: it runs the two binaries rather
/// than restating what the probe already guessed.
///
/// An explicit press, which is why it executes regardless of
/// `auto_detect_version` — that checkbox governs the *passive* probe, not a button
/// the user just pushed.
///
/// A failing npm is reported *inside* the result rather than as an `Err`: "node
/// works, npm does not" is the diagnosis the user came for, and returning early
/// would throw away the half that succeeded and show one error string instead.
#[tauri::command]
pub async fn test_node() -> Result<NodeTest, String> {
    let st = status().await;
    if st.path.is_empty() {
        return Err("还没有可用的 Node".to_string());
    }
    let path_var = crate::runtime::env::resolve_child_path("autodetect", "")
        .await
        .unwrap_or_default();
    let search = npm_search_path(&st.path, &path_var);
    let node_output = match run_version(&st.path, &search).await {
        Ok(out) => out,
        Err(e) => e,
    };
    let (npm_path, npm_output) = match st.npm.as_str() {
        "" => (String::new(), "没有找到 npm".to_string()),
        npm => (
            npm.to_string(),
            match run_version(npm, &search).await {
                Ok(out) => out,
                Err(e) => e,
            },
        ),
    };
    Ok(NodeTest {
        node_path: st.path,
        node_output,
        npm_path,
        npm_output,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{temp_tree, EnvGuard, HOME_LOCK};

    /// The comparison the whole floor rests on, and the one string comparison gets
    /// backwards: `22.19.0` is newer than `22.9.0`.
    #[test]
    fn versions_order_numerically_not_lexically() {
        let a = parse_version("v22.19.0").unwrap();
        let b = parse_version("22.9.0").unwrap();
        assert!(a > b, "22.19.0 must outrank 22.9.0");
        assert!(
            "22.19.0" < "22.9.0",
            "…which is exactly what strings get wrong"
        );

        assert!(parse_version("24.20.0\n").unwrap() >= FLOOR);
        assert!(parse_version("v20.11.1").unwrap() < FLOOR);
        assert_eq!(FLOOR.to_string(), "22.19.0");
    }

    /// `parse_version` also guards what gets interpolated into a download URL, so
    /// anything that is not exactly three runs of digits is refused.
    #[test]
    fn only_three_plain_numbers_parse_as_a_version() {
        for bad in [
            "24.20",
            "24.20.0.1",
            "24.20.x",
            "24..0",
            "v",
            "",
            "24.20.0-rc.1",
            "２４.20.0",
            "24.20.0/../../etc",
            "-1.0.0",
        ] {
            assert!(parse_version(bad).is_none(), "{bad:?} must not parse");
        }
    }

    /// Every supported target maps to a file that really exists on nodejs.org, and
    /// the two vocabularies stay paired: the index's `files` tag is *not* the asset
    /// file name (`osx-arm64-tar` names `node-v…-darwin-arm64.tar.gz`).
    #[test]
    fn the_asset_name_matches_this_machine() {
        let v = Version {
            major: 24,
            minor: 20,
            patch: 0,
        };
        match platform() {
            Ok(p) => {
                let name = asset_name(v, p);
                assert!(name.starts_with("node-v24.20.0-"), "{name}");
                assert!(name.ends_with(p.ext), "{name}");
                assert!(
                    p.ext == ".tar.gz" || p.ext == ".zip",
                    "only two formats exist"
                );
                assert!(!p.file_tag.is_empty());
            }
            // Alpine on arm64 and anything exotic: the message has to name the
            // machine and where to go, because there is no download to offer.
            Err(why) => {
                assert!(why.contains(std::env::consts::ARCH), "{why}");
                assert!(why.contains("nodejs.org/en/download"), "{why}");
            }
        }
    }

    /// The index is newest-first, but "newest LTS" is not enough on its own: the
    /// entry also has to list a build for this machine, which is how a musl box on
    /// an older release avoids being sent to a 404.
    #[test]
    fn the_newest_lts_that_ships_our_build_is_chosen() {
        let p = Platform {
            asset: "linux-x64-musl",
            file_tag: "linux-x64-musl",
            ext: ".tar.gz",
        };
        let index = br#"[
          {"version":"v25.1.0","lts":false,"npm":"11.20.0","files":["linux-x64","linux-x64-musl"]},
          {"version":"v24.20.0","lts":"Krypton","npm":"11.19.0","files":["linux-x64"]},
          {"version":"v22.19.0","lts":"Jod","npm":"10.9.3","files":["linux-x64","linux-x64-musl"]}
        ]"#;
        let pick = pick_lts(index, p).unwrap();
        assert_eq!(
            pick.version.to_string(),
            "22.19.0",
            "v24.20.0 has no musl build"
        );
        assert_eq!(pick.npm, "10.9.3");

        // A current release is never picked, however new.
        let only_current = br#"[{"version":"v25.1.0","lts":false,"files":["linux-x64-musl"]}]"#;
        assert!(pick_lts(only_current, p).is_err());
        // Neither is an entry whose version string could not become a URL.
        let bad_version = br#"[{"version":"v25.x","lts":"Nope","files":["linux-x64-musl"]}]"#;
        assert!(pick_lts(bad_version, p).is_err());
    }

    /// Node's own tarball contains symlinks and they are legitimate, so the rule
    /// cannot be "no links" — it has to be "no link that escapes".
    ///
    /// The frame of reference is what makes this subtle. Depths are counted from the
    /// *staging* root, and staging holds exactly one directory which becomes
    /// `runtimes/node`. So depth 1 is the tree that gets installed and depth 0 is the
    /// npm prefix around it — a target landing there is outside the tree even though
    /// it is inside the staging directory, and it would end up pointing next to
    /// `node_modules/.bin`, which is on every child PATH.
    #[test]
    fn a_relative_link_inside_the_tree_is_fine_and_one_that_climbs_out_is_not() {
        // The real thing: member `node-v24.20.0-linux-x64/bin/npm` (3 components, so
        // base 2) → `../lib/node_modules/npm/bin/npm-cli.js`, landing at depth 1.
        assert!(link_stays_inside(
            2,
            Path::new("../lib/node_modules/npm/bin/npm-cli.js")
        ));
        assert!(link_stays_inside(3, Path::new("../../lib/x")));
        assert!(link_stays_inside(1, Path::new("./bin/node")));

        // Out of the tree, still inside staging — accepted before the floor moved to
        // 1, and after the rename it points at the npm prefix itself.
        assert!(!link_stays_inside(1, Path::new("../lib/x")));
        assert!(!link_stays_inside(
            2,
            Path::new("../../node_modules/.bin/evil")
        ));

        assert!(!link_stays_inside(1, Path::new("../../etc/passwd")));
        assert!(!link_stays_inside(0, Path::new("../anything")));
        assert!(!link_stays_inside(3, Path::new("/etc/passwd")));
        // Depth is checked as it goes, so a detour back inside does not launder it.
        assert!(!link_stays_inside(2, Path::new("../../x/../lib")));
    }

    /// Nothing but plain components survives, so an absolute path, a `..` anywhere,
    /// and an empty name are all refused before a single byte is written.
    #[test]
    fn only_plain_relative_member_paths_are_accepted() {
        assert_eq!(
            safe_member(Path::new("node-v24.20.0-linux-x64/bin/node")).unwrap(),
            PathBuf::from("node-v24.20.0-linux-x64/bin/node")
        );
        // A `.` component is noise, not a threat.
        assert_eq!(
            safe_member(Path::new("pkg/./bin/node")).unwrap(),
            PathBuf::from("pkg/bin/node")
        );
        for bad in ["/etc/passwd", "pkg/../../evil", "../evil", "..", ".", ""] {
            assert!(
                safe_member(Path::new(bad)).is_err(),
                "{bad:?} must be refused"
            );
        }
    }

    /// One tar, built here, so the validation runs against a real archive rather
    /// than against the crate's promise about one.
    ///
    /// The member name is poked straight into the header instead of going through
    /// `set_path`, which refuses `..` and absolute paths — precisely the two shapes
    /// the hostile archives below are made of. A test that cannot express the attack
    /// cannot prove the defence, so this builder is as permissive as a real tarball,
    /// and `set_cksum` runs last because the name is part of the checksum.
    ///
    /// Link *targets* still go through the crate's own setter: `..` is legal there
    /// (Node's own `bin/npm` uses it), so nothing needs bypassing.
    fn tar_gz(entries: &[(&str, tar::EntryType, &str, &[u8])]) -> Vec<u8> {
        use std::io::Write;
        let mut b = tar::Builder::new(Vec::new());
        for (path, kind, link, body) in entries {
            let mut h = tar::Header::new_gnu();
            h.set_entry_type(*kind);
            h.set_mode(0o755);
            if !link.is_empty() {
                h.set_size(0);
                h.set_link_name(link).unwrap();
            } else {
                h.set_size(body.len() as u64);
            }
            let name = path.as_bytes();
            assert!(name.len() < 100, "{path}: no long-name support needed here");
            h.as_old_mut().name[..name.len()].copy_from_slice(name);
            h.set_cksum();
            b.append(&h, *body).unwrap();
        }
        let plain = b.into_inner().unwrap();
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        gz.write_all(&plain).unwrap();
        gz.finish().unwrap()
    }

    fn extract_bytes(tag: &str, bytes: &[u8]) -> Result<String, String> {
        let root = temp_tree(tag);
        let archive = root.path().join("a.tar.gz");
        std::fs::write(&archive, bytes).unwrap();
        let into = root.path().join("staging");
        std::fs::create_dir_all(&into).unwrap();
        let got = extract(&archive, &into, ".tar.gz");
        if got.is_ok() {
            // Whatever happened, it happened inside the staging directory.
            assert!(!root.path().join("evil").exists());
        }
        got
    }

    /// A real Node layout — one top-level directory, an executable, and a relative
    /// symlink into a sibling directory — comes out intact.
    ///
    /// The leading `pax_global_header` is the shape a `tar --format=posix` switch
    /// upstream would produce. `tar` hands it to us as an ordinary member, so without
    /// the filter in `note_top` it reads as a second top-level directory and fails
    /// the whole install; `unpack_in` ignores it either way.
    #[test]
    fn a_node_shaped_archive_extracts_with_its_links() {
        use tar::EntryType as E;
        let bytes = tar_gz(&[
            ("pax_global_header", E::Regular, "", b"52 comment=x\n"),
            ("node-v24.20.0-linux-x64/", E::Directory, "", b""),
            ("node-v24.20.0-linux-x64/bin/node", E::Regular, "", b"#!x\n"),
            (
                "node-v24.20.0-linux-x64/lib/npm-cli.js",
                E::Regular,
                "",
                b"//\n",
            ),
            (
                "node-v24.20.0-linux-x64/bin/npm",
                E::Symlink,
                "../lib/npm-cli.js",
                b"",
            ),
        ]);
        let root = temp_tree("node-extract-ok");
        let archive = root.path().join("a.tar.gz");
        std::fs::write(&archive, &bytes).unwrap();
        let into = root.path().join("staging");
        std::fs::create_dir_all(&into).unwrap();

        let top = extract(&archive, &into, ".tar.gz").unwrap();
        assert_eq!(top, "node-v24.20.0-linux-x64");
        assert!(into.join(&top).join("bin").join("node").is_file());
        #[cfg(unix)]
        assert_eq!(
            std::fs::read_link(into.join(&top).join("bin").join("npm")).unwrap(),
            PathBuf::from("../lib/npm-cli.js")
        );
    }

    /// The three shapes that would write outside the staging directory, and the one
    /// that would make "rename the top-level directory into place" a lie.
    #[test]
    fn an_archive_that_reaches_outside_is_refused() {
        use tar::EntryType as E;

        let climbing = tar_gz(&[("pkg/../../evil", E::Regular, "", b"x")]);
        assert!(extract_bytes("node-extract-climb", &climbing).is_err());

        let absolute = tar_gz(&[("/tmp/evil-agentlauncher", E::Regular, "", b"x")]);
        assert!(extract_bytes("node-extract-abs", &absolute).is_err());

        let escaping_link = tar_gz(&[
            ("pkg/bin/node", E::Regular, "", b"x"),
            ("pkg/bin/npm", E::Symlink, "../../../../etc/passwd", b""),
        ]);
        assert!(extract_bytes("node-extract-link", &escaping_link).is_err());

        let two_tops = tar_gz(&[
            ("pkg-a/bin/node", E::Regular, "", b"x"),
            ("pkg-b/bin/node", E::Regular, "", b"x"),
        ]);
        assert!(extract_bytes("node-extract-tops", &two_tops).is_err());
    }

    /// Drives the async probe from a plain `#[test]`: `HOME_LOCK` is a std mutex and
    /// holding one across an `.await` is both a clippy warning and a real hazard.
    fn block_on<F: std::future::Future>(f: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime")
            .block_on(f)
    }

    /// The whole reason `.version` exists: the managed tree reports its version with
    /// no spawn at all.
    ///
    /// The fake `bin/node` here is a text file with no executable bit, so any code
    /// path that tried to *run* it to learn the version would fail and leave the field
    /// empty — which makes this test proof of the claim, not just of the happy path.
    #[test]
    fn the_managed_tree_reports_its_version_without_running_anything() {
        let _lock = HOME_LOCK.lock().unwrap();
        let home = temp_tree("node-version");
        let _guard = EnvGuard::set("HOME", home.path());
        // First, before a single byte is written: this test truncates a `node`
        // executable and ends in `uninstall()`, so on a platform where redirecting
        // HOME does not work it must fail rather than do that to the real tree.
        crate::test_support::assert_home_redirected(home.path());

        let root = dir().unwrap();
        // Built from `managed_exe()` rather than a hardcoded `bin/node`, because the
        // layout differs by platform (`node/bin/node` vs `node\node.exe`).
        let exe = managed_exe().unwrap();
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, b"not an executable\n").unwrap();
        // `probe` only accepts a managed tree that still has its own npm — a
        // half-deleted one must not shadow a working host Node.
        let npm = exe.with_file_name(if cfg!(windows) { "npm.cmd" } else { "npm" });
        std::fs::write(&npm, b"#!/usr/bin/env node\n").unwrap();

        let v = Version {
            major: 24,
            minor: 20,
            patch: 0,
        };
        write_version(v).unwrap();
        // Trailing newline, so the file is readable with `cat` and by a shell.
        assert_eq!(
            std::fs::read_to_string(version_file().unwrap()).unwrap(),
            "24.20.0\n"
        );
        assert_eq!(read_managed_version().as_deref(), Some("24.20.0"));

        // Detection switched *off* — the managed row must still know its version.
        let settings = crate::launcher_config::NodeSettings {
            auto_detect_version: false,
            ..Default::default()
        };
        let st = block_on(probe(&settings, ""));
        assert_eq!(st.source, "managed");
        assert_eq!(st.version, "24.20.0");
        assert!(st.ok, "24.20.0 clears the {FLOOR} floor");
        assert_eq!(st.path, exe.display().to_string());

        // A truncated or hand-mangled file is ignored rather than believed: the same
        // `parse_version` guard that protects the download URL protects this.
        std::fs::write(version_file().unwrap(), "24.20\n").unwrap();
        assert_eq!(read_managed_version(), None);
        // Found but unversioned, with detection off — `ok` stays true so the checkbox
        // is not a trap (see `probe`).
        let st = block_on(probe(&settings, ""));
        assert_eq!(st.source, "managed");
        assert!(st.version.is_empty());
        assert!(st.ok);

        // One `rm -r`, and everything falls back to "absent".
        uninstall().unwrap();
        assert!(!root.exists());
        let st = block_on(probe(&settings, ""));
        assert!(st.path.is_empty() && st.source.is_empty() && !st.ok);

        // And a tree whose npm was deleted out from under it — a `remove_dir_all`
        // that failed halfway — is not reported as a usable managed Node.
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, b"not an executable\n").unwrap();
        write_version(v).unwrap();
        let st = block_on(probe(&settings, ""));
        assert_ne!(st.source, "managed", "a tree with no npm is not usable");
    }
}
