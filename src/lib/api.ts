import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  DshProfile,
  EngineInfo,
  InstGroups,
  Instance,
  InstallSpec,
  InstanceExtensions,
  LauncherConfig,
  MarketPage,
  MarketQuery,
  McpServerEntry,
  NewInstance,
  RuntimeLogEvent,
  RuntimeStatusEvent,
  SourceStatus,
  SourcesDoc,
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

  /** Live-probe which known agent engines (CLIs) are installed on the host. */
  detectEngines: () => invoke<EngineInfo[]>("detect_engines"),

  // ---- per-instance extensions (edit dialog's three sections) -------------
  /**
   * Plugins + skills + MCP servers for one instance, in one round trip.
   * `engine`/`profile` override the saved values so the edit dialog can ask about
   * the form the user is looking at rather than the last thing written to disk.
   */
  readInstanceExtensions: (id: string, engine?: string, profile?: string) =>
    invoke<InstanceExtensions>("read_instance_extensions", {
      id,
      engine: engine ?? null,
      profile: profile ?? null,
    }),
  /** Replace the whole `mcpServers` map of an instance's mcp.json. */
  setInstanceMcp: (id: string, servers: McpServerEntry[]) =>
    invoke<void>("set_instance_mcp", { id, servers }),
  /** Delete one skill directory under `instances/<id>/skills/`. */
  removeInstanceSkill: (id: string, name: string) =>
    invoke<void>("remove_instance_skill", { id, name }),
  /** Reveal one of an instance's own subdirectories ("skills"|"workspace"|"logs"). */
  openInstanceSubdir: (id: string, sub: string) =>
    invoke<void>("open_instance_subdir", { id, sub }),

  // ---- decentralized market ----------------------------------------------
  /** Query every enabled source that serves `query.kind`, merged and sorted. */
  marketFetch: (query: MarketQuery) => invoke<MarketPage>("market_fetch", { query }),
  /** Force a refetch past the cache; omit the id to refresh every source. */
  marketRefresh: (sourceId?: string) =>
    invoke<SourceStatus[]>("market_refresh", { sourceId: sourceId ?? null }),
  /** Lazily fetch one item's detail Markdown for the right-hand pane. */
  marketReadme: (itemId: string) => invoke<string>("market_readme", { itemId }),
  /** Run an item's install for real; returns a short note (path/package/server). */
  marketInstall: (instanceId: string, name: string, spec: InstallSpec) =>
    invoke<string>("market_install", { instanceId, name, spec }),
  /** Undo `marketInstall` for the same item. */
  marketUninstall: (instanceId: string, name: string, spec: InstallSpec) =>
    invoke<string>("market_uninstall", { instanceId, name, spec }),
  /** The source list; built-ins are re-seeded on read. */
  getMarketSources: () => invoke<SourcesDoc>("get_market_sources"),
  /** Persist the source list (built-ins are restored, `builtin` is not caller-set). */
  setMarketSources: (doc: SourcesDoc) => invoke<void>("set_market_sources", { doc }),

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
