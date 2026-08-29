import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import { fileURLToPath, URL } from "node:url";

const host = process.env.TAURI_DEV_HOST;

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  plugins: [vue()],
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
  // Two entry points, two windows: index.html is the launcher, edit.html is one
  // per-instance editor window (opened by the open_edit_window command). Naming
  // index.html explicitly is required — adding a second input replaces Vite's
  // default single-entry behaviour rather than extending it.
  build: {
    rollupOptions: {
      input: {
        main: fileURLToPath(new URL("./index.html", import.meta.url)),
        edit: fileURLToPath(new URL("./edit.html", import.meta.url)),
      },
    },
  },
  // Tauri expects a fixed port, fail if that port is not available
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? { protocol: "ws", host, port: 1421 }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
}));
