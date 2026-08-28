//! The source list — `~/.agentlauncher/sources.json`.
//!
//! This is the file that makes the market decentralized: nothing is hardcoded
//! into the dialog, every index the launcher consults is a row here, and a user
//! can add their own (a team's internal JSON, a checkout on disk) or disable ours.
//!
//! Built-in rows are seeded on first read and marked `builtin`: they can be
//! disabled or re-pointed but not deleted, so "I removed the defaults and now the
//! market is empty forever" is not a reachable state.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

fn current_format_version() -> u32 {
    1
}

/// One index to consult. `adapter` names the payload shape, which is what lets a
/// third-party feed join without the dialog knowing anything about it.
///
/// * `kind` — `"http"` (fetch `url`) or `"dir"` (read `*.json` under `url`).
/// * `adapter` — `"agentlauncher"` (our canonical `{items:[MarketItem]}` shape),
///   or a named third-party shape (`"dsh-market"`, `"mcp-registry"`, `"npm"`).
/// * `kinds` — which of plugin / skill / mcp this source can answer for; a source
///   is skipped entirely for dialogs it cannot serve.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SourceDef {
    pub id: String,
    pub label: String,
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default)]
    pub url: String,
    #[serde(default = "default_adapter")]
    pub adapter: String,
    #[serde(default)]
    pub kinds: Vec<String>,
    #[serde(default)]
    pub enabled: bool,
    /// Shipped with the launcher: disable-able, not delete-able.
    #[serde(default)]
    pub builtin: bool,
}

fn default_kind() -> String {
    "http".into()
}
fn default_adapter() -> String {
    "agentlauncher".into()
}

