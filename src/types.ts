// ---------------------------------------------------------------------------
// Shared front/back contract. Mirrors the Rust structs in
// src-tauri/src/instance_manager.rs and the events emitted by executor.rs.
// Keep this file and the Rust side in sync.
// ---------------------------------------------------------------------------

/** Per-instance runtime/environment override. Mirrors `RuntimeConfig` in
 *  src-tauri/src/instance_manager.rs. Governs which agent CLI ("framework") runs
 *  and how the host resolves its binary and the child PATH; contains no secrets. */
export interface RuntimeConfig {
  /** which agent CLI to launch: "dsh"|"pi"|"omp"|"claude"|"codex"|"opencode" (missing/empty ⇒ "dsh"). */
  engine: string;
  /** "autodetect" (enrich PATH from the login shell) | "isolated" (minimal PATH). */
  env_policy: string;
  /** absolute path to the agent CLI; overrides PATH lookup when non-empty. */
  custom_bin: string;
}

/** One engine's install status (from the `detect_engines` command). Mirrors
 *  `EngineInfo` in src-tauri/src/engines.rs. */
export interface EngineInfo {
  id: string;
  display: string;
  /** whether the launcher wires a web/serve mode for this engine (dsh only). */
  web: boolean;
  /** whether a provider reaches this engine as a launch flag. False for claude,
   *  which takes provider / base URL / key only from ANTHROPIC_* in the instance
   *  `.env` — the dialog hides the field instead of collecting a dropped value. */
  takes_provider: boolean;
  installed: boolean;
  /** absolute path of the resolved binary, or "" when not found. */
  path: string;
}

/** One dsh profile — mirrors `DshProfile` in src-tauri/src/runtime/dsh_home.rs.
 *  `web` is resolved by the backend from the profile's bundled packages, never
 *  guessed from the name. */
export interface DshProfile {
  name: string;
  web: boolean;
}

/** An Agent instance — one agent CLI + workspace, isolated in its own directory.
 *
 *  Retired fields (`temperature`, `thinking_budget`) are simply absent: no engine
 *  adapter ever passed them to a CLI. Older `instance.json` files still carrying
 *  them load fine — the backend ignores unknown keys, so no schema bump. */
export interface Instance {
  /** on-disk contract version for instance.json (missing ⇒ 1). */
  schema_version: number;
  id: string;
  name: string;
  /** lucide icon name, e.g. "code", "flask-conical", "globe". */
  icon: string;
  /** category shown as a collapsible group in the sidebar. */
  group: string;
  description: string;
  /** dsh profile to boot (default "headless"). Only meaningful for engine "dsh". */
  profile: string;
  /** LLM provider (the other half of the model); meaning is engine-specific. */
  provider: string;
  /** underlying model target, e.g. "deepseek-reasoner". */
  model: string;
  /** default task text prefilled when launching. */
  default_task: string;
  /** runtime/environment override (missing ⇒ autodetect, no custom binary). */
  runtime: RuntimeConfig;
  /** RFC3339 timestamp. */
  created_at: string;
}

/** Payload for creating a new instance. Server fills id/created_at. */
export interface NewInstance {
  name: string;
  icon: string;
  group: string;
  description: string;
  profile: string;
  provider: string;
  model: string;
  default_task: string;
  runtime: RuntimeConfig;
}

export type RunStatus = "idle" | "starting" | "running" | "exited" | "error";

/** `runtime-status` event payload (every engine, not just dsh). */
export interface RuntimeStatusEvent {
  instanceId: string;
  status: RunStatus;
  /** process exit code, present on "exited". */
  code?: number | null;
  /** human-readable message, present on "error". */
  message?: string | null;
  /** served web-UI URL, present on the "running" event of a web (serve) instance. */
  url?: string | null;
}

/** `runtime-log` event payload — a raw chunk to feed straight into xterm. */
export interface RuntimeLogEvent {
  instanceId: string;
  stream: "stdout" | "stderr";
  chunk: string;
}

// ---------------------------------------------------------------------------
// Launcher-level contract. Mirrors src-tauri/src/launcher_config.rs.
// Two backend-owned versioned files under ~/.agentlauncher/:
//   config.json      → LauncherConfig
//   instgroups.json  → InstGroups   (a presentation overlay, not source of truth)
// Secrets never live here — dsh's credentials stay in ~/.dsh/.credentials.yaml,
// every other engine's in the instance `.env`.
// ---------------------------------------------------------------------------

/** UI preferences (was localStorage: agentlauncher.{theme,locale}). */
export interface UiPrefs {
  theme: string;
  locale: string;
}

/** Launcher-wide agent defaults — prefill the New Instance dialog. Non-secret.
 *  Both default to empty: an empty value means "let the chosen engine use its
 *  own default", since provider naming differs per engine. `base_url` and
 *  `profile` were retired (no consumer / dsh-only with no UI). */
export interface AgentDefaults {
  provider: string;
  model: string;
}

/** Transient UX state restored across launches. */
export interface SessionState {
  selected_instance: string;
  last_used_group: string;
}

/** ~/.agentlauncher/config.json */
export interface LauncherConfig {
  format_version: number;
  ui: UiPrefs;
  defaults: AgentDefaults;
  session: SessionState;
}

/** Per-group presentation state; `instances` is a manual ordering overlay. */
export interface GroupState {
  collapsed: boolean;
  instances: string[];
}

