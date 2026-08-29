//! Per-instance extension state — what the edit dialog's 扩展插件 / 技能 Skills /
//! MCP 服务器 sections read and write.
//!
//! Where each of the three actually lives is engine reality, not a launcher
//! invention, and the split is deliberate:
//!
//!   plugins — engine-owned and **profile**-scoped, not instance-scoped (for dsh:
//!             the pnpm dependencies of `~/.dsh/profiles/<p>/package.json`). Read
//!             only from here; installing/removing goes through the engine's own
//!             command (`plugin_add` / `plugin_remove`), so two instances sharing a
//!             profile necessarily share its plugins. `plugin_scope` states that
//!             out loud rather than letting the UI imply per-instance ownership.
//!   skills  — one directory per skill under `instances/<id>/skills/`, genuinely
//!             per-instance.
//!   mcp     — `instances/<id>/mcp.json`, in the MCP-standard `mcpServers` shape.
//!
//! On the `mcp.json` key: the standard shape every MCP host reads (`mcpServers`)
//! is what we write. `create_instance` used to scaffold a bare `{"servers":{}}`,
//! which nothing ever read, so that key is accepted on load as a legacy alias and
//! normalised away on the first write.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

use crate::instance_manager;

// ---- contract (mirrored in src/types.ts) ----------------------------------

/// One MCP server, flattened: `name` is the `mcpServers` map key, carried inline
/// so the frontend can edit a list rather than an object.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct McpServerEntry {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Kept in the file but not handed to the engine. Lets a user park a server
    /// without losing its command line.
    #[serde(default)]
    pub disabled: bool,
}

/// One skill directory under `instances/<id>/skills/`.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct SkillEntry {
    pub name: String,
    /// absolute path, so the UI can reveal it without rebuilding the path itself.
    pub path: String,
    /// first prose line of the skill's own `SKILL.md`/`README.md`, or "".
    pub description: String,
}

/// Everything the three edit-dialog sections need, in one round trip.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct InstanceExtensions {
    pub plugins: Vec<String>,
    /// Who owns `plugins`: `"dsh-profile:<name>"`, or `"unsupported"` when the
    /// instance's engine has no plugin concept the launcher can read.
    pub plugin_scope: String,
    pub skills: Vec<SkillEntry>,
    pub mcp: Vec<McpServerEntry>,
}

// ---- mcp.json -------------------------------------------------------------

/// The on-disk value of one `mcpServers` entry (no `name`: that is the map key).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct McpServerValue {
    #[serde(default)]
    command: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    args: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    env: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "is_false")]
    disabled: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// Replace a file whole: write a sibling temp file, then rename over the target.
/// A crash mid-write leaves the previous file intact instead of a truncated one.
/// The temp name is the target plus `.tmp` rather than a replaced extension, so
/// `mcp.json` stays `mcp.json.tmp` and cannot collide with a sibling that differs
/// only by extension. `fs::rename` is not atomic on every filesystem a home
/// directory can live on, hence the plain-write fallback.
fn write_atomic(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    fs::write(&tmp, text).map_err(|e| e.to_string())?;
    if fs::rename(&tmp, path).is_err() {
        fs::write(path, text).map_err(|e| e.to_string())?;
        let _ = fs::remove_file(&tmp);
    }
    Ok(())
}

/// `mcp.json` as a whole. `servers` is the legacy alias the old scaffold wrote;
/// it is read and then dropped, never written back.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct McpDoc {
    #[serde(default, rename = "mcpServers")]
    mcp_servers: BTreeMap<String, McpServerValue>,
    #[serde(default, skip_serializing)]
    servers: BTreeMap<String, McpServerValue>,
}

fn mcp_path(id: &str) -> Result<PathBuf, String> {
    Ok(instance_manager::instance_dir(id)?.join("mcp.json"))
}

/// The instance's MCP servers alone. `pub(crate)` because the market's installer
/// only ever wants this map: going through `read_instance_extensions` would make
/// it shell out to dsh for a plugin list it then throws away.
pub(crate) fn read_mcp(id: &str) -> Result<Vec<McpServerEntry>, String> {
    let path = mcp_path(id)?;
    let doc: McpDoc = match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
            eprintln!("launcher: ignoring malformed {}: {e}", path.display());
            McpDoc::default()
        }),
        Err(_) => McpDoc::default(),
    };
    // Legacy entries lose to same-named modern ones rather than overwriting them.
    let mut merged = doc.servers;
    merged.extend(doc.mcp_servers);
    Ok(merged
        .into_iter()
        .map(|(name, v)| McpServerEntry {
            name,
            command: v.command,
            args: v.args,
            env: v.env,
            disabled: v.disabled,
        })
        .collect())
}

