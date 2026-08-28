<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { Instance, RunStatus } from "@/types";
import { api, onRuntimeStatus } from "@/lib/api";
import { useI18n } from "@/lib/i18n";
import { config } from "@/lib/launcherConfig";
import TopBar from "@/components/TopBar.vue";
import InstanceGrid from "@/components/InstanceGrid.vue";
import RightPanel from "@/components/RightPanel.vue";
import StatusBar from "@/components/StatusBar.vue";
import EditInstanceDialog from "@/components/EditInstanceDialog.vue";
import HubDialog from "@/components/HubDialog.vue";
import ConsoleDialog from "@/components/ConsoleDialog.vue";
import SettingsDialog from "@/components/SettingsDialog.vue";

const { t } = useI18n();

const instances = ref<Instance[]>([]);
const selectedId = ref<string | null>(null);
const statuses = ref<Record<string, RunStatus>>({});
const urls = ref<Record<string, string>>({});
const editOpen = ref(false);
const editingInstance = ref<Instance | null>(null);
const hubOpen = ref(false);
const consoleOpen = ref(false);
const settingsOpen = ref(false);

let unlistenStatus: (() => void) | null = null;

const selectedInstance = computed(
  () => instances.value.find((i) => i.id === selectedId.value) ?? null
);
const runningIds = computed(() =>
  Object.entries(statuses.value)
    .filter(([, s]) => s === "running" || s === "starting")
    .map(([id]) => id)
);
const selectedStatus = computed<RunStatus>(() =>
  selectedId.value ? statuses.value[selectedId.value] ?? "idle" : "idle"
);
const selectedUrl = computed<string | null>(() =>
  selectedId.value ? urls.value[selectedId.value] ?? null : null
);
const statusLabelMap: Record<RunStatus, string> = {
  idle: "status.idle",
  starting: "status.starting",
  running: "status.running",
  exited: "status.exited",
  error: "status.error",
};
const contextLine = computed(() => {
  if (!selectedInstance.value) return t("status.noSelection");
  return `${selectedInstance.value.name} · ${selectedInstance.value.model} · ${t(statusLabelMap[selectedStatus.value])}`;
});

async function loadInstances() {
  instances.value = await api.listInstances();
  if (!selectedId.value && instances.value.length) {
    // Restore the last selected instance from session state, if it still exists.
    const saved = config.session.selected_instance;
    const exists = saved && instances.value.some((i) => i.id === saved);
    selectedId.value = exists ? saved : instances.value[0].id;
  }
}

// Persist the selection so the next launch reopens on the same instance.
watch(selectedId, (id) => {
  config.session.selected_instance = id ?? "";
});

onMounted(async () => {
  await loadInstances();
  unlistenStatus = await onRuntimeStatus((e) => {
    statuses.value = { ...statuses.value, [e.instanceId]: e.status };
    if (e.url) {
      urls.value = { ...urls.value, [e.instanceId]: e.url };
    } else if (e.status === "exited" || e.status === "error") {
      const { [e.instanceId]: _drop, ...rest } = urls.value;
      urls.value = rest;
    }
  });
});
onBeforeUnmount(() => unlistenStatus?.());

function openCreate() {
  editingInstance.value = null;
  editOpen.value = true;
}
function openEdit() {
  if (selectedInstance.value) {
    editingInstance.value = selectedInstance.value;
    editOpen.value = true;
  }
}
async function onSaved(inst: Instance) {
  // Remember the group for prefilling the next New Instance dialog.
  config.session.last_used_group = inst.group;
  await loadInstances();
  selectedId.value = inst.id;
}

async function start(task?: string) {
  const id = selectedId.value;
  if (!id) return;
  const { [id]: _drop, ...rest } = urls.value;
  urls.value = rest;
  consoleOpen.value = true;
  statuses.value = { ...statuses.value, [id]: "starting" };
  try {
    await api.startInstance(id, task);
  } catch (e) {
    statuses.value = { ...statuses.value, [id]: "error" };
    console.error(e);
  }
}
async function openWeb() {
  if (selectedUrl.value) {
    await api.openUrl(selectedUrl.value).catch(console.error);
  }
}
function activate(id: string) {
  selectedId.value = id;
  start();
}
async function stop() {
  const id = selectedId.value;
  if (!id) return;
  await api.stopInstance(id).catch(console.error);
}
async function openFolder() {
  if (selectedId.value) {
    await api.openInstanceFolder(selectedId.value).catch(console.error);
  }
}
async function duplicate() {
  const s = selectedInstance.value;
  if (!s) return;
  const created = await api.createInstance({
    name: `${s.name} 副本`,
    icon: s.icon,
    group: s.group,
    description: s.description,
    profile: s.profile,
    provider: s.provider,
    model: s.model,
    runtime: { ...s.runtime },
    default_task: s.default_task,
  });
  await loadInstances();
  selectedId.value = created.id;
}
async function remove() {
  const s = selectedInstance.value;
  if (!s) return;
  if (!confirm(`确定删除实例「${s.name}」？该操作会删除其目录与所有数据。`)) return;
  await api.deleteInstance(s.id);
  selectedId.value = null;
  await loadInstances();
}
function exportProfile() {
  console.info("导出 Profile（MVP 占位）", selectedInstance.value?.id);
}
function noop() {}
</script>

<template>
  <div class="flex h-screen flex-col overflow-hidden">
    <TopBar
      @add="openCreate"
      @folder="openFolder"
      @settings="settingsOpen = true"
      @help="noop"
    />
    <div class="flex min-h-0 flex-1">
      <InstanceGrid
        :instances="instances"
        :selected-id="selectedId"
        :running-ids="runningIds"
        @select="(id: string) => (selectedId = id)"
        @activate="activate"
      />
      <RightPanel
        :instance="selectedInstance"
        :status="selectedStatus"
        @start="start()"
        @stop="stop"
        @edit="openEdit"
        @manage-mcp="hubOpen = true"
        @open-folder="openFolder"
        @duplicate="duplicate"
        @remove="remove"
        @export="exportProfile"
      />
    </div>
    <StatusBar
      app-version="v0.1.0"
      :running-count="runningIds.length"
      :context-line="contextLine"
    />

    <ConsoleDialog
      v-model:open="consoleOpen"
      :instance="selectedInstance"
      :status="selectedStatus"
      :url="selectedUrl"
      @stop="stop"
      @open-web="openWeb"
    />
    <EditInstanceDialog
      v-model:open="editOpen"
      :instance="editingInstance"
      @saved="onSaved"
    />
    <HubDialog v-model:open="hubOpen" :profile="selectedInstance?.profile" />
    <SettingsDialog v-model:open="settingsOpen" />
  </div>
</template>
