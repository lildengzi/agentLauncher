import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AgentsDoc,
  DetectedProvider,
  DshProfile,
  EngineInfo,
  InstGroups,
  Instance,
  InstallSpec,
  InstanceExtensions,
  InstanceKeyView,
  LauncherConfig,
  LocalLlm,
  MarketPage,
  MarketQuery,
  McpServerEntry,
  NewInstance,
  ProviderView,
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

  /**
   * Open — or focus, if it is already up — the standalone editor window for one
   * instance. One window per instance; the backend keys them by the label
   * `edit-<id>` and builds them itself, so the frontend needs no permission to
   * create webviews.
   */
  openEditWindow: (id: string) => invoke<void>("open_edit_window", { id }),

  /**
   * Bring the main window forward with one settings page open — the route an
   * instance editor uses to reach the app-level key store, which has exactly one
   * editor on purpose (see `open_settings` in lib.rs).
   */
  openSettings: (page: string) => invoke<void>("open_settings", { page }),

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
  /**
   * The instance's `AGENTS.md` (system prompt / behaviour rules) and whether the
   * file is there at all. Its own call rather than part of
   * `readInstanceExtensions`: that one waits on a dsh plugin probe and is re-issued
   * on every engine/profile change, which would discard a half-typed prompt.
   */
  readInstanceAgents: (id: string) => invoke<AgentsDoc>("read_instance_agents", { id }),
  /** Overwrite `AGENTS.md` verbatim; creates it if the instance has none yet. */
  writeInstanceAgents: (id: string, text: string) =>
    invoke<void>("write_instance_agents", { id, text }),
  /** Delete one skill directory under `instances/<id>/skills/`. */
  removeInstanceSkill: (id: string, name: string) =>
    invoke<void>("remove_instance_skill", { id, name }),
  /** Reveal one of an instance's own subdirectories ("skills"|"workspace"|"logs"). */
  openInstanceSubdir: (id: string, sub: string) =>
    invoke<void>("open_instance_subdir", { id, sub }),
  /**
   * Whether this instance keeps a key of its own in `instances/<id>/.env`, and which
   * variable holds it. Masked: a fingerprint comes back, never the value.
   */
  getInstanceKey: (id: string) => invoke<InstanceKeyView>("get_instance_key", { id }),
  /**
   * Write (or, with an empty value, remove) one variable in this instance's `.env`.
   * The launcher layers that file last, so a key here wins over the shared store.
   */
  setInstanceKey: (id: string, varName: string, value: string) =>
    invoke<void>("set_instance_key", { id, var: varName, value }),

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

  // ---- providers & API keys (~/.agentlauncher/providers.json, 0600) -------
  /** Every provider, keys masked to a fingerprint. Never returns a secret. */
  getProviders: () => invoke<ProviderView[]>("get_providers"),
  /**
   * Persist provider metadata. Key *values* are not part of this payload and cannot
   * be — the frontend never received them; the backend carries each one over from
   * disk by `(provider id, alias)`. Renaming an alias therefore clears that key.
   */
  setProviders: (providers: ProviderView[]) => invoke<void>("set_providers", { providers }),
  /** Set one key's value, or delete the key by passing an empty string. */
  setProviderKey: (provider: string, alias: string, value: string) =>
    invoke<void>("set_provider_key", { provider, alias, value }),
  /** Probe loopback ports for Ollama / LM Studio / vLLM / llama.cpp. No credentials,
   *  no proxy, nothing leaves the machine. Runtimes that are not up are absent. */
  detectLocalLlms: () => invoke<LocalLlm[]>("detect_local_llms"),
  /**
   * Ask one provider's own API which models a stored key can see. The only outbound
   * request in the launcher that carries a secret — the key is read from disk in the
   * backend, so it still never passes through here. `alias` empty ⇒ the provider's
   * first enabled key. The list is returned, not saved.
   */
  fetchProviderModels: (provider: string, alias = "") =>
    invoke<string[]>("fetch_provider_models", { provider, alias }),
  /**
   * Providers the machine's *other* agents already have configured, read from their
   * own config files. Only agents on PATH are consulted; no disk scan, no network.
   * Returns `has_key`, never a key.
   */
  detectAgentProviders: () => invoke<DetectedProvider[]>("detect_agent_providers"),
  /**
   * Copy each named provider's detected key into `providers.json`. The value moves
   * disk-to-disk inside the backend — it is not sent from here and is not returned.
   * The provider row must already be saved. Resolves to how many keys landed.
   */
  importAgentProviderKeys: (providers: string[]) =>
    invoke<number>("import_agent_provider_keys", { providers }),

  // ---- real dsh config wiring --------------------------------------------
  /** Names (not values) of credentials stored in ~/.dsh/.credentials.yaml. */
  listCredentialKeys: () => invoke<string[]>("list_credential_keys"),
  /** Upsert (or, with an empty value, remove) a credential in .credentials.yaml. */
  setCredential: (key: string, value: string) =>
    invoke<void>("set_credential", { key, value }),
  /** Profiles under ~/.dsh/profiles, each with its web capability resolved. */
  listDshProfiles: () => invoke<DshProfile[]>("list_dsh_profiles"),
  /**
   * Provider **routes** a dsh run can resolve a model on — dsh's own namespace, not
   * the launcher's provider ids. Always contains `deepseek-official` (the native
   * adapter's route); anything else was registered by `llm-pi-ai: providers:` in
   * `$DSH_HOME/settings.yaml`, which is what dsh's web Models page writes.
   */
  listDshModelRoutes: () => invoke<string[]>("list_dsh_model_routes"),
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

/** An editor window asked the main window to show one settings page (see
 *  `api.openSettings`). The payload is the page name; the main window is the only
 *  listener, because it is the only window that owns the settings surface. */
export function onOpenSettings(cb: (page: string) => void): Promise<UnlistenFn> {
  return listen<string>("open-settings", (evt) => cb(evt.payload));
}
