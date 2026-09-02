//! Guarded transport for anything the launcher fetches and then puts on disk.
//!
//! Sibling of [`crate::market::http`], and deliberately not the same client. That
//! one sets a 45 s `timeout`, which reqwest applies to the *whole* request, body
//! included — correct for a JSON feed we hold in memory, and fatal for a 58 MB
//! archive on a slow line. Here the deadline is per chunk instead: a host that
//! keeps sending is never cut off for being slow, and a host that accepts the
//! connection and then goes quiet is dropped after [`STALL_TIMEOUT`].
//!
//! Everything else is shared with the market transport, and shared as *code* so
//! the two cannot drift: [`scheme_ok`], [`redirect_policy`] and
//! [`same_host_redirect_policy`] live here and `market::http` calls them.
//!
//! The guards, each closing a way one URL could hurt us:
//!   * **scheme allowlist** — `http`/`https` only, on the original URL and again
//!     on every redirect hop, so nothing can end up reading a local file;
//!   * **host pinning** — a download follows redirects only while they stay on the
//!     host and scheme it started on. A pin checked once, before the request, is
//!     no pin at all: one `302` would move both an archive and the checksum it is
//!     compared against to the same other host;
//!   * **byte cap** — counted while writing, so a host that promises 2 KB and
//!     then sends forever fills no disk; `Content-Length` is consulted only to
//!     fail before the first byte;
//!   * **stall timeout** — bounds silence, including the wait for response
//!     headers, which the per-chunk deadline alone cannot reach;
//!   * **throughput floor** — bounds duration, which a per-chunk deadline does not:
//!     one byte every 29 s would otherwise never be cut off, and the caller holds
//!     the install lock with no way to cancel;
//!   * **`.part` file** — the destination name appears only after the last byte
//!     and the digest are in, so a crashed download can never be mistaken for a
//!     complete archive.
//!
//! What a caller gets back is the size and the sha256 of what was actually
//! written. Comparing that against a checksum published by the same host over the
//! same TLS catches corruption and a swapped file; it is not a defense against
//! that host itself, which is a distinction the callers are expected to state
//! rather than blur.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// How long a transfer may produce nothing at all before we give up on it.
const STALL_TIMEOUT: Duration = Duration::from_secs(30);
/// The floor a transfer has to clear once it has had a fair chance to. 8 KB/s is
/// slower than any connection that can plausibly finish a 58 MB archive, so this
/// only ever fires on a host that is technically responding and going nowhere.
const MIN_BYTES_PER_SEC: u64 = 8 * 1024;
/// Long enough that a slow TLS start, a stalled CDN edge that recovers, or a
/// mid-transfer hiccup does not trip the floor.
const THROUGHPUT_GRACE: Duration = Duration::from_secs(60);
/// Progress is for a human watching a bar move; a callback per 8 KB chunk would
/// be thousands of UI events for one archive.
const PROGRESS_EVERY: Duration = Duration::from_millis(300);

/// The one scheme allowlist. `http`/`https` only — every other scheme reachable
/// from a URL string (`file:`, `data:`, …) is a way to make a fetch read
/// something local instead.
pub(crate) fn scheme_ok(url: &reqwest::Url) -> bool {
    matches!(url.scheme(), "http" | "https")
}

/// The one redirect policy: the allowlist applied again on every hop, because a
/// host that answers `302 Location: file:///…` is asking for exactly that, plus a
/// hop limit so a redirect cycle is an error rather than a hang.
///
/// Deliberately permissive about *where* a hop goes — a market feed URL is typed
/// by the user and legitimately redirects across hosts. A host-pinned download
/// wants [`download_redirect_policy`] instead.
pub(crate) fn redirect_policy() -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(|attempt| {
        if !scheme_ok(attempt.url()) {
            let msg = format!("refusing redirect to non-http(s) URL: {}", attempt.url());
            return attempt.error(msg);
        }
        if attempt.previous().len() >= 5 {
            return attempt.error("too many redirects");
        }
        attempt.follow()
    })
}