fn write_mcp(id: &str, servers: &[McpServerEntry]) -> Result<(), String> {
    let mut map: BTreeMap<String, McpServerValue> = BTreeMap::new();
    for s in servers {
        let name = s.name.trim();
        if name.is_empty() {
            return Err("MCP server name must not be empty".into());
        }
        if map.contains_key(name) {
            return Err(format!("duplicate MCP server name: {name}"));
        }
        map.insert(
            name.to_string(),
            McpServerValue {
                command: s.command.trim().to_string(),
                args: s.args.clone(),
                env: s.env.clone(),
                disabled: s.disabled,
            },
        );
    }
    let doc = McpDoc {
        mcp_servers: map,
        servers: BTreeMap::new(),
    };
    let path = mcp_path(id)?;
    let text = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())? + "\n";
    write_atomic(&path, &text)
}

// ---- skills ---------------------------------------------------------------

/// First line of prose in a skill's own doc — skipping blank lines, Markdown
/// headings and YAML front matter — so the list can show something more useful
/// than the directory name. Absent doc ⇒ "".
fn skill_description(dir: &Path) -> String {
    for candidate in ["SKILL.md", "README.md", "readme.md"] {
        let Ok(text) = fs::read_to_string(dir.join(candidate)) else {
            continue;
        };
        let mut in_front_matter = false;
        for (i, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line == "---" {
                // Only a leading `---` opens front matter; a later one closes it.
                if i == 0 {
                    in_front_matter = true;
                    continue;
                }
                if in_front_matter {
                    in_front_matter = false;
                    continue;
                }
            }
            if in_front_matter || line.is_empty() || line.starts_with('#') {
                continue;
            }
            return line.chars().take(200).collect();
        }
    }
    String::new()
}

fn read_skills(id: &str) -> Result<Vec<SkillEntry>, String> {
    let root = instance_manager::instance_dir(id)?.join("skills");
    let Ok(entries) = fs::read_dir(&root) else {
        return Ok(vec![]);
    };
    let mut out: Vec<SkillEntry> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| {
            let path = e.path();
            SkillEntry {
                name: e.file_name().to_string_lossy().to_string(),
                description: skill_description(&path),
                path: path.to_string_lossy().to_string(),
            }
        })
        .filter(|s| !s.name.starts_with('.'))
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

// ---- AGENTS.md ------------------------------------------------------------
// The instance's system prompt and behaviour rules (the spec's 备注 → 人设与契约
// row). Read and written on its own rather than folded into
// `read_instance_extensions`, for two reasons: that call shells out to dsh for a
// plugin list this page does not want to wait on, and it is re-issued whenever
// the engine/profile pickers change — which would throw away a half-typed draft
// of a file those pickers have nothing to do with.

/// `AGENTS.md` plus whether the file is actually there.
///
/// The bool is not decoration. An instance scaffolded before `create_instance`
/// seeded the file has no `AGENTS.md` at all, and "no file yet, saving creates
/// one" is a different answer from "the file is empty". Absence is never an
/// error: the editor has to open for every instance.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct AgentsDoc {
    pub text: String,
    pub exists: bool,
}

fn agents_path(id: &str) -> Result<PathBuf, String> {
    Ok(instance_manager::instance_dir(id)?.join("AGENTS.md"))
}

