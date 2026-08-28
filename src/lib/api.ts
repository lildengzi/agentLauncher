import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  DshProfile,
  EngineInfo,
  InstGroups,
  Instance,
  LauncherConfig,
  McpPlugin,
  NewInstance,
  RuntimeLogEvent,
  RuntimeStatusEvent,
} from "@/types";

// ---- Tauri commands -------------------------------------------------------

export const api = {
  listInstances: () => invoke<Instance[]>("list_instances"),
  getInstance: (id: string) => invoke<Instance>("get_instance", { id }),
  createInstance: (payload: NewInstance) =>
    invoke<Instance>("create_instance", { payload }),
  updateInstance: (instance: Instance) =>
    invoke<Instance>("update_instance", { instance }),
  deleteInstance: (id: string) => invoke<void>("delete_instance", { id }),

  startInstance: (id: string, task?: string) =>
    invoke<void>("start_instance", { id, task: task ?? null }),
  stopInstance: (id: string) => invoke<void>("stop_instance", { id }),

  openInstanceFolder: (id: string) =>
    invoke<void>("open_instance_folder", { id }),

  /** Open a URL (e.g. a web instance's served UI) in the default browser. */
  openUrl: (url: string) => invoke<void>("open_url", { url }),

  listMcpCatalog: () => invoke<McpPlugin[]>("list_mcp_catalog"),

  /** Live-probe which known agent engines (CLIs) are installed on the host. */
  detectEngines: () => invoke<EngineInfo[]>("detect_engines"),

  // ---- real dsh config wiring --------------------------------------------
  /** Names (not values) of credentials stored in ~/.dsh/.credentials.yaml. */
  listCredentialKeys: () => invoke<string[]>("list_credential_keys"),
  /** Upsert (or, with an empty value, remove) a credential in .credentials.yaml. */
  setCredential: (key: string, value: string) =>
    invoke<void>("set_credential", { key, value }),
  /** Profiles under ~/.dsh/profiles, each with its web capability resolved. */
  listDshProfiles: () => invoke<DshProfile[]>("list_dsh_profiles"),
  /** npm package names installed in a profile (its package.json dependencies). */
  listInstalledPlugins: (profile: string) =>
    invoke<string[]>("list_installed_plugins", { profile }),
  /** `dsh plugin --profile <p> add <pkg>` — real pnpm install. */
  pluginAdd: (profile: string, pkg: string) =>
    invoke<string>("plugin_add", { profile, pkg }),
  /** `dsh plugin --profile <p> remove <pkg>`. */
  pluginRemove: (profile: string, pkg: string) =>
    invoke<string>("plugin_remove", { profile, pkg }),

  // ---- launcher contract (~/.agentlauncher/config.json, instgroups.json) --
  /** Read the launcher config; backend returns defaults if the file is absent. */
  getLauncherConfig: () => invoke<LauncherConfig>("get_launcher_config"),
  /** Persist the whole launcher config document (no secrets). */
  setLauncherConfig: (config: LauncherConfig) =>
    invoke<void>("set_launcher_config", { config }),
  /** Read the sidebar grouping overlay; defaults if absent. */
  getInstGroups: () => invoke<InstGroups>("get_inst_groups"),
  /** Persist the sidebar grouping overlay (order / collapse / intra-group order). */
  setInstGroups: (groups: InstGroups) =>
    invoke<void>("set_inst_groups", { groups }),
};

// ---- Events ---------------------------------------------------------------
// Emitted by src-tauri/src/executor.rs for every engine — the executor is
// agent-agnostic, so the events are named after the seam, not after dsh.

export function onRuntimeLog(
  cb: (e: RuntimeLogEvent) => void
): Promise<UnlistenFn> {
  return listen<RuntimeLogEvent>("runtime-log", (evt) => cb(evt.payload));
}

export function onRuntimeStatus(
  cb: (e: RuntimeStatusEvent) => void
): Promise<UnlistenFn> {
  return listen<RuntimeStatusEvent>("runtime-status", (evt) => cb(evt.payload));
}
