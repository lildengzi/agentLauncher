// Theme engine: applies a theme's CSS variables to <html> and persists choice.
import { ref, readonly } from "vue";
import { themes, getTheme, DEFAULT_THEME } from "@/lib/themes";

const STORAGE_KEY = "dsh-launcher.theme";
const current = ref<string>(localStorage.getItem(STORAGE_KEY) ?? DEFAULT_THEME);

function apply(id: string): void {
  const theme = getTheme(id);
  const root = document.documentElement;
  for (const [k, v] of Object.entries(theme.vars)) {
    root.style.setProperty(k, v);
  }
  root.dataset.theme = theme.id;
  root.classList.toggle("light", !theme.dark);
}

export function setTheme(id: string): void {
  current.value = id;
  localStorage.setItem(STORAGE_KEY, id);
  apply(id);
}

export function initTheme(): void {
  apply(current.value);
}

export function useTheme() {
  return { current: readonly(current), themes, setTheme };
}
