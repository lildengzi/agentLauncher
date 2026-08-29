use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Per-instance runtime/environment override — mirrors `RuntimeConfig` in
/// src/types.ts. Governs how the host resolves the agent binary and the child
/// process PATH. All fields default, so an older `instance.json` without a
/// `runtime` block reads as `autodetect` with no custom binary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// Which agent CLI ("framework") to launch: "dsh" | "pi" | "omp" | "claude"
    /// | "codex" | "opencode". Selects the `AgentRuntime` impl (see runtime::
    /// for_instance). Missing/empty/unknown ⇒ "dsh" (backward compatible).
    #[serde(default = "default_engine")]
    pub engine: String,
    /// "autodetect" — enrich the child PATH from the host login shell; or
    /// "isolated" — a minimal deterministic PATH that does not leak the host
    /// toolchain. Kept as a String (like `profile`) and validated at use.
    #[serde(default = "default_env_policy")]
    pub env_policy: String,
    /// Absolute path to this instance's agent CLI; when non-empty it overrides
    /// the PATH lookup for the binary and its directory is added to PATH.
    #[serde(default)]
    pub custom_bin: String,
}
fn default_engine() -> String {
    "dsh".to_string()
}
fn default_env_policy() -> String {
    "autodetect".to_string()
}
impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            engine: default_engine(),
            env_policy: default_env_policy(),
            custom_bin: String::new(),
        }
    }
}

/// One Agent instance — mirrors `Instance` in src/types.ts.
///
/// Unknown keys are ignored, so a field retired from the contract keeps reading
/// from older files without a `schema_version` bump: `temperature` /
/// `thinking_budget` were dropped because no engine adapter ever consumed them
/// (they were collected, persisted, and never passed to any CLI).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instance {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub icon: String,
    pub group: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_profile")]
    pub profile: String,
    /// LLM provider ("the other half" of the model). Meaning is engine-specific
    /// (dsh's `deepseek-official` ≠ pi's `google`); the launcher passes it
    /// through verbatim to the selected engine. Missing ⇒ empty (engine default).
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub model: String,
    /// Which stored API key this instance launches with: `"<provider>"` to rotate
    /// that provider's enabled keys, `"<provider>/<alias>"` to pin one, empty to let
    /// the launcher fall back to matching `provider` against a provider id (and
    /// inject nothing if that finds no match). A *reference*, never a secret — values
    /// live only in `~/.agentlauncher/providers.json`. See `providers::dispatch`.
    #[serde(default)]
    pub api_key_ref: String,
    #[serde(default)]
    pub default_task: String,
    #[serde(default)]
    pub runtime: RuntimeConfig,
    pub created_at: String,
}

/// Create payload — mirrors `NewInstance` in src/types.ts.
#[derive(Debug, Clone, Deserialize)]
pub struct NewInstance {
    pub name: String,
    #[serde(default = "default_icon")]
    pub icon: String,
    #[serde(default)]
    pub group: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_profile")]
    pub profile: String,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub model: String,
    /// Optional at creation — the New Instance dialog offers the picker only once a
    /// provider list is loaded, and an unbound instance still launches (see
    /// `providers::dispatch`).
    #[serde(default)]
    pub api_key_ref: String,
    #[serde(default)]
    pub default_task: String,
    #[serde(default)]
    pub runtime: RuntimeConfig,
}

fn default_profile() -> String {
    "headless".to_string()
}
fn default_icon() -> String {
    "bot".to_string()
}
fn default_schema_version() -> u32 {
    1
}

/// `~/.agentlauncher/instances`
pub fn instances_root() -> Result<PathBuf, String> {
    Ok(crate::launcher_config::agentlauncher_root()?.join("instances"))
}

pub fn instance_dir(id: &str) -> Result<PathBuf, String> {
    // Guard against path traversal in the id.
    if id.is_empty() || id.contains('/') || id.contains('\\') || id.contains("..") {
        return Err(format!("invalid instance id: {id}"));
    }
    Ok(instances_root()?.join(id))
}

pub fn workspace_dir(id: &str) -> Result<PathBuf, String> {
    Ok(instance_dir(id)?.join("workspace"))
}

fn slugify(name: &str) -> String {
    let mut s: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    while s.contains("--") {
        s = s.replace("--", "-");
    }
    let s = s.trim_matches('-').to_string();
    if s.is_empty() {
        "agent".to_string()
    } else {
        s
    }
}

