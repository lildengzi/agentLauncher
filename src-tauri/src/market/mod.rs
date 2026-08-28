//! Decentralized extension market.
//!
//! There is no single registry for agent plugins / skills / MCP servers, so the
//! launcher does not pretend otherwise: a **source list** (`~/.agentlauncher/
//! sources.json`) names every index to consult, each with an `adapter` saying what
//! shape its payload is in, and the backend normalises all of them into one
//! `MarketItem` vocabulary before the UI ever sees them. Adding a source is a
//! user-level act; teaching the launcher a new payload shape is an adapter.
//!
//! Fetching lives here rather than in the webview for two reasons: a user-supplied
//! URL cannot be expected to send CORS headers, and the results want a disk cache
//! under `~/.agentlauncher/cache/market/` so the dialog opens offline.
//!
//! Layout: `sources` owns the source list, `http` is the guarded transport every
//! remote source is fetched through, `adapters` turns each source's native payload
//! into the vocabulary below, `cache` persists the last good copy of each so the
//! dialog opens offline, `install` turns a chosen `InstallSpec` into the real per-kind
//! side effect, and this module owns the vocabulary and the query surface.

mod adapters;
mod cache;
mod http;
pub mod install;
pub mod sources;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use serde::{Deserialize, Serialize};

use crate::instance_ext::McpServerEntry;
use sources::SourceDef;

// ---- normalised vocabulary (mirrored in src/types.ts) ---------------------

/// How an item is actually installed. `method` is a string rather than an enum so
/// an unknown method from a newer source degrades to "manual" in the UI instead of
/// failing to deserialize the whole payload.
///
/// * `pnpm-profile` — an npm package added to a dsh profile (`package`).
/// * `git-clone`    — a repo cloned into the instance's `skills/` (`repo`).
/// * `mcp-config`   — an `mcpServers` entry merged into the instance (`mcp`).
/// * `manual`       — nothing the launcher can run; show `command` to copy.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct InstallSpec {
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub package: String,
    #[serde(default)]
    pub repo: String,
    /// A shell command to display (never auto-run) for `manual` items.
    #[serde(default)]
    pub command: String,
    /// Env var names the item needs configured — names only, never values.
    #[serde(default)]
    pub env: Vec<String>,
    /// Prefilled server definition for `mcp-config` items.
    #[serde(default)]
    pub mcp: Option<McpServerEntry>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct MarketVersion {
    pub version: String,
    #[serde(default)]
    pub published_at: String,
    #[serde(default)]
    pub install: InstallSpec,
}

/// One market entry, whatever source it came from. Every field is optional at the
/// wire level: a thin source (a hand-written JSON list) should not have to invent
/// download counts to be listable.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct MarketItem {
    /// `"<source id>:<native id>"` — unique across sources, stable across fetches.
    pub id: String,
    pub source: String,
    /// "plugin" | "skill" | "mcp".
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
    /// Detail-pane Markdown. Often empty in a list payload and filled lazily by
    /// `market_readme`, so the list request stays small.
    #[serde(default)]
    pub readme: String,
    /// lucide icon name for the row avatar.
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub homepage: String,
    #[serde(default)]
    pub repo: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub downloads: u64,
    /// RFC3339, or "" when the source does not say.
    #[serde(default)]
    pub updated_at: String,
    /// Newest first. Empty ⇒ nothing installable; the UI shows it read-only.
    #[serde(default)]
    pub versions: Vec<MarketVersion>,
}

