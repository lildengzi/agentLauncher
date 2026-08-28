import { createApp } from "vue";
import App from "./App.vue";
import "@xterm/xterm/css/xterm.css";
import "./style.css";
import { bootstrapTheme } from "@/lib/theme";
import { initLauncherConfig } from "@/lib/launcherConfig";
import { initInstGroups } from "@/lib/instGroups";

// Instant first paint from the cached theme, then hydrate the launcher config
// and the sidebar grouping overlay from ~/.agentlauncher/ (the source of truth)
// before mounting so the reactive stores are populated when components read them.
bootstrapTheme();
Promise.all([initLauncherConfig(), initInstGroups()]).finally(() => {
  createApp(App).mount("#app");
});
