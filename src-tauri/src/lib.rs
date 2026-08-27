mod dsh_config;
mod dsh_runner;
mod instance_manager;

use dsh_runner::RunnerState;
use instance_manager::{Instance, NewInstance};
use serde_json::json;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

// ---- instance CRUD --------------------------------------------------------

#[tauri::command]
fn list_instances() -> Result<Vec<Instance>, String> {
    instance_manager::list_instances()
}

#[tauri::command]
fn get_instance(id: String) -> Result<Instance, String> {
    instance_manager::get_instance(&id)
}

#[tauri::command]
fn create_instance(payload: NewInstance) -> Result<Instance, String> {
    instance_manager::create_instance(payload)
}

#[tauri::command]
fn update_instance(instance: Instance) -> Result<Instance, String> {
    instance_manager::update_instance(instance)
}

#[tauri::command]
fn delete_instance(id: String) -> Result<(), String> {
    instance_manager::delete_instance(&id)
}

// ---- run control ----------------------------------------------------------

#[tauri::command]
async fn start_instance(
    app: AppHandle,
    state: State<'_, RunnerState>,
    id: String,
    task: Option<String>,
) -> Result<(), String> {
    dsh_runner::start(app, state, id, task).await
}

#[tauri::command]
async fn stop_instance(state: State<'_, RunnerState>, id: String) -> Result<(), String> {
    dsh_runner::stop(state, id).await
}

// ---- misc -----------------------------------------------------------------

#[tauri::command]
fn open_instance_folder(app: AppHandle, id: String) -> Result<(), String> {
    let dir = instance_manager::instance_dir(&id)?;
    app.opener()
        .open_path(dir.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// Curated plugin catalog for the Hub. dsh has no remote plugin market, so
/// discovery is a curated list; entries carrying a real `package` (npm name) can
/// be installed/removed for real via `dsh plugin add|remove` (see dsh_config).
/// The frontend cross-references `list_installed_plugins` for live installed state.
#[tauri::command]
fn list_mcp_catalog() -> serde_json::Value {
    json!([
        {
            "id": "mcp-client",
            "name": "MCP Client",
            "author": "@deepseek-ai",
            "package": "@deepseek-ai/dsh-mcp-client",
            "description": "接入任意 Model Context Protocol 服务器（工具 / 资源 / 提示词）",
            "icon": "plug",
            "category": "modrinth",
            "version": "latest",
            "installed": false,
            "links": [{ "label": "npm", "url": "https://www.npmjs.com/package/@deepseek-ai/dsh-mcp-client" }]
        },
        {
            "id": "web-search-deepseek",
            "name": "Web Search (DeepSeek)",
            "author": "@deepseek-ai",
            "package": "@deepseek-ai/dsh-web-search-deepseek",
            "description": "DeepSeek 官方联网搜索工具，供 Agent 检索实时信息",
            "icon": "search",
            "category": "modrinth",
            "version": "latest",
            "installed": false,
            "links": [{ "label": "npm", "url": "https://www.npmjs.com/package/@deepseek-ai/dsh-web-search-deepseek" }]
        },
        {
            "id": "tool-web",
            "name": "Web Fetch Tool",
            "author": "@deepseek-ai",
            "package": "@deepseek-ai/dsh-tool-web",
            "description": "抓取网页并转为可读文本，支持 Agent 直接读取 URL 内容",
            "icon": "globe",
            "category": "modrinth",
            "version": "latest",
            "installed": false,
            "links": [{ "label": "npm", "url": "https://www.npmjs.com/package/@deepseek-ai/dsh-tool-web" }]
        },
        {
            "id": "skill-filesystem",
            "name": "Filesystem Skills",
            "author": "@deepseek-ai",
            "package": "@deepseek-ai/dsh-skill-filesystem",
            "description": "从磁盘目录加载 Skill 定义，扩展 Agent 的技能库",
            "icon": "folder-tree",
            "category": "github",
            "version": "latest",
            "installed": false,
            "links": [{ "label": "npm", "url": "https://www.npmjs.com/package/@deepseek-ai/dsh-skill-filesystem" }]
        },
        {
            "id": "schedule",
            "name": "Schedule",
            "author": "@deepseek-ai",
            "package": "@deepseek-ai/dsh-schedule",
            "description": "为 Agent 提供定时 / 延时任务调度能力",
            "icon": "alarm-clock",
            "category": "github",
            "version": "latest",
            "installed": false,
            "links": [{ "label": "npm", "url": "https://www.npmjs.com/package/@deepseek-ai/dsh-schedule" }]
        }
    ])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(RunnerState::default())
        .invoke_handler(tauri::generate_handler![
            list_instances,
            get_instance,
            create_instance,
            update_instance,
            delete_instance,
            start_instance,
            stop_instance,
            open_instance_folder,
            list_mcp_catalog,
            dsh_config::list_credential_keys,
            dsh_config::set_credential,
            dsh_config::list_dsh_profiles,
            dsh_config::list_installed_plugins,
            dsh_config::plugin_add,
            dsh_config::plugin_remove,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