/// Per-source outcome of a fetch, reported alongside the results so a partial
/// failure is visible in the dialog instead of silently shrinking the list.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct SourceStatus {
    pub id: String,
    pub ok: bool,
    pub item_count: usize,
    /// RFC3339 of the copy actually served (cache or network), or "".
    #[serde(default)]
    pub fetched_at: String,
    /// true when the network refresh failed and a cached copy was served.
    #[serde(default)]
    pub stale: bool,
    #[serde(default)]
    pub error: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct MarketQuery {
    /// "plugin" | "skill" | "mcp" — the three dialogs are the same widget with
    /// this one field changed.
    pub kind: String,
    #[serde(default)]
    pub query: String,
    /// Restrict to these source ids; empty ⇒ every enabled source serving `kind`.
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// "relevance" | "downloads" | "updated" | "name".
    #[serde(default)]
    pub sort: String,
    #[serde(default)]
    pub offset: usize,
    /// 0 ⇒ the backend's own page size.
    #[serde(default)]
    pub limit: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct MarketPage {
    pub items: Vec<MarketItem>,
    /// Total matches across sources, for paging (not `items.len()`).
    pub total: usize,
    /// true when any served source was stale — the dialog shows one notice.
    pub stale: bool,
    pub statuses: Vec<SourceStatus>,
}

/// `~/.agentlauncher/cache/market` — one normalised JSON file per source id.
pub fn cache_dir() -> Result<std::path::PathBuf, String> {
    Ok(crate::launcher_config::agentlauncher_root()?
        .join("cache")
        .join("market"))
}

// ---- freshness and the in-process layer -----------------------------------

/// The feed we ship is served with `Cache-Control: max-age=600`. Matching it means a
/// dialog reopened within ten minutes costs nothing, and the freshness policy in force
/// is the source operator's own rather than one we invented.
const FRESH_TTL_SECS: i64 = 600;

/// Page size when a query does not name one.
const DEFAULT_LIMIT: usize = 60;

/// Cursor pages to walk for a paged source. 100 entries a page, so a 2000-item
/// ceiling: enough to be a market, bounded enough that a registry growing tenfold
/// cannot turn one dialog open into a hundred requests.
const MAX_CURSOR_PAGES: usize = 20;

/// One source's items as last loaded. Shared rather than cloned because `market_fetch`
/// runs again on every keystroke in the search box, and re-reading a 4.7k-item cache
/// file per keystroke is precisely the cost this avoids.
#[derive(Clone)]
struct Snapshot {
    url: String,
    adapter: String,
    fetched_at: String,
    items: Arc<Vec<MarketItem>>,
}

fn memory() -> &'static Mutex<HashMap<String, Snapshot>> {
    static MEM: OnceLock<Mutex<HashMap<String, Snapshot>>> = OnceLock::new();
    MEM.get_or_init(Default::default)
}

/// Detail Markdown already fetched this session, so clicking back and forth between
/// two items is not two requests each time. Cleared wholesale past a few hundred
/// entries rather than tracking an eviction order it does not need.
fn readme_memo() -> &'static Mutex<HashMap<String, String>> {
    static MEMO: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    MEMO.get_or_init(Default::default)
}

/// Every lock in this module is taken this way. A panic anywhere under a held lock
/// would otherwise poison it for the rest of the process — one bad payload would
/// disable the market until restart, which is the opposite of degrading gracefully.
fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// Whether a stamp is inside the TTL. An unparseable stamp, or one in the future
/// (a clock that has been moved), counts as not fresh: refetching is the cheap
/// mistake, serving something indefinitely is the expensive one.
fn is_fresh(stamp: &str) -> bool {
    match chrono::DateTime::parse_from_rfc3339(stamp) {
        Ok(then) => {
            let age = (chrono::Utc::now() - then.with_timezone(&chrono::Utc)).num_seconds();
            (0..FRESH_TTL_SECS).contains(&age)
        }
        Err(_) => false,
    }
}

// ---- transport ------------------------------------------------------------

/// Every payload a source has to offer: one document for an `http` row, one per
/// `*.json` file for a `dir` row, one per cursor page for a paged registry.
///
/// Transport (`kind`) and payload shape (`adapter`) are deliberately independent, so a
/// team can drop the same JSON a server would have served into a directory and the
/// same adapter reads both.
async fn collect_payloads(src: &SourceDef) -> Result<Vec<serde_json::Value>, String> {
    match src.kind.as_str() {
        "http" => {
            let url = http::parse_http_url(&src.url)?;
            if src.adapter == "mcp-registry" {
                fetch_cursor_pages(url).await
            } else {
                Ok(vec![fetch_json(&url).await?])
            }
        }
        "dir" => read_dir_payloads(&sources::resolved_dir(src)?),
        other => Err(format!("unknown source kind: {other}")),
    }
}

