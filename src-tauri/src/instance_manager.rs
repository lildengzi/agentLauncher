use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// One Agent instance — mirrors `Instance` in src/types.ts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub group: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_profile")]
    pub profile: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub temperature: f32,
    #[serde(default)]
    pub thinking_budget: u32,
    #[serde(default)]
    pub default_task: String,
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
    pub model: String,
    #[serde(default)]
    pub temperature: f32,
    #[serde(default)]
    pub thinking_budget: u32,
    #[serde(default)]
    pub default_task: String,
}

fn default_profile() -> String {
    "headless".to_string()
}
fn default_icon() -> String {
    "bot".to_string()
}

/// `~/.dsh-launcher/instances`
pub fn instances_root() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("cannot resolve home directory")?;
    Ok(home.join(".dsh-launcher").join("instances"))
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
        id: id.clone(),
        name: payload.name,
        icon: payload.icon,
        group,
        description: payload.description,
        profile: payload.profile,
        model: payload.model,
        temperature: payload.temperature,
        thinking_budget: payload.thinking_budget,
        default_task: payload.default_task,
        created_at: Utc::now().to_rfc3339(),
    };

    write_instance_json(&inst)?;

    // Default scaffold files.
    let agents_md = format!(
        "# {}\n\n你是一个运行在 dsh-launcher 沙箱中的 AI Agent。\n\n## 行为守则\n- 只在 workspace/ 目录内进行文件读写。\n- 执行高危命令前请说明意图。\n",
        inst.name
    );
    fs::write(dir.join("AGENTS.md"), agents_md).map_err(|e| e.to_string())?;

    let env = "# 该实例专属的 API Keys 与环境变量\n# DEEPSEEK_API_KEY=\n";
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