pub fn list_instances() -> Result<Vec<Instance>, String> {
    let root = instances_root()?;
    if !root.exists() {
        return Ok(vec![]);
    }
    let mut out = vec![];
    for entry in fs::read_dir(&root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.path().is_dir() {
            continue;
        }
        let cfg = entry.path().join("instance.json");
        if let Ok(text) = fs::read_to_string(&cfg) {
            match serde_json::from_str::<Instance>(&text) {
                Ok(inst) => out.push(inst),
                Err(e) => eprintln!("skip {}: {e}", cfg.display()),
            }
        }
    }
    out.sort_by(|a, b| a.group.cmp(&b.group).then(a.name.cmp(&b.name)));
    Ok(out)
}

pub fn get_instance(id: &str) -> Result<Instance, String> {
    let cfg = instance_dir(id)?.join("instance.json");
    let text = fs::read_to_string(&cfg).map_err(|e| format!("{id}: {e}"))?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

fn write_instance_json(inst: &Instance) -> Result<(), String> {
    let cfg = instance_dir(&inst.id)?.join("instance.json");
    let text = serde_json::to_string_pretty(inst).map_err(|e| e.to_string())?;
    fs::write(&cfg, text).map_err(|e| e.to_string())
}

pub fn create_instance(payload: NewInstance) -> Result<Instance, String> {
    let root = instances_root()?;
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;

    // Unique id from the name.
    let base = slugify(&payload.name);
    let mut id = base.clone();
    let mut n = 2;
    while root.join(&id).exists() {
        id = format!("{base}-{n}");
        n += 1;
    }

    let dir = root.join(&id);
    for sub in ["workspace", "skills", "logs"] {
        fs::create_dir_all(dir.join(sub)).map_err(|e| e.to_string())?;
    }

    let group = if payload.group.is_empty() {
        "未分类".to_string()
    } else {
        payload.group
    };

    let inst = Instance {
        schema_version: default_schema_version(),
        id: id.clone(),
        name: payload.name,
        icon: payload.icon,
        group,
        description: payload.description,
        profile: payload.profile,
        provider: payload.provider,
        model: payload.model,
        // Empty is the normal case: an unbound instance falls back to matching
        // `provider` against a provider id, and injects nothing if that misses.
        api_key_ref: payload.api_key_ref.trim().to_string(),
        default_task: payload.default_task,
        runtime: payload.runtime,
        created_at: Utc::now().to_rfc3339(),
    };

    write_instance_json(&inst)?;

    // Default scaffold files.
    let agents_md = format!(
        "# {}\n\n你是一个运行在 agentlauncher 沙箱中的 AI Agent。\n\n## 行为守则\n- 只在 workspace/ 目录内进行文件读写。\n- 执行高危命令前请说明意图。\n",
        inst.name
    );
    fs::write(dir.join("AGENTS.md"), agents_md).map_err(|e| e.to_string())?;

    // The instance `.env` is where *every* engine's credentials land (executor
    // injects it into the child). It used to hint `DEEPSEEK_API_KEY` alone, which
    // is doubly wrong: dsh keeps its keys in ~/.dsh/.credentials.yaml, and the
    // other five engines each read their own variables — so point at the engine
    // instead of naming one vendor's.
    let env = "# 该实例专属的环境变量与 API Keys —— 启动时注入子进程，绝不回流到界面。\n\
               # 按所选框架填写它自己读的变量（例如 claude 读 ANTHROPIC_API_KEY / ANTHROPIC_BASE_URL），\n\
               # 变量名见该框架自己的文档；dsh 的凭据另存于 ~/.dsh/.credentials.yaml。\n";
    fs::write(dir.join(".env"), env).map_err(|e| e.to_string())?;

    fs::write(dir.join("mcp.json"), "{\n  \"servers\": {}\n}\n").map_err(|e| e.to_string())?;

    Ok(inst)
}

pub fn update_instance(inst: Instance) -> Result<Instance, String> {
    if !instance_dir(&inst.id)?.exists() {
        return Err(format!("instance not found: {}", inst.id));
    }
    write_instance_json(&inst)?;
    Ok(inst)
}

pub fn delete_instance(id: &str) -> Result<(), String> {
    let dir = instance_dir(id)?;
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{temp_tree, EnvGuard, HOME_LOCK};

    #[test]
    fn create_web_instance_scaffold_under_agentlauncher_home() {
        let _g = HOME_LOCK.lock().unwrap();
        let tree = temp_tree("home");
        let home = tree.path();
        let _home_env = EnvGuard::set("HOME", home);

        let inst = create_instance(NewInstance {
            name: "Web E2E".into(),
            icon: default_icon(),
            group: String::new(),
            description: String::new(),
            profile: "web".into(),
            provider: String::new(),
            model: String::new(),
            api_key_ref: String::new(),
            default_task: String::new(),
            runtime: RuntimeConfig::default(),
        })
        .expect("create_instance should succeed");

        assert_eq!(inst.profile, "web");

        let dir = instance_dir(&inst.id).unwrap();
        // Data lives under the renamed ~/.agentlauncher tree.
        assert!(
            dir.starts_with(home.join(".agentlauncher")),
            "instance dir {dir:?} must live under ~/.agentlauncher"
        );
        for f in ["instance.json", "AGENTS.md", ".env", "mcp.json"] {
            assert!(dir.join(f).exists(), "missing scaffold file {f}");
        }
        for d in ["workspace", "skills", "logs"] {
            assert!(dir.join(d).is_dir(), "missing scaffold dir {d}");
        }
        // The agent system prompt reflects the rename.
        let agents = fs::read_to_string(dir.join("AGENTS.md")).unwrap();
        assert!(
            agents.contains("agentlauncher"),
            "AGENTS.md should reference agentlauncher"
        );
        // The `.env` template is engine-agnostic: it must not preselect a vendor's
        // key name, since every engine reads its own variables from this file.
        let env = fs::read_to_string(dir.join(".env")).unwrap();
        assert!(
            !env.contains("DEEPSEEK_API_KEY"),
            ".env template must not hardcode one vendor's key: {env}"
        );
        // Round-trips through the read path used by the UI.
        assert_eq!(get_instance(&inst.id).unwrap().id, inst.id);
        assert!(list_instances().unwrap().iter().any(|i| i.id == inst.id));
    }

    /// An older `instance.json` written before schema versioning omits the field;
    /// it must deserialize to schema_version 1 (backward compatible).
    #[test]
    fn instance_missing_schema_version_defaults_to_1() {
        let json = r#"{"id":"x","name":"X","icon":"bot","group":"g","profile":"headless","created_at":"1970-01-01T00:00:00Z"}"#;
        let inst: Instance = serde_json::from_str(json).unwrap();
        assert_eq!(inst.schema_version, 1);
    }

    /// An `instance.json` written before the runtime/environment override omits
    /// the `runtime` block; it must deserialize to the autodetect default with
    /// no custom binary (backward compatible, no schema bump needed).
    #[test]
    fn instance_missing_runtime_defaults() {
        let json = r#"{"id":"x","name":"X","icon":"bot","group":"g","profile":"headless","created_at":"1970-01-01T00:00:00Z"}"#;
        let inst: Instance = serde_json::from_str(json).unwrap();
        assert_eq!(inst.runtime.env_policy, "autodetect");
        assert!(inst.runtime.custom_bin.is_empty());
    }

    /// An `instance.json` written before multi-engine omits `runtime.engine`
    /// (and may omit `runtime` entirely); it must default to the dsh engine so
    /// existing instances keep launching dsh.
    #[test]
    fn runtime_missing_engine_defaults_to_dsh() {
        // runtime present but without `engine`
        let json = r#"{"id":"x","name":"X","icon":"bot","group":"g","profile":"headless","runtime":{"env_policy":"isolated","custom_bin":""},"created_at":"1970-01-01T00:00:00Z"}"#;
        let inst: Instance = serde_json::from_str(json).unwrap();
        assert_eq!(inst.runtime.engine, "dsh");
        // runtime absent entirely
        let json2 = r#"{"id":"x","name":"X","icon":"bot","group":"g","profile":"headless","created_at":"1970-01-01T00:00:00Z"}"#;
        let inst2: Instance = serde_json::from_str(json2).unwrap();
        assert_eq!(inst2.runtime.engine, "dsh");
    }

    /// An `instance.json` written before per-instance provider omits it; it must
    /// deserialize to an empty provider (the engine then uses its own default).
    #[test]
    fn instance_missing_provider_defaults() {
        let json = r#"{"id":"x","name":"X","icon":"bot","group":"g","profile":"headless","created_at":"1970-01-01T00:00:00Z"}"#;
        let inst: Instance = serde_json::from_str(json).unwrap();
        assert!(inst.provider.is_empty());
    }

    /// `temperature` / `thinking_budget` were retired (no engine adapter ever read
    /// them). An older file still carrying them must load, not error — retiring a
    /// field is backward compatible because unknown keys are ignored.
    #[test]
    fn instance_with_retired_fields_still_loads() {
        let json = r#"{"id":"x","name":"X","icon":"bot","group":"g","profile":"headless","model":"m","temperature":0.2,"thinking_budget":4096,"created_at":"1970-01-01T00:00:00Z"}"#;
        let inst: Instance = serde_json::from_str(json).unwrap();
        assert_eq!(inst.model, "m");
    }
}
