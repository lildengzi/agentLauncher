// Theme engine: applies a theme's CSS variables to <html>.
//
// Source of truth is the launcher config (config.ui.theme, persisted to
// ~/.agentlauncher/config.json). A tiny localStorage cache (agentlauncher.theme)
// is kept ONLY for instant first paint before the async backend hydration lands
// — see bootstrapTheme() / main.ts.
import { computed, watch } from "vue";
import { themes, getTheme, DEFAULT_THEME } from "@/lib/themes";
import { config } from "@/lib/launcherConfig";

const FAST_CACHE = "agentlauncher.theme";

function apply(id: string): void {
  const theme = getTheme(id);
  const root = document.documentElement;
  // Clear first: `apply` writes inline properties, so a var a previous theme set
  // but this one omits would otherwise linger on <html> forever.
  for (const k of Object.keys(themes[0].vars)) {
    root.style.removeProperty(k);
  }
  for (const [k, v] of Object.entries(theme.vars)) {
    root.style.setProperty(k, v);
  }
  root.dataset.theme = theme.id;
  root.classList.toggle("light", !theme.dark);
}

// React to any change of the source-of-truth theme (user action or hydration):
// apply the CSS and refresh the first-paint cache.
watch(
  () => config.ui.theme,
  (id) => {
    apply(id);
    try {
      localStorage.setItem(FAST_CACHE, id);
    } catch {
      /* ignore */
    }
  }
);

export function setTheme(id: string): void {
  config.ui.theme = id; // persistence + apply happen via the watcher above
}

/** Instant first paint before backend hydration: apply the cached theme. */
export function bootstrapTheme(): void {
  const cached = localStorage.getItem(FAST_CACHE) ?? DEFAULT_THEME;
  apply(cached);
}

export function useTheme() {
  return { current: computed(() => config.ui.theme), themes, setTheme };
}
