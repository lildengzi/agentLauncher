//! Launcher-level contract — the launcher's own persisted state, distinct from
//! the per-instance contract in `instance_manager`.
//!
//! Two versioned JSON files live directly under `~/.agentlauncher/` (siblings of
//! `instances/`):
//!   * `config.json`      — UI prefs, launcher-wide agent defaults, session state.
//!   * `instgroups.json`  — a *presentation overlay* for the sidebar: group order,
//!     per-group collapsed state, and manual intra-group instance ordering.
//!
//! Both are backend-owned and mirrored in `src/types.ts`. Secrets never live
//! here — credentials remain each engine's own domain: dsh's in
//! `~/.dsh/.credentials.yaml`, every other engine's in the instance `.env`.
//!
//! Robustness: a missing or malformed file yields built-in defaults rather than
//! an error, so the launcher never bricks on a bad file. `instgroups.json` is an
//! overlay, not the source of truth — membership is still each instance's
//! `group` field; the frontend ignores stale ids and appends unknown ones.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

fn current_format_version() -> u32 {
    1
}

/// `~/.agentlauncher` — the launcher data root. Instances live under `instances/`.
pub fn agentlauncher_root() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("cannot resolve home directory")?;
    Ok(home.join(".agentlauncher"))
}

// ---- config.json ----------------------------------------------------------

/// UI preferences previously scattered across localStorage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiPrefs {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_locale")]
    pub locale: String,
}
fn default_theme() -> String {
    // Matches the frontend theme engine's DEFAULT_THEME (src/lib/themes.ts).
    "prism-dark".to_string()
}
fn default_locale() -> String {
    "zh".to_string()
}
impl Default for UiPrefs {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            locale: default_locale(),
        }
    }
}

/// Launcher-wide agent defaults — prefill the New Instance dialog. Non-secret.
///
/// Both fields default to **empty**, which the engines read as "use your own
/// default" (the same "空值即省略 flag" rule the adapters follow). A concrete
/// vendor default here would be wrong for five of the six engines — and even for
/// dsh, whose provider is `deepseek-official`, not `deepseek`.
///
/// Retired: `base_url` (never reached any engine — base URLs travel through the
/// instance `.env`) and `profile` (a dsh-only knob with no UI, so never anything
/// but its own default). Unknown keys are ignored, so older files still load.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentDefaults {
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub model: String,
}

/// Transient UX state restored across launches.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionState {
    #[serde(default)]
    pub selected_instance: String,
    #[serde(default)]
    pub last_used_group: String,
}

/// The launcher's Node runtime settings — modelled field-for-field on Prism
/// Launcher's `设置 ▸ Java` page, because Node is to this launcher exactly what
/// Java is to Prism: a prerequisite the launcher is willing to provide itself
/// rather than make the user go install.
///
/// Every knob here exists because Prism has its analogue, and each one answers a
/// question that would otherwise be answered by a hardcoded constant:
///
///   * `exe` — the escape hatch. Empty means "use whatever the launcher installed,
///     or the host's". A path here always wins, exactly as `custom_bin` wins for
///     an engine, and it is the only answer for a platform with no official build
///     (there is no `linux-arm64-musl` tarball).
///   * `auto_download` — Prism's ☑ 自动下载 Mojang Java. Named, default-on, and
///     switchable off: automatic is not the same as silent, and a user who wants
///     to keep their own toolchain gets to say so once.
///   * `auto_detect_version` — Prism's ☑ 自动检测 Java 版本, and the reason
///     detection may run `node --version` at all. Engine detection never executes
///     a candidate (see [`crate::engines`]); this checkbox is the user's standing
///     permission for the one exception, and turning it off drops us back to a
///     pure PATH lookup with no version to report.
///   * `skip_version_check` — Prism's ☐ 跳过 Java 兼容性检查. The floor is the
///     highest any engine declares, so it is too strict for most of them: someone
///     running only `codex` (`>=16`) should not be told their Node 20 is unusable.
///   * `max_old_space_mb` — Prism's `-Xmx`. Was a hardcoded 4096 in
///     [`crate::runtimes`]; it is a setting because it is the only lever that
///     exists for an npm resolve that runs out of heap.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSettings {
    #[serde(default)]
    pub exe: String,
    #[serde(default = "yes")]
    pub auto_download: bool,
    #[serde(default = "yes")]
    pub auto_detect_version: bool,
    #[serde(default)]
    pub skip_version_check: bool,
    #[serde(default = "default_heap_mb")]
    pub max_old_space_mb: u32,
}
fn yes() -> bool {
    true
}
/// Measured, not precautionary: resolving `@deepseek-ai/dsh@latest` under node's
/// own default (2144 MB here) dies with `Ineffective mark-compacts near heap
/// limit` before it downloads anything. A ceiling is not a reservation, so raising
/// it costs nothing on the installs that never come near it.
fn default_heap_mb() -> u32 {
    4096
}
impl Default for NodeSettings {
    fn default() -> Self {
        Self {
            exe: String::new(),
            auto_download: true,
            auto_detect_version: true,
            skip_version_check: false,
            max_old_space_mb: default_heap_mb(),
        }
    }
}

