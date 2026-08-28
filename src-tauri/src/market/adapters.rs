//! Adapters — the part that actually makes the market decentralized.
//!
//! A source row names its own `adapter`, and an adapter's only job is to turn that
//! source's native payload into the one normalised `MarketItem` vocabulary. Nothing
//! downstream — not the query layer, not the dialog — knows a feed's real shape, so
//! joining a new index costs an adapter here and nothing anywhere else.
//!
//! Everything in this file reads a payload written by somebody else, which is the
//! reason none of it derives `Deserialize` on an input struct. The feed we ship
//! sends `"license": null` on 480 of its 4753 entries, and a derived `String` field
//! rejects `null` — one entry's missing licence would have cost the whole source.
//! The readers below map absent, null and wrong-typed alike onto the empty value, so
//! a sloppy field costs that field and nothing more. Nothing here executes anything
//! a feed says; every string is data on its way to being rendered as text.

use serde_json::Value;

use super::{InstallSpec, MarketItem, MarketVersion};
use crate::instance_ext::McpServerEntry;

// ---- lenient readers ------------------------------------------------------

fn text(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// First non-empty of several keys. Feeds disagree about names for the same thing
/// (`description` / `descriptionZh`, `title` / `name`, `updated_at` / `updatedAt`).
fn text_any(v: &Value, keys: &[&str]) -> String {
    keys.iter()
        .map(|k| text(v, k))
        .find(|s| !s.is_empty())
        .unwrap_or_default()
}

fn number(v: &Value, key: &str) -> u64 {
    v.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn flag(v: &Value, key: &str) -> bool {
    v.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn strings(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn array<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v.get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
}

/// `"<source>:<native id>"`. The prefix is what keeps ids unique across sources, and
/// it is applied here rather than trusted from the payload so no feed can hand back
/// an id that claims to belong to another source.
fn qualify(source_id: &str, native: &str) -> String {
    format!("{source_id}:{native}")
}

/// Normalise one payload from `source_id`, whose row declared `adapter`.
pub fn normalise(
    adapter: &str,
    source_id: &str,
    payload: &Value,
) -> Result<Vec<MarketItem>, String> {
    match adapter {
        "agentlauncher" => Ok(canonical(source_id, payload)),
        "dsh-market" => dsh_market(source_id, payload),
        "mcp-registry" => mcp_registry(source_id, payload),
        // Reported as this one source's error. An unknown adapter is a typo or a
        // `sources.json` from a newer build, and neither is a reason for the other
        // sources' items to vanish from the dialog.
        other => Err(format!("unknown payload adapter: {other}")),
    }
}

// ---- `agentlauncher` — our own canonical shape ----------------------------

/// `{ "items": [ … ] }`, a bare array, or a single item object.
///
/// All three are accepted because this is the adapter the drop-in directory uses:
/// those files are hand-written, and refusing a one-item file for missing its
/// `items` wrapper would be pedantry with no upside.
fn canonical(source_id: &str, payload: &Value) -> Vec<MarketItem> {
    let rows: &[Value] = if let Some(items) = payload.get("items").and_then(Value::as_array) {
        items
    } else if let Some(arr) = payload.as_array() {
        arr
    } else if payload.is_object() {
        std::slice::from_ref(payload)
    } else {
        &[]
    };
    rows.iter()
        .filter_map(|r| canonical_item(source_id, r))
        .collect()
}

fn canonical_item(source_id: &str, row: &Value) -> Option<MarketItem> {
    let native = text_any(row, &["id", "name"]);
    let name = text_any(row, &["name", "id"]);
    // Without something to key a row by and something to label it with there is no
    // row to draw, so the entry is dropped rather than rendered blank.
    if native.is_empty() || name.is_empty() {
        return None;
    }
    let versions = version_list(row);
    Some(MarketItem {
        id: qualify(source_id, &native),
        source: source_id.to_string(),
        kind: canonical_kind(row, &versions),
        name,
        author: text_any(row, &["author", "owner"]),
        description: text_any(row, &["description", "descriptionZh"]),
        readme: text(row, "readme"),
        icon: text(row, "icon"),
        homepage: text(row, "homepage"),
        repo: text(row, "repo"),
        tags: strings(row, "tags"),
        license: text(row, "license"),
        downloads: number(row, "downloads"),
        updated_at: text_any(row, &["updated_at", "updatedAt"]),
        versions,
    })
}

/// A file that forgot `kind` would otherwise be invisible in all three dialogs, so
/// infer it from how the item installs instead of dropping it: only the MCP dialog
/// merges an `mcpServers` entry, and only skills are cloned into `skills/`.
fn canonical_kind(row: &Value, versions: &[MarketVersion]) -> String {
    let declared = text(row, "kind");
    if !declared.is_empty() {
        return declared;
    }
    match versions.first().map(|v| v.install.method.as_str()) {
        Some("mcp-config") => "mcp".into(),
        Some("git-clone") => "skill".into(),
        _ => "plugin".into(),
    }
}

/// `versions: [ … ]`, or the single `install` block a thin file is likelier to write.
fn version_list(row: &Value) -> Vec<MarketVersion> {
    let listed: Vec<MarketVersion> = array(row, "versions")
        .iter()
        .map(|v| MarketVersion {
            version: non_empty(text_any(v, &["version", "tag"]), "latest"),
            published_at: text_any(v, &["published_at", "publishedAt"]),
            // A version entry may nest its own `install` or simply be one.
            install: install_spec(v.get("install").unwrap_or(v)),
        })
        .collect();
    if !listed.is_empty() {
        return listed;
    }
    match row.get("install") {
        Some(i) if i.is_object() => vec![MarketVersion {
            version: non_empty(text(row, "version"), "latest"),
            published_at: text_any(row, &["published_at", "publishedAt"]),
            install: install_spec(i),
        }],
        // Empty `versions` is meaningful: the dialog shows the row read-only rather
        // than offering an install it has no instructions for.
        _ => vec![],
    }
}

fn non_empty(s: String, fallback: &str) -> String {
    if s.is_empty() {
        fallback.to_string()
    } else {
        s
    }
}

/// `method` is carried through verbatim.
///
/// Folding an unrecognised method onto `"manual"` here would throw away the one
/// string that says what the item wanted. `install.rs` already refuses to run a
/// method it does not know and the dialog falls back to showing `command`, which is
/// the degradation the `String` type exists for.
fn install_spec(v: &Value) -> InstallSpec {
    InstallSpec {
        method: text(v, "method"),
        package: text_any(v, &["package", "pkg"]),
        repo: text(v, "repo"),
        command: text(v, "command"),
        // Names only. A feed that ships a value in here is shipping somebody's
        // credential, and it is not going to reach a config file through us.
        env: strings(v, "env"),
        mcp: mcp_entry(v.get("mcp")),
    }
}

fn mcp_entry(v: Option<&Value>) -> Option<McpServerEntry> {
    let v = v?;
    let name = text(v, "name");
    let command = text(v, "command");
    if name.is_empty() || command.is_empty() {
        return None;
    }
    Some(McpServerEntry {
        name,
        command,
        args: strings(v, "args"),
        env: v
            .get("env")
            .and_then(Value::as_object)
            .map(|m| {
                m.iter()
                    .filter_map(|(k, val)| val.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default(),
        disabled: flag(v, "disabled"),
    })
}

// ---- `dsh-market` — the one HTTP feed we ship ------------------------------

/// `https://dsh.market/plugins.json` — a GitHub-derived index of dsh plugins and
/// skills (`schemaVersion` 2, ~4.7k entries, 11.6 MB).
///
/// `schemaVersion` is deliberately not checked. The field has already moved 1 → 2
/// without the fields below changing, so gating on it would have broken the source
/// for a bump that cost us nothing.
fn dsh_market(source_id: &str, payload: &Value) -> Result<Vec<MarketItem>, String> {
    let plugins = payload
        .get("plugins")
        .and_then(Value::as_array)
        .ok_or_else(|| "payload has no `plugins` array — is this a dsh.market feed?".to_string())?;
    Ok(plugins
        .iter()
        .filter_map(|p| dsh_item(source_id, p))
        .collect())
}

fn dsh_item(source_id: &str, p: &Value) -> Option<MarketItem> {
    let native = text(p, "id");
    let name = text(p, "name");
    if native.is_empty() || name.is_empty() {
        return None;
    }
    let kind = match text(p, "type").as_str() {
        "skill" => "skill",
        "cordis-plugin" => "plugin",
        // A type none of the three dialogs serves is dropped rather than filed under
        // a dialog it does not belong in.
        _ => return None,
    };
    let full = text_any(p, &["fullName", "id"]);
    let repo = if full.is_empty() {
        String::new()
    } else {
        format!("https://github.com/{full}")
    };
    Some(MarketItem {
        id: qualify(source_id, &native),
        source: source_id.to_string(),
        kind: kind.to_string(),
        name,
        author: text(p, "owner"),
        // The feed carries both languages and the launcher's default locale is zh.
        description: text_any(p, &["descriptionZh", "description"]),
        // A summary, not the README: `market_readme` fetches the full document
        // lazily when the detail pane asks, so the list payload stays list-sized.
        readme: text(p, "readmeSummary"),
        icon: if kind == "skill" {
            "graduation-cap".into()
        } else {
            "package".into()
        },
        homepage: text(p, "homepage"),
        repo: repo.clone(),
        tags: strings(p, "tags"),
        license: text(p, "license"),
        // The feed has no download counter; stars are its popularity signal, and the
        // "most downloaded" sort is the only place this field is read.
        downloads: number(p, "stars"),
        updated_at: text_any(p, &["pushedAt", "updatedAt"]),
        versions: vec![dsh_version(p, kind, &full, &repo)],
    })
}

/// One version per entry, named `latest`.
///
/// Not a shortcut: every install this feed describes resolves a git ref
/// (`github:owner/repo`), so there is exactly one thing to install, and populating a
/// version dropdown with release numbers the feed does not carry would be a lie.
///
/// `install.needsConfig` is dropped: it is a boolean with no variable names attached,
/// and `InstallSpec::env` is a list of names. Guessing which vars an item wants would
/// put the wrong ones in front of the user.
fn dsh_version(p: &Value, kind: &str, full: &str, repo: &str) -> MarketVersion {
    let install = p.get("install").cloned().unwrap_or(Value::Null);
    let commands = strings(&install, "commands");
    let spec = if kind == "skill" {
        InstallSpec {
            method: "git-clone".into(),
            repo: if repo.is_empty() {
                String::new()
            } else {
                format!("{repo}.git")
            },
            command: commands.first().cloned().unwrap_or_default(),
            ..Default::default()
        }
    } else {
        InstallSpec {
            method: "pnpm-profile".into(),
            package: dsh_package(&commands, full),
            command: commands.first().cloned().unwrap_or_default(),
            ..Default::default()
        }
    };
    MarketVersion {
        version: "latest".into(),
        published_at: text_any(p, &["pushedAt", "updatedAt"]),
        install: spec,
    }
}

/// The pnpm spec to hand `dsh plugin add`.
///
/// The feed scrapes install commands out of READMEs, so `commands` is a hint rather
/// than a fact: some entries carry a monorepo's entire install section (naming
/// sibling packages, not this one) and some carry a literal placeholder such as
/// `<tarball path>`. A scraped spec is therefore trusted only when it plainly names
/// *this* repository; otherwise `github:<fullName>`, which pnpm resolves the same
/// way, is the honest answer.
fn dsh_package(commands: &[String], full: &str) -> String {
    let repo_name = full.rsplit('/').next().unwrap_or_default().to_lowercase();
    for cmd in commands {
        let Some(rest) = cmd.split(" add ").nth(1) else {
            continue;
        };
        let Some(token) = rest.split_whitespace().next() else {
            continue;
        };
        if token.contains('<') || token.contains('>') {
            continue;
        }
        if !repo_name.is_empty() && token.to_lowercase().contains(&repo_name) {
            return token.to_string();
        }
    }
    if full.is_empty() {
        String::new()
    } else {
        format!("github:{full}")
    }
}

// ---- `mcp-registry` — the official MCP registry -----------------------------

/// `https://registry.modelcontextprotocol.io/v0/servers`.
///
/// This adapter is written against the live endpoint (schema `2025-12-11`), not
/// against docs — the row shipped disabled precisely so that nobody was pointed at a
/// payload we had only guessed at. The listing wraps each entry as
/// `{ server: { … }, _meta: { … } }` and pages by `metadata.nextCursor`, which
/// `mod.rs` walks.
///
/// Two shapes of entry, and only one of them is installable here:
///
/// * `packages[]` — a published package with a stdio transport. That is precisely an
///   `mcpServers` command line, so it becomes an `mcp-config` install.
/// * `remotes[]` — an HTTP endpoint. `McpServerEntry` is a command plus argv and
///   cannot express a URL; wrapping one in a bridge process would be the launcher
///   inventing an install mechanism, so the endpoint is offered as a `manual` string
///   for the user to paste into whatever speaks to it.
///
/// The registry lists each version of a server as its own entry, which is what
/// `MarketItem::versions` is for: entries are grouped by name, newest first.
fn mcp_registry(source_id: &str, payload: &Value) -> Result<Vec<MarketItem>, String> {
    let servers = payload
        .get("servers")
        .and_then(Value::as_array)
        .ok_or_else(|| "payload has no `servers` array — is this an MCP registry?".to_string())?;

    let mut out: Vec<MarketItem> = Vec::new();
    for entry in servers {
        // Tolerate both the wrapped listing and a bare server object.
        let s = entry.get("server").unwrap_or(entry);
        let native = text(s, "name");
        if native.is_empty() {
            continue;
        }
        let published = registry_published_at(entry);
        let version = MarketVersion {
            version: non_empty(text(s, "version"), "latest"),
            published_at: published.clone(),
            install: mcp_install(s, &native),
        };
        let id = qualify(source_id, &native);
        if let Some(existing) = out.iter_mut().find(|i| i.id == id) {
            existing.versions.push(version);
            if published > existing.updated_at {
                existing.updated_at = published;
            }
            continue;
        }
        out.push(MarketItem {
            id,
            source: source_id.to_string(),
            kind: "mcp".to_string(),
            // `title` is the human name; `name` is the reverse-DNS identifier.
            name: non_empty(text(s, "title"), &native),
            // The namespace half of `com.example/thing` is who published it.
            author: native.split('/').next().unwrap_or_default().to_string(),
            description: text(s, "description"),
            readme: String::new(),
            icon: "plug".to_string(),
            homepage: text_any(s, &["websiteUrl", "homepage"]),
            repo: s
                .get("repository")
                .map(|r| text(r, "url"))
                .unwrap_or_default(),
            // The registry has no tag vocabulary; an invented one would filter
            // items out of a dialog for reasons the user could not see.
            tags: Vec::new(),
            license: String::new(),
            downloads: 0,
            updated_at: published,
            versions: vec![version],
        });
    }
    for item in &mut out {
        item.versions
            .sort_by(|a, b| b.published_at.cmp(&a.published_at));
    }
    Ok(out)
}

/// The registry stamps its own timestamps into a namespaced `_meta` block rather than
/// onto the server object, so they are read from there and fall back to the entry.
fn registry_published_at(entry: &Value) -> String {
    entry
        .get("_meta")
        .and_then(|m| m.get("io.modelcontextprotocol.registry/official"))
        .map(|o| text_any(o, &["updatedAt", "publishedAt"]))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| text_any(entry, &["updatedAt", "publishedAt"]))
}

/// How one registry server installs.
fn mcp_install(s: &Value, native: &str) -> InstallSpec {
    for pkg in array(s, "packages") {
        let identifier = text_any(pkg, &["identifier", "name"]);
        if identifier.is_empty() {
            continue;
        }
        // An `mcpServers` entry is a child process the engine talks to over pipes, so
        // a package announcing any other transport is not one of these.
        let transport = pkg.get("transport").map(|t| text(t, "type")).unwrap_or_default();
        if !transport.is_empty() && transport != "stdio" {
            continue;
        }
        let version = text(pkg, "version");
        let spec = if version.is_empty() {
            identifier.clone()
        } else {
            format!("{identifier}@{version}")
        };
        // Only runners we can name with confidence. `oci`, `nuget` and `mcpb` each
        // want their own host tooling, and a guessed command line would produce an
        // entry that installs cleanly and then fails at launch.
        let (command, args) = match text_any(pkg, &["registryType", "registry_name"]).as_str() {
            "npm" => ("npx", vec!["-y".to_string(), spec.clone()]),
            "pypi" => ("uvx", vec![spec.clone()]),
            _ => continue,
        };
        // Names only — the registry marks some of these `isSecret`, and a secret's
        // value has no business in an instance's mcp.json.
        let env: Vec<String> = array(pkg, "environmentVariables")
            .iter()
            .map(|e| text(e, "name"))
            .filter(|n| !n.is_empty())
            .collect();
        return InstallSpec {
            method: "mcp-config".into(),
            package: spec,
            command: format!("{command} {}", args.join(" ")),
            env,
            mcp: Some(McpServerEntry {
                // The `mcpServers` key: the trailing segment reads as a server name,
                // where the full reverse-DNS identifier reads as a package.
                name: native.rsplit('/').next().unwrap_or(native).to_string(),
                command: command.to_string(),
                args,
                env: Default::default(),
                disabled: false,
            }),
            ..Default::default()
        };
    }
    for remote in array(s, "remotes") {
        let url = text(remote, "url");
        if !url.is_empty() {
            return InstallSpec {
                method: "manual".into(),
                command: url,
                ..Default::default()
            };
        }
    }
    InstallSpec {
        method: "manual".into(),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The shape of the feed we ship, including the two things that actually bite:
    /// `license: null` and a scraped install command naming a sibling package.
    #[test]
    fn dsh_market_normalises_plugins_and_skills() {
        let payload = json!({
            "schemaVersion": 2,
            "plugins": [
                {
                    "id": "owner/dsh-thing", "type": "cordis-plugin",
                    "name": "dsh-thing", "owner": "owner", "fullName": "owner/dsh-thing",
                    "stars": 11, "license": null,
                    "description": "A thing.", "descriptionZh": "一个东西。",
                    "tags": ["dsh-plugin", ""], "pushedAt": "2026-08-26T14:35:21Z",
                    "readmeSummary": "# thing",
                    "install": {
                        "method": "pnpm-profile", "needsConfig": false,
                        "commands": ["dsh plugin --profile web add github:owner/dsh-thing"]
                    }
                },
                {
                    "id": "who/url-manager", "type": "skill", "name": "url-manager",
                    "owner": "who", "fullName": "who/url-manager",
                    "install": { "method": "skills-add", "target": "~/.agents/skills" }
                },
                { "id": "x/y", "type": "something-else", "name": "y" },
                { "type": "cordis-plugin", "name": "no id" }
            ]
        });
        let items = normalise("dsh-market", "dsh-market", &payload).unwrap();
        assert_eq!(items.len(), 2, "unknown types and id-less rows are dropped");

        let plugin = &items[0];
        assert_eq!(plugin.id, "dsh-market:owner/dsh-thing");
        assert_eq!(plugin.kind, "plugin");
        assert_eq!(plugin.description, "一个东西。", "zh wins for the default locale");
        assert_eq!(plugin.license, "", "a null licence costs the field, not the feed");
        assert_eq!(plugin.tags, vec!["dsh-plugin".to_string()]);
        assert_eq!(plugin.downloads, 11);
        assert_eq!(plugin.repo, "https://github.com/owner/dsh-thing");
        let v = &plugin.versions[0];
        assert_eq!(v.version, "latest");
        assert_eq!(v.install.method, "pnpm-profile");
        assert_eq!(v.install.package, "github:owner/dsh-thing");

        let skill = &items[1];
        assert_eq!(skill.kind, "skill");
        assert_eq!(skill.versions[0].install.method, "git-clone");
        assert_eq!(
            skill.versions[0].install.repo,
            "https://github.com/who/url-manager.git"
        );
    }

    /// A scraped command that names some other package must not become this item's
    /// install spec; nor must a README placeholder.
    #[test]
    fn a_scraped_command_is_only_trusted_when_it_names_this_repo() {
        let cmds = vec![
            "dsh plugin --profile web add github:other/dsh-tool-csv".to_string(),
            "dsh plugin --profile web add <npm pack tarball>".to_string(),
        ];
        assert_eq!(dsh_package(&cmds, "owner/mine"), "github:owner/mine");
        assert_eq!(
            dsh_package(
                &["dsh plugin --profile p add git+https://github.com/o/mine.git".to_string()],
                "o/mine"
            ),
            "git+https://github.com/o/mine.git"
        );
        assert_eq!(dsh_package(&[], ""), "");
    }

    /// A truncated or plainly wrong payload is this source's error, with a message
    /// the row can show — never a panic and never an empty success.
    #[test]
    fn a_malformed_payload_is_an_error_not_a_panic() {
        // Right adapter, wrong document.
        let err = normalise("dsh-market", "dsh-market", &json!({"plugins": "nope"}))
            .expect_err("a string `plugins` is not a feed");
        assert!(err.contains("plugins"), "{err}");
        assert!(normalise("dsh-market", "s", &json!(null)).is_err());
        assert!(normalise("mcp-registry", "s", &json!({"servers": {}})).is_err());
        // An adapter nobody taught us stops at its own source.
        let err = normalise("npm", "s", &json!({})).expect_err("unknown adapter");
        assert!(err.contains("unknown payload adapter"), "{err}");
        // Junk in the canonical shape yields no items rather than an error: a
        // drop-in directory with one bad file should still serve its good ones.
        assert!(normalise("agentlauncher", "local", &json!(42)).unwrap().is_empty());
        assert!(normalise("agentlauncher", "local", &json!({"items": [{}, 7]}))
            .unwrap()
            .is_empty());
    }

    /// The hand-written shapes a drop-in directory actually contains.
    #[test]
    fn the_canonical_adapter_accepts_thin_files() {
        // A bare single object with no `kind`, installing an MCP server.
        let one = json!({
            "id": "pg", "name": "Postgres Inspector",
            "install": {
                "method": "mcp-config",
                "mcp": { "name": "postgres", "command": "npx", "args": ["-y", "pg-mcp"] }
            }
        });
        let items = normalise("agentlauncher", "local", &one).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "local:pg");
        assert_eq!(items[0].kind, "mcp", "kind inferred from the install method");
        assert_eq!(items[0].versions[0].install.mcp.as_ref().unwrap().name, "postgres");

        // A bare array, and an item with nothing installable at all.
        let many = json!([
            { "id": "a", "name": "A", "kind": "skill", "install": { "method": "git-clone", "repo": "https://x.invalid/a.git" } },
            { "id": "b", "name": "B", "kind": "plugin" }
        ]);
        let items = normalise("agentlauncher", "local", &many).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].kind, "skill");
        assert!(
            items[1].versions.is_empty(),
            "no install block ⇒ read-only row, not a fabricated version"
        );
    }

    /// The live registry listing (schema 2025-12-11): a stdio npm package becomes an
    /// `mcpServers` entry, a remote-only server degrades to a manual endpoint, and two
    /// entries for one name collapse into one item with two versions.
    #[test]
    fn mcp_registry_groups_versions_and_only_installs_stdio_packages() {
        let payload = json!({
            "servers": [
                {
                    "server": {
                        "name": "agency.kesey/pretrip", "title": "Pre-Trip scanner",
                        "description": "Screen copy.", "version": "1.0.1",
                        "websiteUrl": "https://scan.kesey.agency/",
                        "packages": [{
                            "registryType": "npm", "identifier": "pretrip-mcp",
                            "version": "1.0.1", "transport": { "type": "stdio" },
                            "environmentVariables": [
                                { "name": "PRETRIP_API_KEY", "isSecret": true, "description": "key" }
                            ]
                        }]
                    },
                    "_meta": { "io.modelcontextprotocol.registry/official": {
                        "publishedAt": "2026-05-01T00:00:00Z", "updatedAt": "2026-05-01T00:00:00Z"
                    }}
                },
                {
                    "server": { "name": "agency.kesey/pretrip", "version": "1.0.0" },
                    "_meta": { "io.modelcontextprotocol.registry/official": {
                        "updatedAt": "2026-04-01T00:00:00Z"
                    }}
                },
                {
                    "server": {
                        "name": "ac.inference.sh/mcp", "title": "inference.sh",
                        "version": "1.0.0",
                        "remotes": [{ "type": "streamable-http", "url": "https://api.inference.sh/mcp" }]
                    }
                }
            ],
            "metadata": { "nextCursor": "ai.adramp/google-ads:1.0.3", "count": 3 }
        });
        let items = normalise("mcp-registry", "mcp-registry", &payload).unwrap();
        assert_eq!(items.len(), 2, "two entries for one name are one item");

        let pretrip = &items[0];
        assert_eq!(pretrip.id, "mcp-registry:agency.kesey/pretrip");
        assert_eq!(pretrip.kind, "mcp");
        assert_eq!(pretrip.name, "Pre-Trip scanner");
        assert_eq!(pretrip.author, "agency.kesey");
        assert_eq!(pretrip.updated_at, "2026-05-01T00:00:00Z");
        assert_eq!(
            pretrip.versions.iter().map(|v| v.version.as_str()).collect::<Vec<_>>(),
            vec!["1.0.1", "1.0.0"],
            "newest first"
        );
        let install = &pretrip.versions[0].install;
        assert_eq!(install.method, "mcp-config");
        let entry = install.mcp.as_ref().unwrap();
        assert_eq!(entry.name, "pretrip");
        assert_eq!(entry.command, "npx");
        assert_eq!(entry.args, vec!["-y".to_string(), "pretrip-mcp@1.0.1".to_string()]);
        assert!(entry.env.is_empty(), "no value for a secret ever enters mcp.json");
        assert_eq!(install.env, vec!["PRETRIP_API_KEY".to_string()], "names only");

        // Remote-only: an endpoint to copy, not an invented bridge command.
        let remote = &items[1];
        assert_eq!(remote.versions[0].install.method, "manual");
        assert_eq!(remote.versions[0].install.command, "https://api.inference.sh/mcp");
        assert!(remote.versions[0].install.mcp.is_none());
    }
}