async fn fetch_json(url: &reqwest::Url) -> Result<serde_json::Value, String> {
    let bytes = http::get_capped(url, http::MAX_FEED_BYTES).await?;
    serde_json::from_slice(&bytes).map_err(|e| format!("payload is not valid JSON: {e}"))
}

/// Walk `metadata.nextCursor` until the source stops offering one.
///
/// The MCP registry answers 30 entries by default and 100 at most, which is not a
/// market — a single page would show the alphabetical first thirtieth of it and look
/// like the rest does not exist. Existing query parameters on the row's URL are kept:
/// they may be the filter the user added the row for.
async fn fetch_cursor_pages(base: reqwest::Url) -> Result<Vec<serde_json::Value>, String> {
    let mut pages = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..MAX_CURSOR_PAGES {
        let mut url = base.clone();
        {
            let mut q = url.query_pairs_mut();
            q.append_pair("limit", "100");
            if let Some(c) = &cursor {
                q.append_pair("cursor", c);
            }
        }
        let page = fetch_json(&url).await?;
        cursor = page
            .get("metadata")
            .and_then(|m| m.get("nextCursor"))
            .and_then(|c| c.as_str())
            .map(str::to_string)
            .filter(|c| !c.is_empty());
        pages.push(page);
        if cursor.is_none() {
            break;
        }
    }
    Ok(pages)
}

/// A directory of item files: every `*.json` directly inside it, filename-ordered so
/// the list is stable between opens.
///
/// This is the source that needs no server at all, so it is forgiving on purpose. A
/// directory that does not exist yet is an empty source, not a broken one — it is
/// created by the user dropping the first file in. One unreadable, oversized or
/// malformed file is skipped with a note on stderr rather than failing the row, since
/// the whole point is that these are edited by hand.
fn read_dir_payloads(dir: &std::path::Path) -> Result<Vec<serde_json::Value>, String> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    let mut files: Vec<std::path::PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .is_some_and(|x| x.eq_ignore_ascii_case("json"))
        })
        .collect();
    files.sort();

    let mut out = Vec::new();
    for path in files {
        if std::fs::metadata(&path).is_ok_and(|m| m.len() as usize > http::MAX_FEED_BYTES) {
            eprintln!("market: skipping oversized {}", path.display());
            continue;
        }
        match std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()))
        {
            Ok(v) => out.push(v),
            Err(e) => eprintln!("market: ignoring {}: {e}", path.display()),
        }
    }
    Ok(out)
}

// ---- loading one source ---------------------------------------------------

/// The newest copy of `src` we already hold: process memory first, then disk.
///
/// Both are keyed by the row's `url` and `adapter`, so a re-pointed row reads as no
/// copy at all rather than quietly serving the previous host's items.
fn snapshot(src: &SourceDef) -> Option<Snapshot> {
    if let Some(s) = lock(memory()).get(&src.id) {
        if s.url == src.url && s.adapter == src.adapter {
            return Some(s.clone());
        }
    }
    let entry = cache::read(&src.id, &src.url, &src.adapter)?;
    let snap = Snapshot {
        url: src.url.clone(),
        adapter: src.adapter.clone(),
        fetched_at: entry.fetched_at,
        items: Arc::new(entry.items),
    };
    lock(memory()).insert(src.id.clone(), snap.clone());
    Some(snap)
}

async fn fetch_source(src: &SourceDef) -> Result<Vec<MarketItem>, String> {
    let payloads = collect_payloads(src).await?;
    let mut items = Vec::new();
    for payload in &payloads {
        items.extend(adapters::normalise(&src.adapter, &src.id, payload)?);
    }
    Ok(items)
}

fn ok_status(src: &SourceDef, snap: &Snapshot, stale: bool, error: String) -> SourceStatus {
    SourceStatus {
        id: src.id.clone(),
        ok: !stale,
        item_count: snap.items.len(),
        fetched_at: snap.fetched_at.clone(),
        stale,
        error,
    }
}

