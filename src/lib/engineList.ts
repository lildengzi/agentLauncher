// The six known engines in one place, plus the one way to ask which are installed.
//
// Two dialogs need this — 新建实例 picks an engine, 编辑实例 changes one — and a
// second copy of the table would drift from `engines.rs::known_engines` at half the
// speed of the first. Display strings mirror it exactly.
import { api } from "@/lib/api";
import type { EngineInfo, Instance } from "@/types";

/** Used when live detection fails, so the picker is never empty. `installed: true`
 *  across the board is deliberate: a failed probe must not lock a user out of an
 *  engine they have. A real probe result overrides every field here.
 *
 *  `install`/`package` are left as `manual`/`""` on purpose — offering a one-click
 *  install off a table that is only reached *because the probe failed* would fetch
 *  and run code on evidence we just admitted we do not have. The docs link is safe
 *  and stays. */
export const FALLBACK_ENGINES: EngineInfo[] = [
  { id: "dsh", display: "dsh (DeepSeek Harness)", web: true, takes_provider: true, installed: true, path: "", install: "manual", package: "", docs: "https://www.npmjs.com/package/@deepseek-ai/dsh", managed: false },
  { id: "pi", display: "pi (pi-coding-agent)", web: false, takes_provider: true, installed: true, path: "", install: "manual", package: "", docs: "https://github.com/earendil-works/pi", managed: false },
  { id: "omp", display: "omp (oh-my-pi)", web: false, takes_provider: true, installed: true, path: "", install: "manual", package: "", docs: "https://omp.sh/", managed: false },
  { id: "claude", display: "claude (Claude Code)", web: false, takes_provider: false, installed: true, path: "", install: "manual", package: "", docs: "https://github.com/anthropics/claude-code", managed: false },
  { id: "codex", display: "codex", web: false, takes_provider: true, installed: true, path: "", install: "manual", package: "", docs: "https://github.com/openai/codex", managed: false },
  { id: "opencode", display: "opencode", web: false, takes_provider: true, installed: true, path: "", install: "manual", package: "", docs: "https://github.com/anomalyco/opencode", managed: false },
];

/** Probe the host for installed engines, falling back to the table above. Never
 *  cached: an engine installed while the launcher is open should appear on the next
 *  refresh, and a stale "installed" is worse than a probe. */
export async function probeEngines(): Promise<EngineInfo[]> {
  try {
    const found = await api.detectEngines();
    return found.length ? found : FALLBACK_ENGINES;
  } catch {
    return FALLBACK_ENGINES;
  }
}

/** Whether launching this instance hands the session to the user's own terminal
 *  instead of piping it into the launcher's console.
 *
 *  The authority is `runtime::RunMode::resolve` in Rust; this is the one thing the
 *  frontend needs from it — whether to pop the console open — and it is deliberately
 *  the *cautious* half of that rule: an empty `mode` on dsh reads as "not
 *  interactive" without consulting the profile list, because dsh's own default is a
 *  task and its web profiles are servers. Both of those belong in the console. */
export function opensOwnTerminal(inst: Instance): boolean {
  const mode = (inst.runtime?.mode ?? "").trim();
  if (mode === "interactive") return true;
  if (mode === "task") return false;
  return (inst.runtime?.engine || "dsh") !== "dsh";
}