impl SourceDef {
    pub fn serves(&self, kind: &str) -> bool {
        kind.is_empty() || self.kinds.iter().any(|k| k == kind)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SourcesDoc {
    #[serde(default = "current_format_version")]
    pub format_version: u32,
    #[serde(default)]
    pub sources: Vec<SourceDef>,
}

impl Default for SourcesDoc {
    fn default() -> Self {
        Self {
            format_version: current_format_version(),
            sources: builtins(),
        }
    }
}

/// The rows every install starts with.
///
/// Deliberately short. A default that 404s teaches users the market is broken, so a
/// row only ships enabled once its endpoint and payload shape have been checked
/// against the live service — which is now true of all three:
///
/// * `dsh-market` — `GET /plugins.json` answers 200 with `schemaVersion: 2` and a
///   `plugins` array (4753 entries at the time of writing, `Cache-Control: max-age=600`,
///   which is where `FRESH_TTL_SECS` comes from).
/// * `local` — a directory read, so it cannot 404; a missing directory is an empty
///   list rather than a failure.
/// * `mcp-registry` — `GET /v0/servers?limit=100` answers 200 with `servers[]` and a
///   `metadata.nextCursor`, schema `2025-12-11`; entries are `{server, _meta}` and the
///   packages we can actually install are the `transport.type == "stdio"` npm/pypi ones.
///   The `mcp-registry` adapter is written against that shape, so the row ships on.
pub fn builtins() -> Vec<SourceDef> {
    vec![
        SourceDef {
            id: "dsh-market".into(),
            label: "dsh.market".into(),
            kind: "http".into(),
            url: "https://dsh.market/plugins.json".into(),
            adapter: "dsh-market".into(),
            kinds: vec!["plugin".into(), "skill".into()],
            enabled: true,
            builtin: true,
        },
        SourceDef {
            id: "local".into(),
            label: "Local".into(),
            kind: "dir".into(),
            // Empty ⇒ `~/.agentlauncher/sources` (see `resolved_dir`).
            url: String::new(),
            adapter: "agentlauncher".into(),
            kinds: vec!["plugin".into(), "skill".into(), "mcp".into()],
            enabled: true,
            builtin: true,
        },
        SourceDef {
            id: "mcp-registry".into(),
            label: "MCP Registry".into(),
            kind: "http".into(),
            // No `limit` here on purpose: the adapter's paging appends its own, and a
            // user who re-points this row keeps whatever query they typed.
            url: "https://registry.modelcontextprotocol.io/v0/servers".into(),
            adapter: "mcp-registry".into(),
            kinds: vec!["mcp".into()],
            enabled: true,
            builtin: true,
        },
    ]
}

// ---- io -------------------------------------------------------------------

fn sources_path() -> Result<PathBuf, String> {
    Ok(crate::launcher_config::agentlauncher_root()?.join("sources.json"))
}

/// Where a `dir` source actually reads from: its `url`, or the default drop-in
/// directory when blank.
pub fn resolved_dir(src: &SourceDef) -> Result<PathBuf, String> {
    if src.url.trim().is_empty() {
        Ok(crate::launcher_config::agentlauncher_root()?.join("sources"))
    } else {
        Ok(PathBuf::from(src.url.trim()))
    }
}

/// Read the list, re-seeding any built-in row a previous version had not written
/// yet. New built-ins therefore appear on upgrade without clobbering user edits:
/// an existing row wins over the shipped one, including its `enabled` flag.
pub fn load() -> Result<SourcesDoc, String> {
    let mut doc: SourcesDoc = crate::launcher_config::read_or_default(&sources_path()?);
    for b in builtins() {
        if !doc.sources.iter().any(|s| s.id == b.id) {
            doc.sources.push(b);
        }
    }
    Ok(doc)
}

/// Persist the list. Built-ins that the caller dropped are restored (disabled if
/// the caller had disabled them), and ids are de-duplicated last-write-wins, so a
/// malformed payload from the UI cannot produce a list that behaves oddly later.
pub fn save(mut doc: SourcesDoc) -> Result<(), String> {
    doc.format_version = current_format_version();
    let mut seen: Vec<String> = Vec::new();
    doc.sources.retain(|s| {
        let keep = !s.id.trim().is_empty() && !seen.contains(&s.id);
        if keep {
            seen.push(s.id.clone());
        }
        keep
    });
    for b in builtins() {
        if !doc.sources.iter().any(|s| s.id == b.id) {
            doc.sources.push(SourceDef {
                enabled: false,
                ..b
            });
        }
    }
    for s in &mut doc.sources {
        // `builtin` is ours to assert, not the frontend's to claim.
        s.builtin = builtins().iter().any(|b| b.id == s.id);
    }
    crate::launcher_config::write_json_atomic(&sources_path()?, &doc)
}

// ---- commands -------------------------------------------------------------

#[tauri::command]
pub fn get_market_sources() -> Result<SourcesDoc, String> {
    load()
}

#[tauri::command]
pub fn set_market_sources(doc: SourcesDoc) -> Result<(), String> {
    save(doc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{temp_tree, EnvGuard, HOME_LOCK};

    #[test]
    fn missing_file_yields_the_builtins() {
        let _g = HOME_LOCK.lock().unwrap();
        let tree = temp_tree("sources-empty");
        let _home = EnvGuard::set("HOME", tree.path());

        let doc = load().unwrap();
        assert_eq!(doc.format_version, 1);
        assert!(doc.sources.iter().all(|s| s.builtin));
        assert!(doc.sources.iter().all(|s| s.enabled), "every shipped row is verified");
        assert!(doc.sources.iter().any(|s| s.id == "dsh-market" && s.enabled));
        // Only sources declaring a kind answer that dialog.
        let mcp: Vec<&str> = doc
            .sources
            .iter()
            .filter(|s| s.serves("mcp"))
            .map(|s| s.id.as_str())
            .collect();
        assert_eq!(mcp, vec!["local", "mcp-registry"]);
    }

    #[test]
    fn builtins_survive_deletion_but_keep_their_disabled_state() {
        let _g = HOME_LOCK.lock().unwrap();
        let tree = temp_tree("sources-save");
        let _home = EnvGuard::set("HOME", tree.path());

        // A user deletes every builtin and adds one of their own, and tries to
        // pass their row off as shipped.
        save(SourcesDoc {
            format_version: 99,
            sources: vec![SourceDef {
                id: "team".into(),
                label: "Team feed".into(),
                kind: "http".into(),
                url: "https://example.invalid/items.json".into(),
                adapter: "agentlauncher".into(),
                kinds: vec!["mcp".into()],
                enabled: true,
                builtin: true,
            }],
        })
        .unwrap();

        let doc = load().unwrap();
        assert_eq!(doc.format_version, 1, "version is ours, not the caller's");
        let team = doc.sources.iter().find(|s| s.id == "team").unwrap();
        assert!(!team.builtin, "a user row cannot claim to be builtin");
        // Restored, but not silently re-enabled.
        let dsh = doc.sources.iter().find(|s| s.id == "dsh-market").unwrap();
        assert!(dsh.builtin && !dsh.enabled);
    }

    #[test]
    fn a_dir_source_defaults_to_the_drop_in_directory() {
        let _g = HOME_LOCK.lock().unwrap();
        let tree = temp_tree("sources-dir");
        let _home = EnvGuard::set("HOME", tree.path());

        let local = builtins().into_iter().find(|s| s.id == "local").unwrap();
        assert_eq!(
            resolved_dir(&local).unwrap(),
            tree.path().join(".agentlauncher").join("sources")
        );
    }
}
