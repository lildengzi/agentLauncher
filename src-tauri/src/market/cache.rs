//! The on-disk market cache — `~/.agentlauncher/cache/market/<source>.json`.
//!
//! What is cached is the **normalised** item list, not the source's raw payload.
//! That is the difference between a 3 MB cache and an 11.6 MB one for the feed we
//! ship, and it means opening the dialog offline costs one parse of exactly the
//! shape the UI wants.
//!
//! Two properties the market leans on:
//!
//! * **Atomic writes.** `write_json_atomic` writes a sibling temp file and renames,
//!   so a process killed mid-write leaves either the previous cache or the new one —
//!   never a truncated file that the next read would report as a corrupt source.
//! * **The cache remembers what produced it.** A cache entry carries the `url` and
//!   `adapter` it was built from, and a mismatch is treated as no cache at all.
//!   Otherwise re-pointing a source row at a different URL would keep serving the
//!   old host's items until the next successful fetch, which looks like the edit
//!   silently did nothing.
//!
//! A cache that fails to parse degrades to "no cache": `read_or_default` already
//! swallows a malformed file, and an entry with an empty `source` is treated as
//! absent, so one bad file can never poison a fetch.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::MarketItem;

fn current_format_version() -> u32 {
    1
}

/// One source's last good normalised payload.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct CachedSource {
    #[serde(default)]
    pub format_version: u32,
    /// Empty ⇒ this file was missing or unreadable; treated as no cache.
    #[serde(default)]
    pub source: String,
    /// The `url` this copy was fetched from — a changed row invalidates it.
    #[serde(default)]
    pub url: String,
    /// The `adapter` that normalised it — likewise.
    #[serde(default)]
    pub adapter: String,
    /// RFC3339 of the fetch that produced this copy.
    #[serde(default)]
    pub fetched_at: String,
    #[serde(default)]
    pub items: Vec<MarketItem>,
}

/// Filename for a source id.
///
/// Source ids come from the UI and from `sources.json`, so a raw id in a path is a
/// traversal waiting to happen (`../../config`). Everything outside a conservative
/// alphabet collapses to `_`, which can make two ids share a file — harmless, since
/// the entry records its own `source` and a mismatch reads as a miss.
pub fn cache_file_name(source_id: &str) -> String {
    let safe: String = source_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let safe = safe.trim_matches('.');
    if safe.is_empty() {
        "_.json".to_string()
    } else {
        format!("{safe}.json")
    }
}

fn cache_path(source_id: &str) -> Result<PathBuf, String> {
    Ok(super::cache_dir()?.join(cache_file_name(source_id)))
}

/// The cached copy for `src`, or `None` when there is none usable.
pub fn read(source_id: &str, url: &str, adapter: &str) -> Option<CachedSource> {
    let path = cache_path(source_id).ok()?;
    let entry: CachedSource = crate::launcher_config::read_or_default(&path);
    if entry.source != source_id || entry.url != url || entry.adapter != adapter {
        return None;
    }
    Some(entry)
}

/// Replace `source_id`'s cached copy. Failure to write is not fatal: the items were
/// fetched successfully and are about to be shown, so a read-only cache directory
/// should cost freshness on the next open, not the current answer.
pub fn write(
    source_id: &str,
    url: &str,
    adapter: &str,
    fetched_at: &str,
    items: &[MarketItem],
) -> Result<(), String> {
    let entry = CachedSource {
        format_version: current_format_version(),
        source: source_id.to_string(),
        url: url.to_string(),
        adapter: adapter.to_string(),
        fetched_at: fetched_at.to_string(),
        items: items.to_vec(),
    };
    crate::launcher_config::write_json_atomic(&cache_path(source_id)?, &entry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{temp_tree, EnvGuard, HOME_LOCK};

    fn item(id: &str) -> MarketItem {
        MarketItem {
            id: id.to_string(),
            source: "team".into(),
            kind: "plugin".into(),
            name: "Thing".into(),
            ..Default::default()
        }
    }

    /// A user-supplied id never escapes the cache directory.
    #[test]
    fn cache_names_cannot_traverse() {
        assert_eq!(cache_file_name("dsh-market"), "dsh-market.json");
        // Separators are what makes traversal work, so only they have to go: the
        // surviving dots cannot form a path component on their own.
        assert_eq!(cache_file_name("../../etc/passwd"), "_.._etc_passwd.json");
        assert_eq!(cache_file_name(".."), "_.json");
        assert_eq!(cache_file_name(""), "_.json");
        assert_eq!(cache_file_name("a/b"), "a_b.json");
        for id in ["../../etc/passwd", "..", "a/b", "C:\\x", "", "  "] {
            let n = cache_file_name(id);
            assert!(
                !n.contains('/') && !n.contains('\\') && !n.starts_with('.'),
                "{id} produced {n}"
            );
        }
    }

    /// The round trip the offline dialog depends on: items written, then read back
    /// with the `fetched_at` that produced them.
    #[test]
    fn a_written_cache_reads_back() {
        let _g = HOME_LOCK.lock().unwrap();
        let tree = temp_tree("market-cache");
        let _home = EnvGuard::set("HOME", tree.path());

        write(
            "team",
            "https://example.invalid/items.json",
            "agentlauncher",
            "2026-01-01T00:00:00Z",
            &[item("team:a"), item("team:b")],
        )
        .unwrap();

        let got = read("team", "https://example.invalid/items.json", "agentlauncher")
            .expect("cache present");
        assert_eq!(got.format_version, 1);
        assert_eq!(got.fetched_at, "2026-01-01T00:00:00Z");
        assert_eq!(got.items.len(), 2);
        assert_eq!(got.items[0].id, "team:a");
    }

    /// Re-pointing a row must not keep serving the previous host's items.
    #[test]
    fn a_repointed_source_reads_as_a_miss() {
        let _g = HOME_LOCK.lock().unwrap();
        let tree = temp_tree("market-cache-repoint");
        let _home = EnvGuard::set("HOME", tree.path());

        write("team", "https://old.invalid/x.json", "agentlauncher", "t", &[item("team:a")])
            .unwrap();
        assert!(read("team", "https://new.invalid/x.json", "agentlauncher").is_none());
        assert!(read("team", "https://old.invalid/x.json", "dsh-market").is_none());
        assert!(read("other", "https://old.invalid/x.json", "agentlauncher").is_none());
        assert!(read("team", "https://old.invalid/x.json", "agentlauncher").is_some());
    }

    /// A half-written or hand-mangled cache file is a miss, not a failure.
    #[test]
    fn a_corrupt_cache_file_is_a_miss() {
        let _g = HOME_LOCK.lock().unwrap();
        let tree = temp_tree("market-cache-corrupt");
        let _home = EnvGuard::set("HOME", tree.path());

        let dir = super::super::cache_dir().unwrap();
        std::fs::create_dir_all(&dir).unwrap();
        // The shape a kill mid-write would leave if writes were not atomic.
        std::fs::write(dir.join("team.json"), r#"{"source":"team","items":[{"id":"#).unwrap();
        assert!(read("team", "https://example.invalid/x.json", "agentlauncher").is_none());
    }
}