/** ~/.agentlauncher/instgroups.json — sidebar grouping overlay. */
export interface InstGroups {
  format_version: number;
  /** group display order (top → bottom); unknown groups append. */
  order: string[];
  groups: Record<string, GroupState>;
}

// ---------------------------------------------------------------------------
// Per-instance extension state. Mirrors src-tauri/src/instance_ext.rs.
// The three edit-dialog sections (扩展插件 / 技能 Skills / MCP 服务器) read this
// in one call; each of the three lives wherever that kind of extension actually
// lives, which is not uniformly per-instance — see `plugin_scope`.
// ---------------------------------------------------------------------------

/** One MCP server. `name` is the `mcpServers` map key in the instance's
 *  `mcp.json`, carried inline so the UI edits a list rather than an object. */
export interface McpServerEntry {
  name: string;
  command: string;
  args: string[];
  env: Record<string, string>;
  /** kept in the file but not handed to the engine. */
  disabled: boolean;
}

/** One skill directory under `instances/<id>/skills/`. */
export interface SkillEntry {
  name: string;
  /** absolute path, so the UI can reveal it without rebuilding it. */
  path: string;
  /** first prose line of the skill's own SKILL.md/README.md, or "". */
  description: string;
}

export interface InstanceExtensions {
  plugins: string[];
  /** Who owns `plugins`: `"dsh-profile:<name>"` (shared by every instance on that
   *  profile — the UI must say so) or `"unsupported"` for engines with no
   *  readable plugin concept. */
  plugin_scope: string;
  skills: SkillEntry[];
  mcp: McpServerEntry[];
}

// ---------------------------------------------------------------------------
// Decentralized extension market. Mirrors src-tauri/src/market/{mod,sources}.rs.
// No single registry exists, so every index is a row in ~/.agentlauncher/
// sources.json and the backend normalises all of them into `MarketItem` before
// the UI sees them. Fetching is backend-side (a user-supplied URL owes us no CORS
// header, and the results are disk-cached so the dialog opens offline).
// ---------------------------------------------------------------------------

/** The three market dialogs are one widget with this field changed. */
export type ExtensionKind = "plugin" | "skill" | "mcp";

/** One index to consult. `adapter` names the payload shape, which is what lets a
 *  third-party feed join without the dialog knowing anything about it. */
export interface SourceDef {
  id: string;
  label: string;
  /** "http" (fetch `url`) | "dir" (read *.json under `url`; blank ⇒ the default
   *  `~/.agentlauncher/sources`). */
  kind: string;
  url: string;
  /** "agentlauncher" (our canonical `{items:[...]}`) | "dsh-market" |
   *  "mcp-registry" | "npm". */
  adapter: string;
  /** which kinds this source can answer for; others skip it entirely. */
  kinds: string[];
  enabled: boolean;
  /** shipped with the launcher: disable-able, not delete-able. */
  builtin: boolean;
}

/** ~/.agentlauncher/sources.json */
export interface SourcesDoc {
  format_version: number;
  sources: SourceDef[];
}

/** How an item is actually installed. `method` is a string, not a union, so an
 *  unknown method from a newer source degrades to "manual" instead of breaking
 *  the payload: "pnpm-profile" | "git-clone" | "mcp-config" | "manual". */
export interface InstallSpec {
  method: string;
  /** npm package, for "pnpm-profile". */
  package: string;
  /** git remote, for "git-clone". */
  repo: string;
  /** a command to display (never auto-run) for "manual" items. */
  command: string;
  /** env var *names* the item needs configured — never values. */
  env: string[];
  /** prefilled server definition for "mcp-config". */
  mcp?: McpServerEntry | null;
}

export interface MarketVersion {
  version: string;
  published_at: string;
  install: InstallSpec;
}

/** One market entry, whatever source it came from. Thin sources legitimately
 *  leave most fields empty — the UI must render a row with name alone. */
export interface MarketItem {
  /** `"<source id>:<native id>"`, unique across sources. */
  id: string;
  source: string;
  kind: string;
  name: string;
  author: string;
  description: string;
  /** detail-pane Markdown; often empty in a list payload, filled by `marketReadme`. */
  readme: string;
  /** lucide icon name for the row avatar. */
  icon: string;
  homepage: string;
  repo: string;
  tags: string[];
  license: string;
  downloads: number;
  /** RFC3339, or "" when the source does not say. */
  updated_at: string;
  /** newest first; empty ⇒ nothing installable, show it read-only. */
  versions: MarketVersion[];
}

/** Per-source outcome, returned with the results so a partial failure is visible
 *  rather than silently shrinking the list. */
export interface SourceStatus {
  id: string;
  ok: boolean;
  item_count: number;
  fetched_at: string;
  stale: boolean;
  error: string;
}

export interface MarketQuery {
  kind: string;
  query: string;
  /** empty ⇒ every enabled source serving `kind`. */
  sources: string[];
  tags: string[];
  /** "relevance" | "downloads" | "updated" | "name". */
  sort: string;
  offset: number;
  /** 0 ⇒ the backend's own page size. */
  limit: number;
}

export interface MarketPage {
  items: MarketItem[];
  /** total matches across sources, for paging (not `items.length`). */
  total: number;
  stale: boolean;
  statuses: SourceStatus[];
}