/// Items for one source, plus the status the dialog shows next to it.
///
/// This never returns an error, which is the whole reason `SourceStatus` exists: a
/// source that cannot be reached becomes a row saying why, alongside the sources that
/// did work. Three outcomes:
///
/// * a copy inside the TTL, or a successful fetch → `ok`, not stale;
/// * a failed fetch with a cached copy → `ok: false` with the error, the cached items
///   still served and `stale: true` so the dialog can say where they came from. The
///   row reads "failed · 42 items · last fetched …", which is exactly the truth;
/// * a failed fetch with nothing cached → `ok: false`, no items.
async fn load_source(src: &SourceDef, force: bool) -> (Arc<Vec<MarketItem>>, SourceStatus) {
    let cached = if force { None } else { snapshot(src) };
    if let Some(snap) = &cached {
        if is_fresh(&snap.fetched_at) {
            return (snap.items.clone(), ok_status(src, snap, false, String::new()));
        }
    }
    match fetch_source(src).await {
        Ok(items) => {
            let fetched_at = now_rfc3339();
            // A cache we could not write costs freshness next time, not this answer.
            if let Err(e) = cache::write(&src.id, &src.url, &src.adapter, &fetched_at, &items) {
                eprintln!("market: could not cache {}: {e}", src.id);
            }
            let snap = Snapshot {
                url: src.url.clone(),
                adapter: src.adapter.clone(),
                fetched_at,
                items: Arc::new(items),
            };
            lock(memory()).insert(src.id.clone(), snap.clone());
            let status = ok_status(src, &snap, false, String::new());
            (snap.items.clone(), status)
        }
        Err(error) => {
            // `force` skipped the lookup above, but a failed forced refresh should
            // still be able to keep showing what was already there.
            match cached.or_else(|| snapshot(src)) {
                Some(snap) => {
                    let status = ok_status(src, &snap, true, error);
                    (snap.items.clone(), status)
                }
                None => (
                    Arc::new(Vec::new()),
                    SourceStatus {
                        id: src.id.clone(),
                        ok: false,
                        error,
                        ..Default::default()
                    },
                ),
            }
        }
    }
}

/// Load several sources at once, results in the order given.
///
/// Concurrent because the timeouts are the point: three sources, one of which is
/// unreachable, should cost the dialog one connect timeout rather than three in a row.
/// A task that panics is reported as that source failing — a bug in one adapter must
/// not take the whole fetch with it.
async fn load_all(srcs: &[SourceDef], force: bool) -> Vec<(Arc<Vec<MarketItem>>, SourceStatus)> {
    let mut set = tokio::task::JoinSet::new();
    for (i, src) in srcs.iter().enumerate() {
        let src = src.clone();
        set.spawn(async move { (i, load_source(&src, force).await) });
    }
    let mut slots: Vec<Option<(Arc<Vec<MarketItem>>, SourceStatus)>> = vec![None; srcs.len()];
    while let Some(res) = set.join_next().await {
        if let Ok((i, loaded)) = res {
            slots[i] = Some(loaded);
        }
    }
    srcs.iter()
        .zip(slots)
        .map(|(src, slot)| {
            slot.unwrap_or_else(|| {
                (
                    Arc::new(Vec::new()),
                    SourceStatus {
                        id: src.id.clone(),
                        ok: false,
                        error: "this source's loader crashed".into(),
                        ..Default::default()
                    },
                )
            })
        })
        .collect()
}

// ---- search and sort ------------------------------------------------------

/// How well one item answers `q` (already lowercased and non-empty). Zero means it
/// does not — the filter and the sort therefore share one function, so a row can never
/// be listed as a match and then ordered as if it were not one.
///
/// Substring-first rather than fuzzy. The layer this replaces ran Fuse.js in the
/// webview and had to special-case a plain substring hit straight back to the top,
/// because normalising by match-length ratio buried long repository names.
fn relevance(item: &MarketItem, q: &str) -> u32 {
    let name = item.name.to_lowercase();
    if name == q {
        return 1000;
    }
    if name.starts_with(q) {
        return 800;
    }
    if name.contains(q) {
        return 600;
    }
    if item.author.to_lowercase().contains(q) || item.id.to_lowercase().contains(q) {
        return 400;
    }
    if item.description.to_lowercase().contains(q) {
        return 200;
    }
    if item.tags.iter().any(|t| t.to_lowercase().contains(q)) {
        return 100;
    }
    0
}