/// The redirect policy for a host-pinned fetch: same host, same scheme, all the
/// way down.
///
/// A caller that pins a host — [`crate::node`] checks the URL is on `nodejs.org`
/// before asking for it — gets nothing from that check if hop two may go anywhere,
/// and the integrity story is worse than it reads: the archive is compared against
/// a `SHASUMS256.txt` fetched the same way, so a redirect that moves both to the
/// same other host makes the comparison self-consistent and meaningless.
///
/// The scheme is held too, not just checked against the allowlist: `https` → `http`
/// is a downgrade that would hand the transfer to anyone on the path, and no
/// legitimate host needs it.
pub(crate) fn same_host_redirect_policy() -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(|attempt| {
        // `previous()[0]` is the URL originally requested — the one the caller
        // pinned, not the one before this hop, so a chain cannot walk away one
        // host at a time.
        let Some(origin) = attempt.previous().first().cloned() else {
            return attempt.error("redirect with no request to compare against");
        };
        if attempt.url().scheme() != origin.scheme() {
            let msg = format!(
                "refusing redirect that changes scheme: {} -> {}",
                origin.scheme(),
                attempt.url().scheme()
            );
            return attempt.error(msg);
        }
        if !scheme_ok(attempt.url()) {
            let msg = format!("refusing redirect to non-http(s) URL: {}", attempt.url());
            return attempt.error(msg);
        }
        if attempt.url().host_str() != origin.host_str() {
            let msg = format!(
                "refusing redirect off {} to {}",
                origin.host_str().unwrap_or("?"),
                attempt.url()
            );
            return attempt.error(msg);
        }
        if attempt.previous().len() >= 5 {
            return attempt.error("too many redirects");
        }
        attempt.follow()
    })
}

/// Honest, and the same string the market transport sends: some hosts reject an
/// empty User-Agent outright, and an operator is entitled to know who is calling.
pub(crate) const USER_AGENT: &str = concat!("agentlauncher/", env!("CARGO_PKG_VERSION"));

/// One pooled client for the process. Note the absence of `.timeout()` — see the
/// module header; a whole-response deadline is wrong for a 58 MB archive.
///
/// `read_timeout` is what replaces it, and it covers the gap the per-chunk deadline
/// in [`run`] cannot: that one starts at the first `chunk()` await, so a host which
/// completes the TLS handshake and then never answers at all leaves `send()`
/// pending forever — and `install_node` holds the install lock across that await,
/// with no cancel, so one silent host wedges every later install until the app is
/// restarted. `read_timeout` applies to each read and resets after a successful
/// one, headers included.
fn client() -> Result<&'static reqwest::Client, String> {
    static CLIENT: OnceLock<Result<reqwest::Client, String>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .connect_timeout(CONNECT_TIMEOUT)
                .read_timeout(STALL_TIMEOUT)
                .redirect(same_host_redirect_policy())
                .user_agent(USER_AGENT)
                .build()
                .map_err(|e| format!("cannot build HTTP client: {e}"))
        })
        .as_ref()
        .map_err(|e| e.clone())
}

