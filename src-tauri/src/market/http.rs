//! The guarded transport every remote source is fetched through.
//!
//! A source URL is typed by the user and its payload is written by whoever runs
//! that host, so this module treats both as hostile input. Nothing here knows what
//! a market item is; it only answers "give me at most N bytes from this URL, or an
//! error" — which is the whole reason the fetch moved out of the webview, where
//! neither the timeout nor the byte cap was ours to set.
//!
//! Four guards, each closing a way one bad feed could hurt the launcher:
//!   * **scheme allowlist** — `http`/`https` only, so a source row can never turn
//!     into a local-file read;
//!   * **redirect policy** — the same allowlist applied again on every hop, because
//!     a feed that answers `302 Location: file:///…` is asking for exactly that;
//!   * **timeouts** — connect *and* whole-response, so a host that accepts the
//!     connection and then says nothing cannot wedge a dialog open forever;
//!   * **byte cap** — counted while streaming, so the cap is reached before the
//!     memory is, gzip-decompressed size included.

use std::sync::OnceLock;
use std::time::Duration;

/// Enough headroom over the 11.6 MB feed we ship that growth does not break it,
/// while still bounding what one source can cost us.
pub const MAX_FEED_BYTES: usize = 48 * 1024 * 1024;

/// A detail README is prose. A megabyte of it is already far more than the pane
/// will ever render.
pub const MAX_README_BYTES: usize = 1024 * 1024;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(45);

fn scheme_ok(url: &reqwest::Url) -> bool {
    matches!(url.scheme(), "http" | "https")
}

/// One pooled client for the whole process: a market dialog refetches several
/// sources from the same hosts, and a fresh client per request would throw away
/// the TLS session and connection pool each time.
fn client() -> Result<&'static reqwest::Client, String> {
    static CLIENT: OnceLock<Result<reqwest::Client, String>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .connect_timeout(CONNECT_TIMEOUT)
                .timeout(REQUEST_TIMEOUT)
                // A feed does not get to choose the scheme we end up on.
                .redirect(reqwest::redirect::Policy::custom(|attempt| {
                    if !scheme_ok(attempt.url()) {
                        let msg =
                            format!("refusing redirect to non-http(s) URL: {}", attempt.url());
                        return attempt.error(msg);
                    }
                    if attempt.previous().len() >= 5 {
                        return attempt.error("too many redirects");
                    }
                    attempt.follow()
                }))
                // Some hosts reject an empty User-Agent outright, and an honest one
                // tells a feed operator who is calling.
                .user_agent(concat!("agentlauncher/", env!("CARGO_PKG_VERSION")))
                .build()
                .map_err(|e| format!("cannot build HTTP client: {e}"))
        })
        .as_ref()
        .map_err(|e| e.clone())
}

/// Validate a source URL before it is ever handed to the client, so "this row is
/// misconfigured" is reported as such instead of as a network failure.
pub fn parse_http_url(raw: &str) -> Result<reqwest::Url, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("this source has no URL".into());
    }
    let url = reqwest::Url::parse(trimmed).map_err(|e| format!("invalid URL: {e}"))?;
    if !scheme_ok(&url) {
        return Err(format!(
            "only http and https sources are supported, not {}:",
            url.scheme()
        ));
    }
    Ok(url)
}

/// GET `url`, refusing to hold more than `cap` bytes of its body.
///
/// The body is streamed rather than buffered by `reqwest` so the cap is enforced
/// as bytes arrive: a host that promises 2 KB and then sends forever is cut off at
/// the cap instead of after it has already been allocated. `Content-Length`, when
/// present, is checked first purely to fail fast.
pub async fn get_capped(url: &reqwest::Url, cap: usize) -> Result<Vec<u8>, String> {
    let res = client()?
        .get(url.clone())
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    if !res.status().is_success() {
        return Err(format!("HTTP {}", res.status().as_u16()));
    }
    if let Some(len) = res.content_length() {
        if len as usize > cap {
            return Err(format!("response too large ({len} bytes, cap {cap})"));
        }
    }

    // `reqwest`'s own timeout has already covered the handshake and headers; this
    // one bounds the body, which is where a slow-drip host would otherwise sit.
    let read = async {
        let mut res = res;
        let mut buf: Vec<u8> = Vec::new();
        while let Some(chunk) = res
            .chunk()
            .await
            .map_err(|e| format!("read failed: {e}"))?
        {
            if buf.len() + chunk.len() > cap {
                return Err(format!("response exceeded {cap} bytes"));
            }
            buf.extend_from_slice(&chunk);
        }
        Ok(buf)
    };
    tokio::time::timeout(REQUEST_TIMEOUT, read)
        .await
        .map_err(|_| "timed out while reading the response".to_string())?
}

/// `raw.githubusercontent.com` URL for a GitHub repo's README on its default branch.
///
/// Returns `None` for anything that is not exactly `github.com/<owner>/<repo>`, so a
/// `repo` field out of an untrusted feed cannot steer this at another host or at a
/// deeper path: the host is compared literally and only two path segments are used.
pub fn github_readme_url(repo: &str) -> Option<reqwest::Url> {
    let url = reqwest::Url::parse(repo.trim()).ok()?;
    if !scheme_ok(&url) || url.host_str()? != "github.com" {
        return None;
    }
    let mut seg = url.path_segments()?;
    let owner = seg.next()?;
    let name = seg.next()?.trim_end_matches(".git");
    if seg.next().is_some() || owner.is_empty() || name.is_empty() {
        return None;
    }
    reqwest::Url::parse(&format!(
        "https://raw.githubusercontent.com/{owner}/{name}/HEAD/README.md"
    ))
    .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A source row cannot smuggle a local-file read past the transport.
    #[test]
    fn only_http_schemes_are_accepted() {
        assert!(parse_http_url("https://example.invalid/items.json").is_ok());
        assert!(parse_http_url("http://example.invalid/items.json").is_ok());
        for bad in [
            "file:///etc/passwd",
            "ftp://example.invalid/x.json",
            "data:application/json,{}",
            "",
            "   ",
            "not a url",
        ] {
            assert!(parse_http_url(bad).is_err(), "{bad} must be rejected");
        }
    }

    /// A feed's `repo` string only ever becomes a README URL when it is plainly one
    /// GitHub repository.
    #[test]
    fn readme_url_is_derived_only_from_a_plain_github_repo() {
        let u = github_readme_url("https://github.com/owner/name.git").unwrap();
        assert_eq!(
            u.as_str(),
            "https://raw.githubusercontent.com/owner/name/HEAD/README.md"
        );
        for bad in [
            "https://github.com/owner",
            "https://github.com/owner/name/tree/main",
            "https://evil.invalid/owner/name",
            "https://github.com.evil.invalid/owner/name",
            "file:///etc/passwd",
            "",
        ] {
            assert!(github_readme_url(bad).is_none(), "{bad} must not resolve");
        }
    }
}