/// The launcher config document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LauncherConfig {
    #[serde(default = "current_format_version")]
    pub format_version: u32,
    #[serde(default)]
    pub ui: UiPrefs,
    #[serde(default)]
    pub defaults: AgentDefaults,
    #[serde(default)]
    pub session: SessionState,
    #[serde(default)]
    pub node: NodeSettings,
}
impl Default for LauncherConfig {
    fn default() -> Self {
        Self {
            format_version: current_format_version(),
            ui: UiPrefs::default(),
            defaults: AgentDefaults::default(),
            session: SessionState::default(),
            node: NodeSettings::default(),
        }
    }
}

// ---- instgroups.json ------------------------------------------------------

/// Per-group presentation state. `instances` is a manual ordering overlay.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GroupState {
    #[serde(default)]
    pub collapsed: bool,
    #[serde(default)]
    pub instances: Vec<String>,
}

/// The sidebar grouping overlay. Not the source of truth for membership.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstGroups {
    #[serde(default = "current_format_version")]
    pub format_version: u32,
    /// Group display order (top → bottom); unknown groups append.
    #[serde(default)]
    pub order: Vec<String>,
    #[serde(default)]
    pub groups: std::collections::BTreeMap<String, GroupState>,
}
impl Default for InstGroups {
    fn default() -> Self {
        Self {
            format_version: current_format_version(),
            order: Vec::new(),
            groups: std::collections::BTreeMap::new(),
        }
    }
}

// ---- io -------------------------------------------------------------------

fn config_path() -> Result<PathBuf, String> {
    Ok(agentlauncher_root()?.join("config.json"))
}
fn inst_groups_path() -> Result<PathBuf, String> {
    Ok(agentlauncher_root()?.join("instgroups.json"))
}

/// Parse a JSON doc, falling back to `T::default()` when the file is missing or
/// malformed (never bricks the launcher on a bad file).
pub(crate) fn read_or_default<T: Default + serde::de::DeserializeOwned>(path: &Path) -> T {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
            eprintln!("launcher: ignoring malformed {}: {e}", path.display());
            T::default()
        }),
        Err(_) => T::default(),
    }
}

/// Write `value` as pretty JSON, atomically where the platform allows (temp file
/// + rename), falling back to a direct overwrite.
pub(crate) fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, &text).map_err(|e| e.to_string())?;
    if fs::rename(&tmp, path).is_err() {
        // e.g. Windows won't rename onto an existing file.
        fs::write(path, &text).map_err(|e| e.to_string())?;
        let _ = fs::remove_file(&tmp);
    }
    Ok(())
}

// ---- commands -------------------------------------------------------------

#[tauri::command]
pub fn get_launcher_config() -> Result<LauncherConfig, String> {
    Ok(read_or_default(&config_path()?))
}

#[tauri::command]
pub fn set_launcher_config(config: LauncherConfig) -> Result<(), String> {
    write_json_atomic(&config_path()?, &config)
}

#[tauri::command]
pub fn get_inst_groups() -> Result<InstGroups, String> {
    Ok(read_or_default(&inst_groups_path()?))
}

#[tauri::command]
pub fn set_inst_groups(groups: InstGroups) -> Result<(), String> {
    write_json_atomic(&inst_groups_path()?, &groups)
}

/// Read just the Node section. Its own command (rather than the whole config)
/// because the Node settings page owns its load/save the way `ProvidersSection`
/// does — the settings dialog holds no Node state to get out of sync.
#[tauri::command]
pub fn get_node_settings() -> Result<NodeSettings, String> {
    Ok(node_settings())
}

#[tauri::command]
pub fn set_node_settings(settings: NodeSettings) -> Result<(), String> {
    let path = config_path()?;
    let mut cfg: LauncherConfig = read_or_default(&path);
    cfg.node = settings;
    write_json_atomic(&path, &cfg)
}

