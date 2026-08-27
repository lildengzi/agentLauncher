<script setup lang="ts">
import { reactive, ref, watch } from "vue";
import { Save, SlidersHorizontal, BrainCircuit, ListChecks } from "lucide-vue-next";
import Dialog from "@/components/ui/Dialog.vue";
import Button from "@/components/ui/Button.vue";
import Input from "@/components/ui/Input.vue";
import Textarea from "@/components/ui/Textarea.vue";
import Label from "@/components/ui/Label.vue";
import GroupBox from "@/components/ui/GroupBox.vue";
import AppIcon from "@/components/ui/AppIcon.vue";
import { api } from "@/lib/api";
import { useI18n } from "@/lib/i18n";
import type { Instance, NewInstance } from "@/types";

const { t } = useI18n();

const props = defineProps<{ instance: Instance | null }>();
const open = defineModel<boolean>("open", { default: false });
const emit = defineEmits<{ saved: [instance: Instance] }>();

type Section = "general" | "model" | "task";
const section = ref<Section>("general");

// Real dsh profiles discovered under ~/.dsh/profiles (fallback to the built-ins).
const profiles = ref<string[]>(["headless", "web"]);
watch(
  open,
  async (v) => {
    if (!v) return;
    try {
      const found = await api.listDshProfiles();
      if (found.length) profiles.value = found;
    } catch {
      /* keep fallback */
    }
  },
  { immediate: false }
);

interface FormState {
  name: string;
  icon: string;
  group: string;
  description: string;
  profile: string;
  model: string;
  temperature: number | string;
  thinking_budget: number | string;
  default_task: string;
}

function defaults(): FormState {
  return {
    name: "",
    icon: "bot",
    group: "未分类",
    description: "",
    profile: "headless",
    model: "deepseek-reasoner",
    temperature: 0.2,
    thinking_budget: 4096,
    default_task: "",
  };
}

const form = reactive<FormState>(defaults());
const nameError = ref("");
const errorBanner = ref("");
const saving = ref(false);

watch(
  () => [open.value, props.instance] as const,
  ([isOpen]) => {
    if (!isOpen) return;
    nameError.value = "";
    errorBanner.value = "";
    section.value = "general";
    if (props.instance) {
      form.name = props.instance.name;
      form.icon = props.instance.icon;
      form.group = props.instance.group;
      form.description = props.instance.description;
      form.profile = props.instance.profile;
      form.model = props.instance.model;
      form.temperature = props.instance.temperature;
      form.thinking_budget = props.instance.thinking_budget;
      form.default_task = props.instance.default_task;
    } else {
      Object.assign(form, defaults());
    }
  },
  { immediate: true }
);

async function save() {
  nameError.value = "";
  errorBanner.value = "";

  if (!form.name.trim()) {
    nameError.value = "名称不能为空";
    section.value = "general";
    return;
  }

  const payload: NewInstance = {
    name: form.name,
    icon: form.icon,
    group: form.group,
    description: form.description,
    profile: form.profile,
    model: form.model,
    temperature: Number(form.temperature),
    thinking_budget: Number(form.thinking_budget),
    default_task: form.default_task,
  };

  saving.value = true;
  try {
    let result: Instance;
    if (props.instance) {
      result = await api.updateInstance({
        ...props.instance,
        ...payload,
        id: props.instance.id,
        created_at: props.instance.created_at,
      });
    } else {
      result = await api.createInstance(payload);
    }
    emit("saved", result);
    open.value = false;
  } catch (err) {
    errorBanner.value = err instanceof Error ? err.message : String(err);
  } finally {
    saving.value = false;
  }
}

const navItems = [
  { key: "general" as const, icon: SlidersHorizontal, label: () => t("edit.nav.general") },
  { key: "model" as const, icon: BrainCircuit, label: () => t("edit.nav.model") },
  { key: "task" as const, icon: ListChecks, label: () => t("edit.nav.task") },
];

const rowGrid = "grid grid-cols-[120px_1fr] items-center gap-x-3 gap-y-3";
const selectClass =
  "h-8 rounded-sm border border-input bg-[hsl(var(--input))] px-2 text-[13px] focus:border-selection focus:outline-none";
</script>

