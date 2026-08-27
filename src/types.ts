// ---------------------------------------------------------------------------
// Shared front/back contract. Mirrors the Rust structs in
// src-tauri/src/instance_manager.rs and the events emitted by dsh_runner.rs.
// Keep this file and the Rust side in sync.
// ---------------------------------------------------------------------------

/** An Agent instance — one isolated dsh profile + workspace. */
export interface Instance {
  id: string;
  name: string;
  /** lucide icon name, e.g. "code", "flask-conical", "globe". */
  icon: string;
  /** category shown as a collapsible group in the sidebar. */
  group: string;
  description: string;
  /** dsh profile to boot (default "headless"). */
  profile: string;
  /** underlying model target, e.g. "deepseek-reasoner". */
  model: string;
  temperature: number;
  thinking_budget: number;
  /** default task text prefilled when launching. */
  default_task: string;
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
  model: string;
  temperature: number;
  thinking_budget: number;
  default_task: string;
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
