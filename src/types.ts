// ---------------------------------------------------------------------------
// Shared front/back contract. Mirrors the Rust structs in
// src-tauri/src/instance_manager.rs and the events emitted by dsh_runner.rs.
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

/** One dsh profile — mirrors `DshProfile` in src-tauri/src/dsh_config.rs.
 *  `web` is resolved by the backend from the profile's bundled packages, never
 *  guessed from the name. */
export interface DshProfile {
  name: string;
  web: boolean;
}

/** An Agent instance — one isolated dsh profile + workspace. */
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
  temperature: number;
  thinking_budget: number;
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
  temperature: number;
  thinking_budget: number;
  default_task: string;
  runtime: RuntimeConfig;
}

export type RunStatus = "idle" | "starting" | "running" | "exited" | "error";

/** dsh-status event payload. */
export interface DshStatusEvent {
  instanceId: string;
  status: RunStatus;
  /** process exit code, present on "exited". */
  code?: number | null;
  /** human-readable message, present on "error". */
  message?: string | null;
  /** dsh web-UI URL, present on the "running" event of a web (serve) instance. */
  url?: string | null;
}

/** dsh-log event payload — a raw chunk to feed straight into xterm. */
export interface DshLogEvent {
  instanceId: string;
  stream: "stdout" | "stderr";
  chunk: string;
}

/** A plugin/skill card in the Hub modal (mock catalog for the MVP). */
export interface McpPlugin {
  id: string;
  name: string;
  author: string;
  /** real npm package name; present ⇒ install/remove operate on dsh for real. */
  package?: string | null;
  description: string;
  /** lucide icon name for the row avatar. */
  icon: string;
  category: "modrinth" | "github" | "commercial";
  version: string;
  installed: boolean;
  links: { label: string; url: string }[];
}

// ---------------------------------------------------------------------------
// Launcher-level contract. Mirrors src-tauri/src/launcher_config.rs.
// Two backend-owned versioned files under ~/.agentlauncher/:
//   config.json      → LauncherConfig
//   instgroups.json  → InstGroups   (a presentation overlay, not source of truth)
// Secrets never live here — credentials stay in ~/.dsh/.credentials.yaml.
// ---------------------------------------------------------------------------

/** UI preferences (was localStorage: agentlauncher.{theme,locale}). */
export interface UiPrefs {
  theme: string;
  locale: string;
}

/** Launcher-wide agent defaults — prefill the New Instance dialog. Non-secret. */
export interface AgentDefaults {
  profile: string;
  provider: string;
  base_url: string;
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