fn sort_items(items: &mut [MarketItem], sort: &str, q: &str) {
    match sort {
        "downloads" => items.sort_by(|a, b| {
            b.downloads
                .cmp(&a.downloads)
                .then_with(|| a.name.cmp(&b.name))
        }),
        // RFC3339 at a fixed offset sorts lexicographically, and every feed read here
        // stamps UTC. Parsing thousands of dates per keystroke to gain nothing is the
        // trade being declined.
        "updated" => items.sort_by(|a, b| {
            b.updated_at
                .cmp(&a.updated_at)
                .then_with(|| a.name.cmp(&b.name))
        }),
        "name" => items.sort_by_key(|i| i.name.to_lowercase()),
        // "relevance", and anything unrecognised: matched text first, then popularity,
        // which is also the right order for an empty query.
        _ => items.sort_by(|a, b| {
            let (ra, rb) = if q.is_empty() {
                (0, 0)
            } else {
                (relevance(a, q), relevance(b, q))
            };
            rb.cmp(&ra)
                .then_with(|| b.downloads.cmp(&a.downloads))
                .then_with(|| a.name.cmp(&b.name))
        }),
    }
}

// ---- commands -------------------------------------------------------------

/// Query every enabled source that serves `query.kind`, merged and sorted.
///
/// Read-through: each source answers from memory, then disk, then the network, and
/// every source reports its own outcome in `statuses`. A half-broken source list still
/// returns the sources that worked, which is why nothing in here propagates a
/// per-source failure up as this command's error — only a `sources.json` that cannot be
/// read at all is a failure of the whole query.
#[tauri::command]
pub async fn market_fetch(query: MarketQuery) -> Result<MarketPage, String> {
    let doc = sources::load()?;
    let wanted: Vec<SourceDef> = doc
        .sources
        .into_iter()
        .filter(|s| s.enabled && s.serves(&query.kind))
        // An empty `sources` means every candidate; a non-empty one is the dialog's
        // own source tabs narrowing the same query.
        .filter(|s| query.sources.is_empty() || query.sources.contains(&s.id))
        .collect();

    let q = query.query.trim().to_lowercase();
    let tags: Vec<String> = query.tags.iter().map(|t| t.to_lowercase()).collect();
    let mut statuses: Vec<SourceStatus> = Vec::with_capacity(wanted.len());
    let mut matched: Vec<MarketItem> = Vec::new();
    for (items, status) in load_all(&wanted, false).await {
        for item in items.iter() {
            if !query.kind.is_empty() && item.kind != query.kind {
                continue;
            }
            // Tags are an AND: each additional tag narrows, the way the Prism
            // marketplace's filter checkboxes do.
            if !tags
                .iter()
                .all(|t| item.tags.iter().any(|x| x.to_lowercase() == *t))
            {
                continue;
            }
            if !q.is_empty() && relevance(item, &q) == 0 {
                continue;
            }
            matched.push(item.clone());
        }
        statuses.push(status);
    }

    sort_items(&mut matched, &query.sort, &q);
    let total = matched.len();
    let stale = statuses.iter().any(|s| s.stale);
    let limit = if query.limit == 0 {
        DEFAULT_LIMIT
    } else {
        query.limit
    };
    Ok(MarketPage {
        items: matched.into_iter().skip(query.offset).take(limit).collect(),
        total,
        stale,
        statuses,
    })
}

/// Force a refetch, bypassing the cache. `source_id` = None refreshes all.
///
/// A named row is refreshed even when it is disabled: the Settings section's per-row
/// button is how a user finds out whether a row they have just typed works, and making
/// them enable it first to find out has the order backwards. `None` — the "refresh
/// everything" path — sticks to enabled rows, since a disabled row is one the user has
/// said they do not want fetched.
#[tauri::command]
pub async fn market_refresh(source_id: Option<String>) -> Result<Vec<SourceStatus>, String> {
    let doc = sources::load()?;
    let wanted: Vec<SourceDef> = match &source_id {
        Some(id) => doc.sources.into_iter().filter(|s| &s.id == id).collect(),
        None => doc.sources.into_iter().filter(|s| s.enabled).collect(),
    };
    Ok(load_all(&wanted, true)
        .await
        .into_iter()
        .map(|(_, status)| status)
        .collect())
}

