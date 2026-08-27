import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  DshLogEvent,
  DshStatusEvent,
  Instance,
  McpPlugin,
  NewInstance,
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

  /** Open a URL (e.g. a web instance's dsh UI) in the default browser. */
  openUrl: (url: string) => invoke<void>("open_url", { url }),

  listMcpCatalog: () => invoke<McpPlugin[]>("list_mcp_catalog"),

  // ---- real dsh config wiring --------------------------------------------
  /** Names (not values) of credentials stored in ~/.dsh/.credentials.yaml. */
  listCredentialKeys: () => invoke<string[]>("list_credential_keys"),
  /** Upsert (or, with an empty value, remove) a credential in .credentials.yaml. */
  setCredential: (key: string, value: string) =>
    invoke<void>("set_credential", { key, value }),
  /** Profile names under ~/.dsh/profiles. */
  listDshProfiles: () => invoke<string[]>("list_dsh_profiles"),
  /** npm package names installed in a profile (its package.json dependencies). */
  listInstalledPlugins: (profile: string) =>
    invoke<string[]>("list_installed_plugins", { profile }),
  /** `dsh plugin --profile <p> add <pkg>` — real pnpm install. */
  pluginAdd: (profile: string, pkg: string) =>
    invoke<string>("plugin_add", { profile, pkg }),
  /** `dsh plugin --profile <p> remove <pkg>`. */
  pluginRemove: (profile: string, pkg: string) =>
    invoke<string>("plugin_remove", { profile, pkg }),
};

// ---- Events ---------------------------------------------------------------

export function onDshLog(cb: (e: DshLogEvent) => void): Promise<UnlistenFn> {
  return listen<DshLogEvent>("dsh-log", (evt) => cb(evt.payload));
}

export function onDshStatus(
  cb: (e: DshStatusEvent) => void
): Promise<UnlistenFn> {
  return listen<DshStatusEvent>("dsh-status", (evt) => cb(evt.payload));
}
