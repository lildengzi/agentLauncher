import { createApp } from "vue";
import EditWindow from "./EditWindow.vue";
import "./style.css";
import { bootstrapTheme } from "@/lib/theme";
import { initLauncherConfig } from "@/lib/launcherConfig";
import { bootStep, reveal } from "@/lib/boot";

// Entry point of a per-instance editor window. Same first-paint order as main.ts —
// cached theme immediately, then the launcher config before mounting — but with
// `persist: false`: this window reads the config and must never write it, because
// each webview holds its own reactive copy and two autosaving copies would take
// turns reverting each other. See initLauncherConfig.
//
// initInstGroups() is deliberately absent: the grouping overlay is the sidebar's
// presentation state, and nothing in the edit surface reads it.
bootstrapTheme();
bootStep("config");
initLauncherConfig({ persist: false }).finally(() => {
  bootStep("ui");
  try {
    createApp(EditWindow).mount("#app");
  } finally {
    reveal();
  }
});