/// Render an error together with its source chain.
///
/// Not cosmetic. A redirect that one of the policies above refuses reaches us from
/// `reqwest` as `error following redirect for url (…)`, with the actual reason —
/// ours, naming the host it would not follow to — one level down in `source()`.
/// Showing only the top line turns every guard in this module into an opaque
/// failure the user cannot act on.
fn chain(e: &(dyn std::error::Error + 'static)) -> String {
    let mut out = e.to_string();
    let mut cur = e.source();
    while let Some(s) = cur {
        let text = s.to_string();
        if !out.contains(&text) {
            out.push_str(": ");
            out.push_str(&text);
        }
        cur = s.source();
    }
    out
}

/// What landed on disk.
#[derive(Debug, Clone)]
pub struct Fetched {
    pub bytes: u64,
    /// Lowercase hex, computed over the bytes as they were written — so it
    /// describes the file that exists, not the response we hoped for.
    pub sha256: String,
}

/// GET `url` into `dest`, refusing to write more than `cap` bytes.
///
/// `on_progress(received, total)` is called at most every [`PROGRESS_EVERY`] and
/// once more when the transfer ends; `total` is `None` when the host sent no
/// `Content-Length` (chunked transfer), which a caller must render as an
/// indeterminate bar rather than as zero.
///
/// The parent directory must exist. On any error the `.part` file is removed, so
/// a retry starts clean and a failed attempt leaves nothing behind.
pub async fn download_to_file<F>(
    url: &reqwest::Url,
    dest: &Path,
    cap: u64,
    mut on_progress: F,
) -> Result<Fetched, String>
where
    F: FnMut(u64, Option<u64>),
{
    if !scheme_ok(url) {
        return Err(format!(
            "only http and https downloads are supported, not {}:",
            url.scheme()
        ));
    }
    // Built by appending to the OsString, not through `format!("{}.part", …)`:
    // `Path::display` is lossy, so a parent directory holding a non-UTF-8 byte
    // (legal on Linux) would come back with U+FFFD substituted and `File::create`
    // would fail with a bare ENOENT on a path that does not exist.
    let part = {
        let mut s = dest.as_os_str().to_os_string();
        s.push(".part");
        PathBuf::from(s)
    };
    let out = run(url, &part, cap, &mut on_progress).await;
    // One cleanup path for both kinds of failure. A failing rename used to return
    // straight through `?` and leave 58 MB of `.part` behind — the doc above says
    // it does not, so it must not.
    let out = match out {
        Ok(fetched) => tokio::fs::rename(&part, dest)
            .await
            .map(|()| fetched)
            .map_err(|e| format!("{}: {e}", dest.display())),
        Err(e) => Err(e),
    };
    if out.is_err() {
        let _ = tokio::fs::remove_file(&part).await;
    }
    out
}

/// The transfer itself, split out so [`download_to_file`] has exactly one place
/// to clean up from.
async fn run<F>(
    url: &reqwest::Url,
    part: &Path,
    cap: u64,
    on_progress: &mut F,
) -> Result<Fetched, String>
where
    F: FnMut(u64, Option<u64>),
{
    let mut res = client()?
        .get(url.clone())
        .send()
        .await
        .map_err(|e| format!("request failed: {}", chain(&e)))?;
    if !res.status().is_success() {
        return Err(format!("HTTP {}", res.status().as_u16()));
    }
    let total = res.content_length();
    // Only to fail before the first byte; the cap that matters is counted below,
    // because this header is the host's claim, not a fact.
    if let Some(len) = total {
        if len > cap {
            return Err(format!("download too large ({len} bytes, cap {cap})"));
        }
    }

    let mut file = tokio::fs::File::create(part)
        .await
        .map_err(|e| format!("{}: {e}", part.display()))?;
    let mut hasher = Sha256::new();
    let mut received: u64 = 0;
    let mut last = Instant::now();
    let started = Instant::now();
    on_progress(0, total);

    loop {
        let chunk = tokio::time::timeout(STALL_TIMEOUT, res.chunk())
            .await
            .map_err(|_| {
                format!(
                    "the server stopped sending for {}s ({received} bytes in)",
                    STALL_TIMEOUT.as_secs()
                )
            })?
            .map_err(|e| format!("read failed: {}", chain(&e)))?;
        let Some(chunk) = chunk else { break };
        received += chunk.len() as u64;
        if received > cap {
            return Err(format!("download exceeded {cap} bytes"));
        }
        // The stall deadline above re-arms on every chunk, so it bounds *silence*
        // and not duration: a host dribbling one byte every 29 seconds is never cut
        // off, and against a 128 MB cap that is longer than a human lifetime — with
        // the install lock held and no way to cancel. A whole-response timeout is
        // still the wrong guard (58 MB on a bad line is legitimately slow), so the
        // floor is throughput, measured only after enough time to be meaningful.
        let secs = started.elapsed().as_secs();
        if secs >= THROUGHPUT_GRACE.as_secs() && received / secs < MIN_BYTES_PER_SEC {
            return Err(format!(
                "download too slow: {received} bytes in {secs}s, under {MIN_BYTES_PER_SEC} B/s"
            ));
        }
        hasher.update(&chunk);
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("{}: {e}", part.display()))?;
        if last.elapsed() >= PROGRESS_EVERY {
            last = Instant::now();
            on_progress(received, total);
        }
    }
    // `flush` is not enough on its own for a file we are about to hand to a
    // decompressor by path: the handle has to be closed, which `drop` does.
    file.flush()
        .await
        .map_err(|e| format!("{}: {e}", part.display()))?;
    drop(file);
    on_progress(received, total);

    Ok(Fetched {
        bytes: received,
        sha256: hex(&hasher.finalize()),
    })
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// Look one file up in `sha256sum`-format text: `<64 hex><spaces><name>`, one per
/// line, where the name may carry the `*` binary marker `sha256sum -b` writes.
///
/// Matching is on the trailing path component and is exact — a manifest lists
/// every artifact of a release, so a substring match would happily accept
/// `…-linux-x64-musl.tar.gz` when asked about `…-linux-x64.tar.gz`. The digest is
/// checked for shape here so a truncated or HTML error page cannot be mistaken
/// for a checksum that simply fails to match.
pub fn sha256_from_manifest(text: &str, filename: &str) -> Result<String, String> {
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let (Some(digest), Some(name)) = (it.next(), it.next()) else {
            continue;
        };
        if it.next().is_some() {
            // A file name with a space in it; no Node artifact has one, and
            // guessing where the name starts is how you accept the wrong line.
            continue;
        }
        let name = name.trim_start_matches('*');
        if name.rsplit('/').next() != Some(filename) {
            continue;
        }
        let digest = digest.to_ascii_lowercase();
        if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(format!("{filename}: malformed checksum in manifest"));
        }
        return Ok(digest);
    }
    Err(format!("{filename} is not listed in the checksum manifest"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;

    #[test]
    fn only_http_schemes_are_downloadable() {
        for good in [
            "https://example.invalid/a.tar.gz",
            "http://example.invalid/a",
        ] {
            assert!(scheme_ok(&reqwest::Url::parse(good).unwrap()), "{good}");
        }
        for bad in ["file:///etc/passwd", "ftp://example.invalid/a", "data:,x"] {
            assert!(!scheme_ok(&reqwest::Url::parse(bad).unwrap()), "{bad}");
        }
    }

    /// The manifest lookup takes the whole name or nothing: every sibling artifact
    /// of a release is in the same file, and several are prefixes of each other.
    #[test]
    fn manifest_lookup_matches_the_whole_name() {
        let text = "\
aaaa1111aaaa1111aaaa1111aaaa1111aaaa1111aaaa1111aaaa1111aaaa1111  node-v24.20.0-linux-x64-musl.tar.gz
BBBB2222bbbb2222bbbb2222bbbb2222bbbb2222bbbb2222bbbb2222bbbb2222 *node-v24.20.0-linux-x64.tar.gz
cccc3333cccc3333cccc3333cccc3333cccc3333cccc3333cccc3333cccc3333  ./win/node-v24.20.0-win-x64.zip
";
        assert_eq!(
            sha256_from_manifest(text, "node-v24.20.0-linux-x64.tar.gz").unwrap(),
            "bbbb2222bbbb2222bbbb2222bbbb2222bbbb2222bbbb2222bbbb2222bbbb2222"
        );
        assert_eq!(
            sha256_from_manifest(text, "node-v24.20.0-linux-x64-musl.tar.gz").unwrap(),
            "aaaa1111aaaa1111aaaa1111aaaa1111aaaa1111aaaa1111aaaa1111aaaa1111"
        );
        // A leading directory is stripped, so a manifest written from elsewhere
        // still resolves.
        assert!(sha256_from_manifest(text, "node-v24.20.0-win-x64.zip").is_ok());
        // Absent, and — the case that matters — never satisfied by a near miss.
        assert!(sha256_from_manifest(text, "node-v24.20.0-darwin-arm64.tar.gz").is_err());
        assert!(sha256_from_manifest(text, "linux-x64.tar.gz").is_err());
        assert!(sha256_from_manifest("", "anything").is_err());
    }

    /// An HTML error page served where a manifest was expected must not read as
    /// "the checksum did not match" — that would send the user hunting a corrupt
    /// download that is really a 404.
    #[test]
    fn a_malformed_digest_is_reported_as_malformed() {
        let e = sha256_from_manifest("<html>404</html>  a.tar.gz", "a.tar.gz").unwrap_err();
        assert!(e.contains("malformed"), "{e}");
        let short = sha256_from_manifest("abcdef  a.tar.gz", "a.tar.gz").unwrap_err();
        assert!(short.contains("malformed"), "{short}");
    }

    /// A one-shot HTTP server on loopback, so the transfer path itself is tested
    /// without reaching the network. Returns the URL it is listening on.
    async fn serve_once(body: Vec<u8>, send_length: bool) -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };
            // Read the request line and headers; a client that never sees its
            // request consumed can block on the write side.
            let mut scratch = [0u8; 2048];
            let _ = sock.read(&mut scratch).await;
            let mut head = String::from("HTTP/1.1 200 OK\r\nConnection: close\r\n");
            if send_length {
                head.push_str(&format!("Content-Length: {}\r\n", body.len()));
            }
            head.push_str("\r\n");
            let _ = sock.write_all(head.as_bytes()).await;
            let _ = sock.write_all(&body).await;
            let _ = sock.shutdown().await;
        });
        (format!("http://{addr}/artifact.bin"), handle)
    }

    /// A one-shot server that answers with a redirect instead of a body, so the
    /// pinning policy can be exercised for real. `location` is sent verbatim.
    async fn redirect_once(location: String) -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };
            let mut scratch = [0u8; 2048];
            let _ = sock.read(&mut scratch).await;
            let head = format!(
                "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
            let _ = sock.write_all(head.as_bytes()).await;
            let _ = sock.shutdown().await;
        });
        (format!("http://{addr}/artifact.bin"), handle)
    }

    /// A download follows a redirect only while it stays on the host it started on.
    ///
    /// This is the guard behind every "the host is a compile-time constant" claim in
    /// [`crate::node`]: without it, one `302` moves both the archive and the
    /// `SHASUMS256.txt` it is compared against to the same other host, and the
    /// comparison becomes self-consistent and worthless. `localhost` and `127.0.0.1`
    /// resolve to the same machine and are still different hosts, which is exactly
    /// the distinction the policy has to make.
    #[tokio::test]
    async fn a_download_refuses_a_redirect_off_its_own_host() {
        let dir = crate::test_support::temp_tree("download-redirect");
        let dest = dir.path().join("artifact.bin");

        let (elsewhere, target) = serve_once(b"payload".to_vec(), true).await;
        let moved = elsewhere.replace("127.0.0.1", "localhost");
        let (url, server) = redirect_once(moved).await;

        let e = download_to_file(&reqwest::Url::parse(&url).unwrap(), &dest, 1024, |_, _| {})
            .await
            .unwrap_err();
        assert!(e.contains("refusing redirect off"), "{e}");
        assert!(!dest.exists(), "nothing lands when the hop is refused");
        assert!(
            !dir.path().join("artifact.bin.part").exists(),
            "and no .part is left behind"
        );

        server.await.unwrap();
        target.abort();
    }

    #[tokio::test]
    async fn a_download_lands_under_its_final_name_with_its_digest() {
        let dir = crate::test_support::temp_tree("download-ok");
        let dest = dir.path().join("artifact.bin");
        let (url, server) = serve_once(b"hello node".to_vec(), true).await;

        let mut seen: Vec<(u64, Option<u64>)> = Vec::new();
        let got = download_to_file(&reqwest::Url::parse(&url).unwrap(), &dest, 1024, |r, t| {
            seen.push((r, t))
        })
        .await
        .unwrap();
        server.await.unwrap();

        assert_eq!(got.bytes, 10);
        // `printf 'hello node' | sha256sum`
        assert_eq!(
            got.sha256,
            "c3f5abe3e11d87d645b9e9fda1bad6a8d2f9e54f7e81478138ac134ba7ac7280"
        );
        assert_eq!(std::fs::read(&dest).unwrap(), b"hello node");
        // The `.part` name never survives a success.
        assert!(!dir.path().join("artifact.bin.part").exists());
        // Progress is reported at least at the start and at the end, and the
        // last call carries the full size.
        assert!(seen.len() >= 2, "{seen:?}");
        assert_eq!(seen.last().unwrap(), &(10, Some(10)));
    }

    /// The cap is enforced against bytes actually arriving, not against the
    /// header — a host that under-reports its size still cannot overrun it, and
    /// nothing is left on disk to be mistaken for a complete file.
    #[tokio::test]
    async fn an_oversized_body_is_cut_off_and_leaves_nothing_behind() {
        let dir = crate::test_support::temp_tree("download-cap");
        let dest = dir.path().join("artifact.bin");
        let (url, server) = serve_once(vec![b'x'; 4096], false).await;

        let err = download_to_file(&reqwest::Url::parse(&url).unwrap(), &dest, 512, |_, _| {})
            .await
            .unwrap_err();
        server.await.unwrap();

        assert!(err.contains("exceeded"), "{err}");
        assert!(!dest.exists(), "no partial file may keep the final name");
        assert!(!dir.path().join("artifact.bin.part").exists());
    }

    /// A `Content-Length` over the cap fails before a single byte is written.
    #[tokio::test]
    async fn a_declared_size_over_the_cap_fails_early() {
        let dir = crate::test_support::temp_tree("download-declared");
        let dest = dir.path().join("artifact.bin");
        let (url, server) = serve_once(vec![b'x'; 4096], true).await;

        let err = download_to_file(&reqwest::Url::parse(&url).unwrap(), &dest, 512, |_, _| {})
            .await
            .unwrap_err();
        let _ = server.await;

        assert!(err.contains("too large"), "{err}");
        assert!(!dir.path().join("artifact.bin.part").exists());
    }
}