/// The Node settings, for backend callers. Falls back to defaults on any problem
/// reaching the file, same as everything else here: a launcher that cannot read
/// its config still has to be able to install Node.
pub(crate) fn node_settings() -> NodeSettings {
    match config_path() {
        Ok(p) => read_or_default::<LauncherConfig>(&p).node,
        Err(_) => NodeSettings::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(tag: &str) -> PathBuf {
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "agentlauncher-cfg-{tag}-{n}-{}.json",
            std::process::id()
        ))
    }

    /// A written config round-trips byte-for-field back through the read path.
    #[test]
    fn launcher_config_round_trips() {
        let p = temp_path("roundtrip");
        let mut cfg = LauncherConfig::default();
        cfg.ui.theme = "dark".into();
        cfg.ui.locale = "en".into();
        cfg.defaults.model = "deepseek-chat".into();
        cfg.session.selected_instance = "web-baseline".into();
        write_json_atomic(&p, &cfg).unwrap();

        let got: LauncherConfig = read_or_default(&p);
        assert_eq!(got.format_version, 1);
        assert_eq!(got.ui.theme, "dark");
        assert_eq!(got.ui.locale, "en");
        assert_eq!(got.defaults.model, "deepseek-chat");
        assert_eq!(got.session.selected_instance, "web-baseline");
        fs::remove_file(&p).ok();
    }

    /// A missing file yields built-in defaults rather than an error.
    #[test]
    fn missing_file_is_default() {
        let cfg: LauncherConfig = read_or_default(&temp_path("absent"));
        assert_eq!(cfg.format_version, 1);
        assert_eq!(cfg.ui.theme, "prism-dark");
        assert_eq!(cfg.ui.locale, "zh");
        // No vendor default: empty ⇒ the chosen engine's own default.
        assert!(cfg.defaults.provider.is_empty());
        assert!(cfg.defaults.model.is_empty());
    }

    /// A partial doc fills every absent field from defaults (forward/backward
    /// compatible: adding a field never breaks an older on-disk file).
    #[test]
    fn partial_doc_fills_defaults() {
        let p = temp_path("partial");
        fs::write(&p, r#"{"ui":{"theme":"light"}}"#).unwrap();
        let cfg: LauncherConfig = read_or_default(&p);
        assert_eq!(cfg.ui.theme, "light");
        assert_eq!(cfg.ui.locale, "zh"); // filled from default
        assert!(cfg.defaults.model.is_empty()); // whole section defaulted
        assert_eq!(cfg.format_version, 1);
        fs::remove_file(&p).ok();
    }

    /// `defaults.base_url` / `defaults.profile` were retired; a config.json still
    /// carrying them must load and keep the fields that remain.
    #[test]
    fn config_with_retired_defaults_still_loads() {
        let p = temp_path("retired");
        fs::write(
            &p,
            r#"{"defaults":{"profile":"headless","provider":"p","base_url":"https://x","model":"m"}}"#,
        )
        .unwrap();
        let cfg: LauncherConfig = read_or_default(&p);
        assert_eq!(cfg.defaults.provider, "p");
        assert_eq!(cfg.defaults.model, "m");
        fs::remove_file(&p).ok();
    }

    /// A malformed file falls back to defaults instead of bricking the launcher.
    #[test]
    fn malformed_file_is_default() {
        let p = temp_path("malformed");
        fs::write(&p, "not json at all {{{").unwrap();
        let cfg: LauncherConfig = read_or_default(&p);
        assert_eq!(cfg.ui.theme, "prism-dark");
        fs::remove_file(&p).ok();
    }

    /// The two Node checkboxes that gate behaviour default *on*, and — the part
    /// that a plain `#[derive(Default)]` would get wrong — a config.json written
    /// before this section existed must come back with them on, not off.
    #[test]
    fn node_settings_default_to_downloading_and_detecting() {
        let d = NodeSettings::default();
        assert!(
            d.auto_download,
            "automatic, named, and switchable — not off"
        );
        assert!(d.auto_detect_version);
        assert!(!d.skip_version_check, "the floor applies until asked");
        assert_eq!(d.max_old_space_mb, 4096);
        assert!(d.exe.is_empty(), "empty ⇒ managed, else host");

        let p = temp_path("node-absent");
        fs::write(&p, r#"{"ui":{"theme":"light"}}"#).unwrap();
        let cfg: LauncherConfig = read_or_default(&p);
        assert!(cfg.node.auto_download);
        assert!(cfg.node.auto_detect_version);
        assert_eq!(cfg.node.max_old_space_mb, 4096);

        // An explicit `false` survives, which is the whole point of a checkbox.
        fs::write(
            &p,
            r#"{"node":{"auto_download":false,"max_old_space_mb":1024}}"#,
        )
        .unwrap();
        let cfg: LauncherConfig = read_or_default(&p);
        assert!(!cfg.node.auto_download);
        assert!(
            cfg.node.auto_detect_version,
            "unmentioned keys stay default"
        );
        assert_eq!(cfg.node.max_old_space_mb, 1024);
        fs::remove_file(&p).ok();
    }

    /// The instgroups overlay round-trips its order and per-group state.
    #[test]
    fn inst_groups_round_trips() {
        let p = temp_path("groups");
        let mut g = InstGroups {
            order: vec!["未分类".into(), "Web".into()],
            ..Default::default()
        };
        g.groups.insert(
            "Web".into(),
            GroupState {
                collapsed: true,
                instances: vec!["web-baseline".into(), "test-agent".into()],
            },
        );
        write_json_atomic(&p, &g).unwrap();

        let got: InstGroups = read_or_default(&p);
        assert_eq!(got.format_version, 1);
        assert_eq!(got.order, vec!["未分类".to_string(), "Web".to_string()]);
        let web = got.groups.get("Web").expect("Web group present");
        assert!(web.collapsed);
        assert_eq!(
            web.instances,
            vec!["web-baseline".to_string(), "test-agent".to_string()]
        );
        fs::remove_file(&p).ok();
    }
}