#[tauri::command]
pub fn read_instance_agents(id: String) -> Result<AgentsDoc, String> {
    let path = agents_path(&id)?;
    match fs::read_to_string(&path) {
        Ok(text) => Ok(AgentsDoc { text, exists: true }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(AgentsDoc::default()),
        // Anything else (a permission problem, a directory in its place) is a real
        // failure and must not be shown as an empty prompt the user might save over.
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// Write `AGENTS.md` verbatim — no trimming, no added trailing newline. It is
/// prose the user typed, and normalising it here would make the next read differ
/// from what the editor still has on screen.
#[tauri::command]
pub fn write_instance_agents(id: String, text: String) -> Result<(), String> {
    // Reject an unknown id before touching the disk, as set_instance_mcp does.
    instance_manager::get_instance(&id)?;
    write_atomic(&agents_path(&id)?, &text)
}

// ---- commands -------------------------------------------------------------

/// Everything the edit dialog's three extension sections show, in one call.
/// A missing `skills/` dir or `mcp.json` is an empty list, never an error — the
/// sections must render for an instance scaffolded by an older build too.
///
/// `engine` and `profile` override what the instance has on disk, because the
/// dialog asks this question about the form the user is *looking at*: switching
/// the profile picker changes which plugin set is in scope before anything is
/// saved, and answering from the saved value would make that switch a no-op that
/// still looks like an answer. Pass `None` for both to read the instance as saved.
#[tauri::command]
pub fn read_instance_extensions(
    id: String,
    engine: Option<String>,
    profile: Option<String>,
) -> Result<InstanceExtensions, String> {
    let inst = instance_manager::get_instance(&id)?;
    let engine = engine.unwrap_or_else(|| inst.runtime.engine.clone());
    // Plugins are the engine's, and only dsh exposes a readable set today.
    let (plugins, plugin_scope) = if engine.is_empty() || engine == "dsh" {
        let profile = profile
            .filter(|p| !p.is_empty())
            .or_else(|| Some(inst.profile.clone()).filter(|p| !p.is_empty()))
            .unwrap_or_else(|| "headless".to_string());
        let found = crate::runtime::dsh_home::list_installed_plugins(profile.clone())
            .unwrap_or_default();
        (found, format!("dsh-profile:{profile}"))
    } else {
        (vec![], "unsupported".to_string())
    };
    Ok(InstanceExtensions {
        plugins,
        plugin_scope,
        skills: read_skills(&id)?,
        mcp: read_mcp(&id)?,
    })
}

/// Replace the whole `mcpServers` map. Whole-document writes keep the file and the
/// dialog from drifting apart; per-entry patching would need a merge policy that
/// the UI has no way to express.
#[tauri::command]
pub fn set_instance_mcp(id: String, servers: Vec<McpServerEntry>) -> Result<(), String> {
    // Reject an unknown id before touching the disk.
    instance_manager::get_instance(&id)?;
    write_mcp(&id, &servers)
}

/// Delete one skill directory. Guarded against a `name` that would escape
/// `skills/` — the frontend passes back a name it was given, but a command is
/// callable with anything.
#[tauri::command]
pub fn remove_instance_skill(id: String, name: String) -> Result<(), String> {
    if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err(format!("invalid skill name: {name}"));
    }
    let dir = instance_manager::instance_dir(&id)?.join("skills").join(&name);
    if !dir.is_dir() {
        return Err(format!("no such skill: {name}"));
    }
    fs::remove_dir_all(&dir).map_err(|e| e.to_string())
}

/// Reveal one of an instance's own subdirectories in the file manager, creating it
/// first so "open the skills folder" works on an instance that has none yet. The
/// allowed set is closed: this is not a general "open any path" command.
#[tauri::command]
pub fn open_instance_subdir(app: AppHandle, id: String, sub: String) -> Result<(), String> {
    if !matches!(sub.as_str(), "skills" | "workspace" | "logs") {
        return Err(format!("not an instance subdirectory: {sub}"));
    }
    let dir = instance_manager::instance_dir(&id)?.join(&sub);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    app.opener()
        .open_path(dir.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{temp_tree, EnvGuard, HOME_LOCK};

    fn scaffold(home: &Path) -> String {
        let inst = instance_manager::create_instance(instance_manager::NewInstance {
            name: "Ext Fixture".into(),
            icon: "bot".into(),
            group: String::new(),
            description: String::new(),
            profile: "headless".into(),
            provider: String::new(),
            model: String::new(),
            api_key_ref: String::new(),
            default_task: String::new(),
            runtime: Default::default(),
        })
        .expect("create_instance should succeed");
        assert!(instance_manager::instance_dir(&inst.id)
            .unwrap()
            .starts_with(home.join(".agentlauncher")));
        inst.id
    }

    #[test]
    fn mcp_roundtrips_and_normalises_the_legacy_servers_key() {
        let _g = HOME_LOCK.lock().unwrap();
        let tree = temp_tree("ext-mcp");
        let _home = EnvGuard::set("HOME", tree.path());
        let id = scaffold(tree.path());

        // Freshly scaffolded instances carry the legacy `{"servers":{}}` shape.
        assert_eq!(read_mcp(&id).unwrap(), vec![]);

        let entry = McpServerEntry {
            name: "fs".into(),
            command: "npx".into(),
            args: vec!["-y".into(), "@modelcontextprotocol/server-filesystem".into()],
            env: BTreeMap::from([("ROOT".to_string(), "/tmp".to_string())]),
            disabled: true,
        };
        write_mcp(&id, std::slice::from_ref(&entry)).unwrap();
        assert_eq!(read_mcp(&id).unwrap(), vec![entry]);

        // The write normalises the key: standard in, legacy alias gone.
        let text = fs::read_to_string(mcp_path(&id).unwrap()).unwrap();
        assert!(text.contains("mcpServers"), "should write the standard key");
        assert!(!text.contains("\"servers\""), "legacy key should be dropped");
    }

    #[test]
    fn write_mcp_rejects_blank_and_duplicate_names() {
        let _g = HOME_LOCK.lock().unwrap();
        let tree = temp_tree("ext-mcp-bad");
        let _home = EnvGuard::set("HOME", tree.path());
        let id = scaffold(tree.path());

        let blank = McpServerEntry {
            name: "  ".into(),
            ..Default::default()
        };
        assert!(write_mcp(&id, &[blank]).is_err());

        let dup = McpServerEntry {
            name: "fs".into(),
            ..Default::default()
        };
        assert!(write_mcp(&id, &[dup.clone(), dup]).is_err());
    }

    #[test]
    fn skills_are_listed_with_their_doc_summary_and_removed_safely() {
        let _g = HOME_LOCK.lock().unwrap();
        let tree = temp_tree("ext-skills");
        let _home = EnvGuard::set("HOME", tree.path());
        let id = scaffold(tree.path());
        let skills = instance_manager::instance_dir(&id).unwrap().join("skills");

        fs::create_dir_all(skills.join("pdf-forms")).unwrap();
        fs::write(
            skills.join("pdf-forms/SKILL.md"),
            "---\nname: pdf-forms\n---\n\n# PDF Forms\n\nFill in PDF form fields.\n",
        )
        .unwrap();
        fs::create_dir_all(skills.join("bare")).unwrap();
        fs::create_dir_all(skills.join(".hidden")).unwrap();
        fs::write(skills.join("loose.txt"), "not a skill").unwrap();

        let found = read_skills(&id).unwrap();
        let names: Vec<&str> = found.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["bare", "pdf-forms"]);
        assert_eq!(found[1].description, "Fill in PDF form fields.");
        assert_eq!(found[0].description, "");

        // A name that would escape `skills/` is refused before any removal.
        assert!(remove_instance_skill(id.clone(), "../../etc".into()).is_err());
        assert!(remove_instance_skill(id.clone(), "nope".into()).is_err());
        remove_instance_skill(id.clone(), "bare".into()).unwrap();
        assert!(!skills.join("bare").exists());
        assert!(skills.join("pdf-forms").is_dir());
    }

    #[test]
    fn agents_md_roundtrips_and_tells_absence_from_emptiness() {
        let _g = HOME_LOCK.lock().unwrap();
        let tree = temp_tree("ext-agents");
        let _home = EnvGuard::set("HOME", tree.path());
        let id = scaffold(tree.path());
        let path = agents_path(&id).unwrap();

        // create_instance seeds the file, so a fresh instance already has one.
        let seeded = read_instance_agents(id.clone()).unwrap();
        assert!(seeded.exists);
        assert!(!seeded.text.is_empty());

        // An instance from an older build has none: absent, not an error.
        fs::remove_file(&path).unwrap();
        let missing = read_instance_agents(id.clone()).unwrap();
        assert!(!missing.exists, "a missing AGENTS.md must report exists=false");
        assert_eq!(missing.text, "");

        // Written verbatim — trailing whitespace and all, so a reload matches what
        // the editor still shows.
        let body = "# Persona\n\n  你是一个自包含的代码分析 Agent。  \n\n";
        write_instance_agents(id.clone(), body.into()).unwrap();
        let back = read_instance_agents(id.clone()).unwrap();
        assert!(back.exists);
        assert_eq!(back.text, body);

        // An emptied file still exists, which is why `exists` is not `!text.is_empty()`.
        write_instance_agents(id.clone(), String::new()).unwrap();
        let emptied = read_instance_agents(id.clone()).unwrap();
        assert!(emptied.exists);
        assert_eq!(emptied.text, "");

        // The atomic write leaves no temp file next to the target.
        assert!(!path.with_file_name("AGENTS.md.tmp").exists());
    }
}