/// Fetch one item's detail Markdown for the right-hand pane, lazily.
///
/// `item_id` is `"<source>:<native id>"`, so its first half says whose item this is.
/// Three tiers, cheapest first: the memo, then whatever the list payload already
/// carried, then the item's own repository README — which is what "lazily" is for, and
/// why a list payload is not obliged to ship 4.7k READMEs to be listable.
///
/// Every failure returns `""` rather than an error. The pane already has a "no
/// description" state, and a red error banner for a repository that simply has no
/// README would be noise about something the user cannot act on.
#[tauri::command]
pub async fn market_readme(item_id: String) -> Result<String, String> {
    if let Some(hit) = lock(readme_memo()).get(&item_id) {
        return Ok(hit.clone());
    }
    let Some((source_id, _)) = item_id.split_once(':') else {
        return Ok(String::new());
    };
    let doc = sources::load()?;
    let Some(src) = doc.sources.iter().find(|s| s.id == source_id) else {
        return Ok(String::new());
    };
    // Cache-first: opening a detail pane must not trigger a refetch of the list.
    let (items, _) = load_source(src, false).await;
    let Some(item) = items.iter().find(|i| i.id == item_id) else {
        return Ok(String::new());
    };
    if !item.readme.trim().is_empty() {
        return Ok(item.readme.clone());
    }

    // Nothing inline. `github_readme_url` resolves only a plain
    // `github.com/<owner>/<repo>`, so a `repo` string out of an untrusted feed cannot
    // steer this at another host — and lossy decoding means one stray byte blanks a
    // character rather than the pane.
    let text = match http::github_readme_url(&item.repo) {
        Some(url) => http::get_capped(&url, http::MAX_README_BYTES)
            .await
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .unwrap_or_default(),
        None => String::new(),
    };
    let mut memo = lock(readme_memo());
    if memo.len() > 256 {
        memo.clear();
    }
    memo.insert(item_id, text.clone());
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{temp_tree, EnvGuard, HOME_LOCK};
    use sources::SourcesDoc;

    /// These tests drive the async commands from a plain `#[test]` on purpose. They
    /// hold `HOME_LOCK` while a throwaway `HOME` is in place, and a guard held across
    /// an `.await` is both a clippy warning and a real hazard once the runtime is free
    /// to run something else on the thread.
    fn block_on<F: std::future::Future>(f: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime")
            .block_on(f)
    }

    /// `.invalid` is reserved by RFC 2606, so a row pointing at it is guaranteed to
    /// fail rather than reach somebody's server during a test run.
    const UNREACHABLE: &str = "https://market.invalid/items.json";

    fn row(id: &str, kind: &str, url: &str) -> SourceDef {
        SourceDef {
            id: id.into(),
            label: id.into(),
            kind: kind.into(),
            url: url.into(),
            adapter: "agentlauncher".into(),
            kinds: vec!["plugin".into()],
            enabled: true,
            builtin: false,
        }
    }

    fn only_source(src: SourceDef) {
        // `save` re-seeds the built-ins disabled, so the row under test is the only
        // one a fetch will consider.
        sources::save(SourcesDoc {
            format_version: 1,
            sources: vec![src],
        })
        .unwrap();
    }

    fn plugin_query(query: &str, sort: &str, offset: usize, limit: usize) -> MarketQuery {
        MarketQuery {
            kind: "plugin".into(),
            query: query.into(),
            sort: sort.into(),
            offset,
            limit,
            ..Default::default()
        }
    }

    fn cached_item(id: &str, name: &str) -> MarketItem {
        MarketItem {
            id: id.to_string(),
            source: id.split(':').next().unwrap_or_default().to_string(),
            kind: "plugin".into(),
            name: name.to_string(),
            ..Default::default()
        }
    }

    /// The read-through path: a cache written inside the TTL answers the dialog, and
    /// the unreachable URL proves no network call was needed to do it.
    #[test]
    fn a_fresh_cache_answers_without_the_network() {
        let _g = HOME_LOCK.lock().unwrap();
        let tree = temp_tree("market-fresh");
        let _home = EnvGuard::set("HOME", tree.path());

        only_source(row("team-fresh", "http", UNREACHABLE));
        cache::write(
            "team-fresh",
            UNREACHABLE,
            "agentlauncher",
            &now_rfc3339(),
            &[
                cached_item("team-fresh:a", "Alpha"),
                cached_item("team-fresh:b", "Beta"),
            ],
        )
        .unwrap();

        let page = block_on(market_fetch(plugin_query("", "name", 0, 0))).unwrap();
        assert_eq!(page.total, 2);
        assert_eq!(page.items.len(), 2);
        assert!(!page.stale);
        assert_eq!(page.statuses.len(), 1);
        assert!(page.statuses[0].ok, "{:?}", page.statuses[0].error);
        assert_eq!(page.statuses[0].item_count, 2);
        assert!(!page.statuses[0].fetched_at.is_empty());
    }

    /// Offline, with only an expired cache: the dialog still opens, the items are the
    /// cached ones, and the row says the refresh failed rather than pretending it did
    /// not happen.
    #[test]
    fn an_expired_cache_opens_the_dialog_stale() {
        let _g = HOME_LOCK.lock().unwrap();
        let tree = temp_tree("market-stale");
        let _home = EnvGuard::set("HOME", tree.path());

        only_source(row("team-stale", "http", UNREACHABLE));
        cache::write(
            "team-stale",
            UNREACHABLE,
            "agentlauncher",
            "2020-01-01T00:00:00Z",
            &[cached_item("team-stale:a", "Alpha")],
        )
        .unwrap();

        let page = block_on(market_fetch(plugin_query("", "", 0, 0))).unwrap();
        assert_eq!(page.items.len(), 1, "cached items are still served");
        assert!(page.stale, "the dialog gets one notice for the whole page");
        let st = &page.statuses[0];
        assert!(!st.ok && st.stale);
        assert_eq!(st.item_count, 1);
        assert_eq!(st.fetched_at, "2020-01-01T00:00:00Z", "the copy's own stamp");
        assert!(!st.error.is_empty(), "and why it could not be refreshed");
    }

    /// The server-less source. Three of the shapes a hand-edited directory really
    /// contains, one of them broken, plus a file that is not JSON at all.
    #[test]
    fn a_dir_source_reads_its_files_and_survives_a_bad_one() {
        let _g = HOME_LOCK.lock().unwrap();
        let tree = temp_tree("market-dir");
        let _home = EnvGuard::set("HOME", tree.path());
        let dir = tree.path().join("drop-in");
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(
            dir.join("a-list.json"),
            r#"{"items":[
                 {"id":"pg","name":"Postgres Inspector","kind":"plugin","downloads":10,
                  "tags":["Database"],"description":"Inspect a database."},
                 {"id":"web","name":"Web Fetch","kind":"plugin","downloads":30}
               ]}"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("b-single.json"),
            r#"{"id":"solo","name":"Solo","kind":"plugin","downloads":20}"#,
        )
        .unwrap();
        // Truncated mid-write, the way a killed editor leaves a file.
        std::fs::write(dir.join("c-broken.json"), r#"{"items":[{"id":"x""#).unwrap();
        std::fs::write(dir.join("notes.txt"), "not a payload").unwrap();

        only_source(row("drop-in", "dir", &dir.to_string_lossy()));

        let page = block_on(market_fetch(plugin_query("", "name", 0, 0))).unwrap();
        assert!(page.statuses[0].ok, "{:?}", page.statuses[0].error);
        assert_eq!(page.total, 3, "the broken file costs itself, not the source");
        assert_eq!(
            page.items.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(),
            vec!["Postgres Inspector", "Solo", "Web Fetch"]
        );
        assert_eq!(page.items[0].id, "drop-in:pg", "ids are source-qualified");

        // Reading the directory wrote the cache the next open will serve.
        let entry = cache::read("drop-in", &dir.to_string_lossy(), "agentlauncher")
            .expect("cache written after a successful load");
        assert_eq!(entry.items.len(), 3);
        assert!(is_fresh(&entry.fetched_at));

        // Search, tags and paging all run over the same merged list.
        let hit = block_on(market_fetch(plugin_query("postgres", "", 0, 0))).unwrap();
        assert_eq!(hit.total, 1);
        assert_eq!(hit.items[0].id, "drop-in:pg");

        let tagged = block_on(market_fetch(MarketQuery {
            tags: vec!["database".into()],
            ..plugin_query("", "", 0, 0)
        }))
        .unwrap();
        assert_eq!(tagged.total, 1, "tag matching ignores case");

        let by_downloads = block_on(market_fetch(plugin_query("", "downloads", 0, 2))).unwrap();
        assert_eq!(by_downloads.total, 3, "total counts matches, not the page");
        assert_eq!(
            by_downloads
                .items
                .iter()
                .map(|i| i.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Web Fetch", "Solo"]
        );
        let tail = block_on(market_fetch(plugin_query("", "downloads", 2, 2))).unwrap();
        assert_eq!(tail.items.len(), 1);
        assert_eq!(tail.items[0].name, "Postgres Inspector");
    }

    /// A row that cannot be reached and has never been cached reports itself as failed,
    /// and `market_refresh` reports the row it was asked about even though it is off —
    /// that is the Settings section's "does this URL work?" button.
    #[test]
    fn refresh_reports_a_disabled_row_it_was_asked_about() {
        let _g = HOME_LOCK.lock().unwrap();
        let tree = temp_tree("market-refresh");
        let _home = EnvGuard::set("HOME", tree.path());

        let mut src = row("team-off", "http", UNREACHABLE);
        src.enabled = false;
        only_source(src);

        let statuses = block_on(market_refresh(Some("team-off".into()))).unwrap();
        assert_eq!(statuses.len(), 1);
        assert!(!statuses[0].ok);
        assert!(!statuses[0].stale, "nothing cached, so nothing was served");
        assert_eq!(statuses[0].item_count, 0);
        assert!(!statuses[0].error.is_empty());

        // The blanket refresh leaves disabled rows alone.
        let all = block_on(market_refresh(None)).unwrap();
        assert!(all.is_empty());
    }

    /// A source whose row names an adapter nobody taught us fails alone, with a message
    /// the Settings row can show.
    #[test]
    fn an_unknown_adapter_is_one_rows_error() {
        let _g = HOME_LOCK.lock().unwrap();
        let tree = temp_tree("market-adapter");
        let _home = EnvGuard::set("HOME", tree.path());
        let dir = tree.path().join("drop-in");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.json"), r#"{"id":"a","name":"A","kind":"plugin"}"#).unwrap();

        let mut src = row("weird", "dir", &dir.to_string_lossy());
        src.adapter = "some-future-shape".into();
        only_source(src);

        let page = block_on(market_fetch(plugin_query("", "", 0, 0))).unwrap();
        assert!(page.items.is_empty());
        assert!(!page.statuses[0].ok);
        assert!(
            page.statuses[0].error.contains("unknown payload adapter"),
            "{}",
            page.statuses[0].error
        );
    }

    /// `market_readme` degrades to the pane's own empty state for anything it cannot
    /// resolve, and never reaches the network for an id it does not recognise.
    #[test]
    fn readme_is_empty_rather_than_an_error_when_it_cannot_be_found() {
        let _g = HOME_LOCK.lock().unwrap();
        let tree = temp_tree("market-readme");
        let _home = EnvGuard::set("HOME", tree.path());

        only_source(row("team-readme", "http", UNREACHABLE));
        cache::write(
            "team-readme",
            UNREACHABLE,
            "agentlauncher",
            &now_rfc3339(),
            &[MarketItem {
                readme: "# Alpha\n\nprose".into(),
                ..cached_item("team-readme:a", "Alpha")
            }],
        )
        .unwrap();

        assert_eq!(
            block_on(market_readme("team-readme:a".into())).unwrap(),
            "# Alpha\n\nprose",
            "an inline readme is served without a request"
        );
        assert_eq!(block_on(market_readme("no-such-source:x".into())).unwrap(), "");
        assert_eq!(block_on(market_readme("team-readme:missing".into())).unwrap(), "");
        assert_eq!(block_on(market_readme("unqualified".into())).unwrap(), "");
    }
}
