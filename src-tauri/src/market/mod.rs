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
//! Layout: `sources` owns the source list, `install` turns a chosen
//! `InstallSpec` into the real per-kind side effect, and this module owns the
//! normalised item vocabulary and the query surface.

pub mod install;
pub mod sources;

use serde::{Deserialize, Serialize};

use crate::instance_ext::McpServerEntry;

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

// ---- commands -------------------------------------------------------------

/// Query every enabled source that serves `query.kind`, merged and sorted.
///
/// SEAM: returns an empty page with one status per candidate source. The fetch /
/// cache / adapter layer is this round's Stream C; the shape above is the contract
/// the download dialog is written against and must not change without updating
/// `src/types.ts` in the same commit.
#[tauri::command]
pub async fn market_fetch(query: MarketQuery) -> Result<MarketPage, String> {
    let doc = sources::load()?;
    let statuses = doc
        .sources
        .iter()
        .filter(|s| s.enabled && s.serves(&query.kind))
        .map(|s| SourceStatus {
            id: s.id.clone(),
            ok: false,
            error: "source fetching not implemented yet".into(),
            ..Default::default()
        })
        .collect();
    Ok(MarketPage {
        items: vec![],
        total: 0,
        stale: false,
        statuses,
    })
}

/// Force a refetch, bypassing the cache. `source_id` = None refreshes all.
///
/// SEAM: reports "not implemented" per source (see `market_fetch`).
#[tauri::command]
pub async fn market_refresh(source_id: Option<String>) -> Result<Vec<SourceStatus>, String> {
    let doc = sources::load()?;
    Ok(doc
        .sources
        .iter()
        .filter(|s| source_id.as_deref().is_none_or(|id| id == s.id))
        .map(|s| SourceStatus {
            id: s.id.clone(),
            ok: false,
            error: "source fetching not implemented yet".into(),
            ..Default::default()
        })
        .collect())
}

/// Fetch one item's detail Markdown for the right-hand pane, lazily.
///
/// SEAM: returns "" so the pane renders its own "no description" state.
#[tauri::command]
pub async fn market_readme(item_id: String) -> Result<String, String> {
    let _ = item_id;
    Ok(String::new())
}
