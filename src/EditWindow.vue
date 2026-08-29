<script setup lang="ts">
import { onMounted, ref, watch } from "vue";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import EditInstanceDialog from "@/components/EditInstanceDialog.vue";
import { api } from "@/lib/api";
import type { Instance } from "@/types";

// Root of a standalone per-instance editor window. It is a shell, deliberately:
// every section, the market dialog and the MCP editor are the same components the
// modal uses, so there is one implementation of the edit surface and this file only
// answers "which instance, and when is the window done".
//
// The window's label *is* the instance id — `open_edit_window` builds `edit-<id>`
// (see src-tauri/src/lib.rs for why the character sets line up), so nothing has to
// be smuggled through the URL. Reading the label is synchronous and needs no
// permission: it comes from metadata injected into the page, not from IPC.
const win = getCurrentWebviewWindow();
const id = win.label.startsWith("edit-") ? win.label.slice("edit-".length) : "";

const instance = ref<Instance | null>(null);
const error = ref("");
const open = ref(true);

onMounted(async () => {
  if (!id) {
    error.value = `unexpected window label: ${win.label}`;
    return;
  }
  try {
    instance.value = await api.getInstance(id);
  } catch (e) {
    // The instance was deleted (or its instance.json is unreadable) between the
    // click and this read. Say so instead of showing an editor that cannot save.
    error.value = e instanceof Error ? e.message : String(e);
  }
});

// Cancel and Save both flip `open`, which in a window means "done" — the editor
// component does not know it is in a window, and does not need to.
watch(open, (v) => {
  if (!v) void win.close();
});
</script>

<template>
  <div
    v-if="error"
    class="flex h-screen items-center justify-center bg-background px-8 text-center text-[13px] text-destructive"
  >
    {{ error }}
  </div>
  <EditInstanceDialog
    v-else-if="instance"
    v-model:open="open"
    :instance="instance"
    inline
  />
  <div v-else class="h-screen bg-background" />
</template>
