mod engines;
mod executor;
mod instance_ext;
mod instance_manager;
mod launcher_config;
mod market;
mod providers;
mod runtime;
#[cfg(test)]
mod test_support;

use executor::RunnerState;
use instance_manager::{Instance, NewInstance};
use tauri::{AppHandle, Manager, State};
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
    executor::start(app, state, id, task).await
}

#[tauri::command]
async fn stop_instance(state: State<'_, RunnerState>, id: String) -> Result<(), String> {
    executor::stop(state, id).await
}

// ---- misc -----------------------------------------------------------------

#[tauri::command]
fn open_instance_folder(app: AppHandle, id: String) -> Result<(), String> {
    let dir = instance_manager::instance_dir(&id)?;
    app.opener()
        .open_path(dir.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// Open a URL in the default browser (e.g. the web-UI URL an engine prints when a
/// serve-mode instance starts; dsh is the only one wired for that today). The
/// launcher hosts no agent UI of its own — interaction happens in the engine's own
/// page, and the launcher's log view stays read-only.
#[tauri::command]
fn open_url(app: AppHandle, url: String) -> Result<(), String> {
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

/// Open — or focus — the standalone editor window for one instance.
///
/// One window per instance, keyed by the label `edit-<id>`. The label *is* the
/// identity: a second call for the same instance focuses the window already on
/// screen instead of opening a duplicate, which Tauri would reject anyway
/// (`WindowLabelAlreadyExists`). It is also how the window learns which instance it
/// edits, so nothing is passed in the URL — `is_label_valid` accepts exactly the
/// characters `slugify` can put in an id (both are `char::is_alphanumeric`, both
/// Unicode-aware), so a CJK-named instance round-trips with no encoding.
///
/// Built here rather than from JavaScript on purpose: window creation stays a
/// backend decision, so the frontend never needs permission to spawn a webview at
/// an arbitrary URL. The editor window's own capability is in
/// capabilities/default.json.
#[tauri::command]
fn open_edit_window(app: AppHandle, id: String) -> Result<(), String> {
    // Fail before opening an empty window if the instance is gone.
    let inst = instance_manager::get_instance(&id)?;
    let label = format!("edit-{id}");
    if let Some(win) = app.get_webview_window(&label) {
        win.unminimize().ok(); // a minimized window would "focus" invisibly
        return win.set_focus().map_err(|e| e.to_string());
    }
    tauri::WebviewWindowBuilder::new(&app, &label, tauri::WebviewUrl::App("edit.html".into()))
        .title(format!("{} — agentLauncher", inst.name))
        .inner_size(900.0, 680.0)
        .min_inner_size(720.0, 520.0)
        .build()
        .map_err(|e| e.to_string())?;
    Ok(())
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
            open_url,
            open_edit_window,
            engines::detect_engines,
            instance_ext::read_instance_extensions,
            instance_ext::read_instance_agents,
            instance_ext::write_instance_agents,
            instance_ext::set_instance_mcp,
            instance_ext::remove_instance_skill,
            instance_ext::open_instance_subdir,
            market::market_fetch,
            market::market_refresh,
            market::market_readme,
            market::install::market_install,
            market::install::market_uninstall,
            market::sources::get_market_sources,
            market::sources::set_market_sources,
            providers::get_providers,
            providers::set_providers,
            providers::set_provider_key,
            providers::detect::detect_local_llms,
            providers::detect::fetch_provider_models,
            runtime::dsh_home::list_credential_keys,
            runtime::dsh_home::set_credential,
            runtime::dsh_home::list_dsh_profiles,
            runtime::dsh_home::list_installed_plugins,
            runtime::dsh_home::plugin_add,
            runtime::dsh_home::plugin_remove,
            launcher_config::get_launcher_config,
            launcher_config::set_launcher_config,
            launcher_config::get_inst_groups,
            launcher_config::set_inst_groups,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
