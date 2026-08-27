import { createApp } from "vue";
import App from "./App.vue";
import "@xterm/xterm/css/xterm.css";
import "./style.css";
import { initTheme } from "@/lib/theme";

initTheme();
createApp(App).mount("#app");
