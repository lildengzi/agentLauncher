// The sidebar grouping overlay store — a *presentation layer* over the instance
// list, backed by ~/.agentlauncher/instgroups.json (mirrors the Rust InstGroups).
//
// Membership is NOT owned here: each instance's own `group` field is the source
// of truth. This overlay only carries group display order, per-group collapsed
// state, and manual intra-group ordering. Robustness rule: it must never hide a
// real instance — stale ids in the overlay are ignored, and any instance/group
// missing from the overlay falls back to name ordering and is appended.
import { reactive, watch } from "vue";
import { api } from "@/lib/api";
import type { InstGroups, Instance } from "@/types";

export const instGroups = reactive<InstGroups>({ format_version: 1, order: [], groups: {} });

let hydrated = false;
let saveTimer: ReturnType<typeof setTimeout> | undefined;

function scheduleSave(): void {
  if (!hydrated) return;
  clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    const plain: InstGroups = JSON.parse(JSON.stringify(instGroups));
    api.setInstGroups(plain).catch((e) => console.error("save instgroups failed", e));
  }, 300);
}

/** Hydrate the overlay from disk, then persist subsequent changes (debounced). */
export async function initInstGroups(): Promise<void> {
  try {
    const r = await api.getInstGroups();
    instGroups.format_version = r.format_version ?? 1;
    instGroups.order = Array.isArray(r.order) ? [...r.order] : [];
    instGroups.groups = r.groups ? { ...r.groups } : {};
  } catch (e) {
    console.error("load instgroups failed; using defaults", e);
  }
  hydrated = true;
  watch(instGroups, scheduleSave, { deep: true });
}

export function isCollapsed(name: string): boolean {
  return instGroups.groups[name]?.collapsed ?? false;
}

export function toggleCollapsed(name: string): void {
  const g = instGroups.groups[name] ?? (instGroups.groups[name] = { collapsed: false, instances: [] });
  g.collapsed = !g.collapsed;
}

export interface OrderedGroup {
  name: string;
  items: Instance[];
}

/** Apply the overlay to the live instance list. Groups come out in the overlay's
 *  `order` (only those that actually have members), with any remaining groups
 *  appended by name. Within each group, instances follow the overlay's manual
 *  order, with unknown ids appended by name. Never drops a real instance. */
export function applyOverlay(instances: Instance[]): OrderedGroup[] {
  const byGroup = new Map<string, Instance[]>();
  for (const inst of instances) {
    const bucket = byGroup.get(inst.group);
    if (bucket) bucket.push(inst);
    else byGroup.set(inst.group, [inst]);
  }
  const known = instGroups.order.filter((n) => byGroup.has(n));
  const rest = [...byGroup.keys()]
    .filter((n) => !known.includes(n))
    .sort((a, b) => a.localeCompare(b));
  return [...known, ...rest].map((name) => {
    const items = byGroup.get(name)!;
    const overlay = instGroups.groups[name]?.instances ?? [];
    const pos = new Map(overlay.map((id, i) => [id, i] as const));
    const sorted = [...items].sort((a, b) => {
      const ai = pos.get(a.id) ?? Number.MAX_SAFE_INTEGER;
      const bi = pos.get(b.id) ?? Number.MAX_SAFE_INTEGER;
      return ai !== bi ? ai - bi : a.name.localeCompare(b.name);
    });
    return { name, items: sorted };
  });
}
