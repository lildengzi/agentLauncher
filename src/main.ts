import { createApp } from "vue";
import App from "./App.vue";
import "@xterm/xterm/css/xterm.css";
import "./style.css";
import { bootstrapTheme } from "@/lib/theme";
import { initLauncherConfig } from "@/lib/launcherConfig";
import { initInstGroups } from "@/lib/instGroups";
import { bootStep, reveal } from "@/lib/boot";

// Instant first paint from the cached theme, then hydrate the launcher config
// and the sidebar grouping overlay from ~/.agentlauncher/ (the source of truth)
// before mounting so the reactive stores are populated when components read them.
// The launch screen's bar is pushed along the way, and `reveal` cross-fades it out
// — in a `finally`, because a window that fails to render still has to stop
// showing a progress bar.
bootstrapTheme();
bootStep("config");
Promise.all([initLauncherConfig(), initInstGroups()]).finally(() => {
  bootStep("ui");
  try {
    createApp(App).mount("#app");
  } finally {
    reveal();
  }
});
