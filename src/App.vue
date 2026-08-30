<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { Instance, RunStatus } from "@/types";
import { api, onOpenSettings, onRuntimeStatus } from "@/lib/api";
import { useI18n } from "@/lib/i18n";
import { config } from "@/lib/launcherConfig";
import { opensOwnTerminal } from "@/lib/engineList";
import { instGroups } from "@/lib/instGroups";
import TopBar from "@/components/TopBar.vue";
import InstanceGrid from "@/components/InstanceGrid.vue";
import RightPanel from "@/components/RightPanel.vue";
import StatusBar from "@/components/StatusBar.vue";
import NewInstanceDialog from "@/components/NewInstanceDialog.vue";
import ChangeGroupDialog from "@/components/ChangeGroupDialog.vue";
import ConsoleDialog from "@/components/ConsoleDialog.vue";
import SettingsDialog from "@/components/SettingsDialog.vue";

const REPO_URL = "https://github.com/lildengzi/agentLauncher";

const { t } = useI18n();

const instances = ref<Instance[]>([]);
const selectedId = ref<string | null>(null);
const statuses = ref<Record<string, RunStatus>>({});
const urls = ref<Record<string, string>>({});
const createOpen = ref(false);
const groupOpen = ref(false);
const consoleOpen = ref(false);
const settingsOpen = ref(false);
/** Which settings page to open at; only set when something asks for a specific one
 *  (an editor window linking to the key store). Empty = leave the dialog's own. */
const settingsPage = ref("");

let unlistenStatus: (() => void) | null = null;
let unlistenSettings: (() => void) | null = null;

const dialogOpen = computed(
  // Only real modals belong here. The instance editor is its own window now, and a
  // window must not disable this window's mnemonics.
  () => createOpen.value || groupOpen.value || consoleOpen.value || settingsOpen.value
);

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
// Group names already in use, for the 改变分组 picker. Both sources matter: the
// instances themselves are the truth, and the overlay remembers a group whose last
// instance moved out, which is exactly the name a user is likely to move back to.
const groupNames = computed(() => {
  const seen = new Set<string>();
  for (const g of instGroups.order) if (g) seen.add(g);
  for (const i of instances.value) if (i.group) seen.add(i.group);
  return [...seen].sort((a, b) => a.localeCompare(b));
});
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
  instances.value = await api.listInstances().catch((e) => {
    console.error("list instances failed", e);
    return instances.value;
  });
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
  // Bind the mnemonics first so a slow backend can never strand the keyboard.
  window.addEventListener("keydown", onKey);
  // An editor window writes instance.json behind this window's back, so re-read the
  // list whenever focus comes back rather than plumbing an event for it. It also
  // picks up an instance.json edited outside the app, which an event would not.
  window.addEventListener("focus", onWindowFocus);
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
  // An editor window has no key store of its own — it asks this window to show one.
  unlistenSettings = await onOpenSettings((page) => {
    openSettings(page);
  });
});
onBeforeUnmount(() => {
  unlistenStatus?.();
  unlistenSettings?.();
  window.removeEventListener("keydown", onKey);
  window.removeEventListener("focus", onWindowFocus);
});

function onWindowFocus() {
  void loadInstances();
}

function openSettings(page = "") {
  // Always set the page, even to "": a stale value from an editor window's
  // 管理密钥… would otherwise make the toolbar's 设置 button open on 模型与 API.
  settingsPage.value = page;
  settingsOpen.value = true;
}
function openCreate() {
  // 新建是它自己的对话框，不再是编辑界面的 instance=null 分支：实例还不存在时，插件 /
  // 技能 / MCP / 人设 都没有目录可落，那四页只能显示「先保存」——点进去才发现是死的。
  // 新建只问能当场回答的问题（叫什么、哪个 Agent、什么模型），其余归编辑窗口。
  createOpen.value = true;
}
function openEdit() {
  if (selectedId.value) {
    api.openEditWindow(selectedId.value).catch((e) => console.error("open editor failed", e));
  }
}
function openChangeGroup() {
  if (selectedInstance.value) groupOpen.value = true;
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
  // 只在输出真的会落进控制台时才把它顶出来。交互式实例的对话在用户自己的终端里，
  // 控制台那边只有一行「用了哪个终端、脚本在哪」的记录——为一行字盖住列表，是把
  // 启动器摆在会话前面。真出错时状态会变红，控制台按钮还在原处。
  const inst = instances.value.find((i) => i.id === id);
  if (!inst || !opensOwnTerminal(inst)) consoleOpen.value = true;
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
    // A duplicate launches like its original, key binding included — the copy is
    // only useful if it can actually start.
    api_key_ref: s.api_key_ref,
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
function openRepo() {
  api.openUrl(REPO_URL).catch(console.error);
}

// The mnemonics printed on the buttons, bound the Qt way: Alt + letter, and only
// while no dialog is open. They used to be decoration only.
function onKey(e: KeyboardEvent) {
  if (!e.altKey || e.ctrlKey || e.metaKey || dialogOpen.value) return;
  const hit = (fn: () => void) => {
    e.preventDefault();
    fn();
  };
  switch (e.key.toLowerCase()) {
    case "e":
      return hit(openCreate);
    case "o":
      return hit(() => void openFolder());
    case "n":
      return hit(() => openSettings());
    case "l":
      return hit(() => void start());
    case "k":
      return hit(() => void stop());
    case "c":
      return hit(openChangeGroup);
    case "f":
      return hit(() => void openFolder());
    case "x":
      return hit(exportProfile);
    case "y":
      return hit(() => void duplicate());
    case "t":
      return hit(() => void remove());
  }
}
</script>

<template>
  <div class="flex h-screen flex-col overflow-hidden">
    <TopBar
      @add="openCreate"
      @folder="openFolder"
      @settings="openSettings()"
      @help="openRepo"
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
        @change-group="openChangeGroup"
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
      @more="openRepo"
    />

    <ConsoleDialog
      v-model:open="consoleOpen"
      :instance="selectedInstance"
      :status="selectedStatus"
      :url="selectedUrl"
      @stop="stop"
      @open-web="openWeb"
    />
    <NewInstanceDialog v-model:open="createOpen" :groups="groupNames" @saved="onSaved" />
    <ChangeGroupDialog
      v-model:open="groupOpen"
      :instance="selectedInstance"
      :groups="groupNames"
      @saved="onSaved"
    />
    <SettingsDialog v-model:open="settingsOpen" :page="settingsPage" />
  </div>
</template>