<template>
  <Dialog
    v-model:open="open"
    width="max-w-2xl"
    class="h-[72vh]"
    :title="props.instance ? t('edit.title.edit') : t('edit.title.new')"
  >
    <div class="flex h-full min-h-0">
      <!-- Left nav -->
      <nav class="w-40 shrink-0 border-r border-border bg-toolbar py-2">
        <button
          v-for="item in navItems"
          :key="item.key"
          type="button"
          class="flex w-full items-center gap-2.5 px-3 py-2 text-left text-[13px] transition-colors"
          :class="
            section === item.key
              ? 'bg-selection text-selection-foreground'
              : 'text-foreground/85 hover:bg-accent'
          "
          @click="section = item.key"
        >
          <component :is="item.icon" class="h-4 w-4 shrink-0" :stroke-width="1.75" />
          <span>{{ item.label() }}</span>
        </button>
      </nav>

      <!-- Right content -->
      <div class="min-w-0 flex-1 overflow-y-auto px-5 py-4">
        <div
          v-if="errorBanner"
          class="mb-4 rounded border border-destructive/50 bg-destructive/10 px-3 py-2 text-[13px] text-destructive"
        >
          {{ errorBanner }}
        </div>

        <!-- General -->
        <GroupBox v-if="section === 'general'" :title="t('edit.nav.general')">
          <div :class="rowGrid">
            <Label for="inst-name">{{ t("edit.name") }}</Label>
            <div>
              <Input id="inst-name" v-model="form.name" />
              <p v-if="nameError" class="mt-1 text-[12px] text-destructive">
                {{ nameError }}
              </p>
            </div>

            <Label for="inst-icon">{{ t("edit.icon") }}</Label>
            <div>
              <div class="flex items-center gap-2">
                <span
                  class="flex h-8 w-8 shrink-0 items-center justify-center rounded-sm border border-border bg-muted text-foreground"
                >
                  <AppIcon :name="form.icon || 'bot'" class="h-4 w-4" />
                </span>
                <Input id="inst-icon" v-model="form.icon" class="flex-1" />
              </div>
              <p class="mt-1 text-[12px] text-muted-foreground">
                lucide 图标名，如 code / globe / flask-conical
              </p>
            </div>

            <Label for="inst-group">{{ t("edit.group") }}</Label>
            <Input id="inst-group" v-model="form.group" />

            <Label for="inst-desc" class="self-start pt-1.5">{{ t("edit.description") }}</Label>
            <Textarea id="inst-desc" v-model="form.description" class="min-h-[64px]" />
          </div>
        </GroupBox>

        <!-- Model -->
        <GroupBox v-if="section === 'model'" :title="t('edit.nav.model')">
          <div :class="rowGrid">
            <Label for="inst-profile">{{ t("edit.profile") }}</Label>
            <div>
              <select id="inst-profile" v-model="form.profile" :class="selectClass">
                <option v-for="p in profiles" :key="p" :value="p">{{ p }}</option>
              </select>
              <p class="mt-1 text-[12px] text-muted-foreground">
                headless = 单次任务，web = 交互式
              </p>
            </div>

            <Label for="inst-model">{{ t("edit.model") }}</Label>
            <Input id="inst-model" v-model="form.model" />

            <Label for="inst-temp">{{ t("edit.temperature") }}</Label>
            <Input
              id="inst-temp"
              v-model="form.temperature"
              type="number"
              step="0.1"
              min="0"
              max="2"
            />

            <Label for="inst-budget">{{ t("edit.thinking") }}</Label>
            <Input id="inst-budget" v-model="form.thinking_budget" type="number" />
          </div>
        </GroupBox>

        <!-- Task -->
        <GroupBox v-if="section === 'task'" :title="t('edit.nav.task')">
          <div class="grid gap-1.5">
            <Label for="inst-task">{{ t("edit.defaultTask") }}</Label>
            <Textarea id="inst-task" v-model="form.default_task" class="min-h-[140px]" />
            <p class="text-[12px] text-muted-foreground">
              留空则启动时使用通用默认任务
            </p>
          </div>
        </GroupBox>
      </div>
    </div>

    <template #footer>
      <Button variant="ghost" @click="open = false">{{ t("edit.cancel") }}</Button>
      <div class="flex-1" />
      <Button variant="primary" :disabled="saving" @click="save">
        <Save class="h-4 w-4" />
        {{ t("edit.save") }}
      </Button>
    </template>
  </Dialog>
</template>
