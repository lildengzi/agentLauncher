// The launcher-level config store — the single source of truth for UI prefs,
// launcher-wide agent defaults, and session state. Backed by the backend file
// ~/.agentlauncher/config.json (mirrors src-tauri/src/launcher_config.rs).
//
// Design: a reactive `config` object is hydrated from the backend once at
// startup (initLauncherConfig). After hydration, any deep change is written back
// to disk (debounced). theme.ts / i18n.ts / settings.ts bind their editable
// surfaces to slices of this object rather than to localStorage.
//
// Secrets never enter this file — dsh's keys live in ~/.dsh/.credentials.yaml,
// every other engine's in the instance .env.
import { reactive, watch } from "vue";
import { api } from "@/lib/api";
import type { LauncherConfig } from "@/types";

export function defaultConfig(): LauncherConfig {
  return {
    format_version: 1,
    ui: { theme: "catppuccin-mocha", locale: "zh" },
    // Empty defaults = "let the chosen engine use its own"; see AgentDefaults in
    // src-tauri/src/launcher_config.rs. Must match the backend's defaults, since
    // this object is what a first run persists.
    defaults: { provider: "", model: "" },
    session: { selected_instance: "", last_used_group: "" },
  };
}

/** Reactive source of truth. Starts at defaults; hydrated from disk on init. */
export const config = reactive<LauncherConfig>(defaultConfig());

let hydrated = false;
let saveTimer: ReturnType<typeof setTimeout> | undefined;

function plain(): LauncherConfig {
  return JSON.parse(JSON.stringify(config));
}

function scheduleSave(): void {
  if (!hydrated) return; // never write back during hydration
  clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    api.setLauncherConfig(plain()).catch((e) => console.error("save launcher config failed", e));
  }, 300);
}

// Legacy localStorage keys, migrated once into config.json then removed.
const LS = { theme: "agentlauncher.theme", locale: "agentlauncher.locale", model: "agentlauncher.modelConfig" };
const MIGRATED = "agentlauncher.migrated";

/** One-time seed of config.json from the pre-backend localStorage state, so a
 *  returning user keeps their theme / language / model defaults. Idempotent via
 *  the MIGRATED flag; the theme/locale keys survive as first-paint caches. */
function migrateFromLocalStorage(): void {
  try {
    if (localStorage.getItem(MIGRATED)) return;
    const theme = localStorage.getItem(LS.theme);
    const locale = localStorage.getItem(LS.locale);
    if (theme) config.ui.theme = theme;
    if (locale === "zh" || locale === "en") config.ui.locale = locale;
    const raw = localStorage.getItem(LS.model);
    if (raw) {
      const m = JSON.parse(raw);
      if (m.provider) config.defaults.provider = m.provider;
      if (m.defaultModel) config.defaults.model = m.defaultModel;
    }
    localStorage.setItem(MIGRATED, "1");
    localStorage.removeItem(LS.model); // no longer read; theme/locale kept as cache
    api.setLauncherConfig(plain()).catch((e) => console.error("seed launcher config failed", e));
  } catch (e) {
    console.error("launcher config migration failed", e);
  }
}

/** Hydrate `config` from disk, run the one-time migration, and start persisting
 *  subsequent changes. Call once at startup before relying on config values. */
export async function initLauncherConfig(): Promise<void> {
  try {
    const remote = await api.getLauncherConfig();
    config.format_version = remote.format_version ?? 1;
    Object.assign(config.ui, remote.ui);
    Object.assign(config.defaults, remote.defaults);
    Object.assign(config.session, remote.session);
  } catch (e) {
    console.error("load launcher config failed; using defaults", e);
  }
  migrateFromLocalStorage();
  hydrated = true;
  watch(config, scheduleSave, { deep: true });
}
