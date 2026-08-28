<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { Save, SlidersHorizontal, BrainCircuit, ListChecks, Wrench } from "lucide-vue-next";
import Dialog from "@/components/ui/Dialog.vue";
import Button from "@/components/ui/Button.vue";
import Input from "@/components/ui/Input.vue";
import Textarea from "@/components/ui/Textarea.vue";
import Label from "@/components/ui/Label.vue";
import Select, { type SelectOption } from "@/components/ui/Select.vue";
import GroupBox from "@/components/ui/GroupBox.vue";
import AppIcon from "@/components/ui/AppIcon.vue";
import { api } from "@/lib/api";
import { useI18n } from "@/lib/i18n";
import { config } from "@/lib/launcherConfig";
import type { DshProfile, EngineInfo, Instance, NewInstance } from "@/types";

const { t } = useI18n();

const props = defineProps<{ instance: Instance | null }>();
const open = defineModel<boolean>("open", { default: false });
const emit = defineEmits<{ saved: [instance: Instance] }>();

type Section = "general" | "model" | "runtime" | "task";
const section = ref<Section>("general");

// Known engines, used as a fallback when live detection fails so the picker is
// always usable. Display strings mirror engines.rs::known_engines.
const FALLBACK_ENGINES: EngineInfo[] = [
  { id: "dsh", display: "dsh (DeepSeek Harness)", web: true, takes_provider: true, installed: true, path: "" },
  { id: "pi", display: "pi (pi-coding-agent)", web: false, takes_provider: true, installed: true, path: "" },
  { id: "omp", display: "omp (oh-my-pi)", web: false, takes_provider: true, installed: true, path: "" },
  { id: "claude", display: "claude (Claude Code)", web: false, takes_provider: false, installed: true, path: "" },
  { id: "codex", display: "codex", web: false, takes_provider: true, installed: true, path: "" },
  { id: "opencode", display: "opencode", web: false, takes_provider: true, installed: true, path: "" },
];
const engines = ref<EngineInfo[]>(FALLBACK_ENGINES);

// Real dsh profiles discovered under ~/.dsh/profiles (fallback to the built-ins).
// `web` comes from the backend, which reads the profile's bundled packages — the
// name alone does not decide it.
const profiles = ref<DshProfile[]>([
  { name: "headless", web: false },
  { name: "web", web: true },
]);
watch(
  open,
  async (v) => {
    if (!v) return;
    // Probe installed engines fresh each time the dialog opens (never cached).
    api
      .detectEngines()
      .then((found) => {
        if (found.length) engines.value = found;
      })
      .catch(() => {
        /* keep fallback */
      });
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
  engine: string;
  profile: string;
  provider: string;
  model: string;
  env_policy: string;
  custom_bin: string;
  default_task: string;
}

function defaults(): FormState {
  // Prefill a new instance from the launcher-wide defaults + last used group.
  return {
    name: "",
    icon: "bot",
    group: config.session.last_used_group || "未分类",
    description: "",
    engine: "dsh",
    profile: "headless",
    provider: config.defaults.provider || "",
    // Empty = the selected framework's own default (see "空值即省略" contract).
    // The launcher-wide default is owned by config.defaults, not hardcoded here.
    model: config.defaults.model || "",
    env_policy: "autodetect",
    custom_bin: "",
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
      form.engine = props.instance.runtime?.engine || "dsh";
      form.profile = props.instance.profile;
      form.provider = props.instance.provider || "";
      form.model = props.instance.model;
      form.env_policy = props.instance.runtime?.env_policy || "autodetect";
      form.custom_bin = props.instance.runtime?.custom_bin || "";
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
    provider: form.provider.trim(),
    model: form.model,
    runtime: {
      engine: form.engine,
      env_policy: form.env_policy,
      custom_bin: form.custom_bin.trim(),
    },
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
  { key: "runtime" as const, icon: Wrench, label: () => t("edit.nav.runtime") },
  { key: "task" as const, icon: ListChecks, label: () => t("edit.nav.task") },
];

// The dsh `profile` concept only applies to the dsh engine; other engines hide it.
const isDsh = computed(() => form.engine === "dsh");
// Live install status of the currently selected engine, for a hint line.
const selectedEngine = computed(
  () => engines.value.find((e) => e.id === form.engine) ?? null
);
// Whether the selected engine actually receives a provider on the command line.
// claude does not (ANTHROPIC_* env only), so the field is hidden rather than
// silently dropped. Unknown engines default to showing it.
const takesProvider = computed(() => selectedEngine.value?.takes_provider ?? true);

// How this instance will run, stated the same way for every engine instead of
// leaving it implied by a dsh-only field: interactive only when the engine has a
// web mode *and* the chosen profile bundles it (the same judgement the backend's
// `is_serve` makes). Every other combination is a one-shot task.
const runsWeb = computed(() => {
  if (!selectedEngine.value?.web) return false;
  return profiles.value.find((p) => p.name === form.profile)?.web ?? false;
});

// Labels sit on the first line of their cell: rows whose field carries a hint are
// taller than the label, and centering made the label drift down beside the hint.
const rowGrid =
  "grid grid-cols-[120px_1fr] items-start gap-x-3 gap-y-3 [&>label]:pt-2";

// Dropdown option lists. Engine rows carry an install-status hint so a missing
// CLI is visible in the closed trigger too, not just in the open list.
const engineOptions = computed<SelectOption[]>(() =>
  engines.value.map((e) => ({
    value: e.id,
    label: e.display,
    hint: e.installed ? undefined : t("edit.runtime.engineMissing"),
    warn: !e.installed,
  }))
);
const profileOptions = computed<SelectOption[]>(() =>
  profiles.value.map((p) => ({
    value: p.name,
    label: p.name,
    hint: p.web ? t("edit.runtime.shape.web") : t("edit.runtime.shape.headless"),
  }))
);
const envPolicyOptions = computed<SelectOption[]>(() => [
  { value: "autodetect", label: t("edit.runtime.autodetect") },
  { value: "isolated", label: t("edit.runtime.isolated") },
]);
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
          class="flex w-full items-center gap-2.5 px-3 py-2 text-left text-[14px] transition-colors"
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
          class="mb-4 rounded border border-destructive/50 bg-destructive/10 px-3 py-2 text-[14px] text-destructive"
        >
          {{ errorBanner }}
        </div>

        <!-- General -->
        <GroupBox v-if="section === 'general'" :title="t('edit.nav.general')">
          <div :class="rowGrid">
            <Label for="inst-name">{{ t("edit.name") }}</Label>
            <div>
              <Input id="inst-name" v-model="form.name" />
              <p v-if="nameError" class="mt-1 text-[13px] text-destructive">
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
              <p class="mt-1 text-[13px] text-muted-foreground">
                {{ t("edit.iconHint") }}
              </p>
            </div>

            <Label for="inst-group">{{ t("edit.group") }}</Label>
            <Input id="inst-group" v-model="form.group" />

            <Label for="inst-desc" class="self-start">{{ t("edit.description") }}</Label>
            <Textarea id="inst-desc" v-model="form.description" class="min-h-[64px]" />
          </div>
        </GroupBox>

        <!-- Model — LLM identity only; the framework/host knobs live under Runtime. -->
        <GroupBox v-if="section === 'model'" :title="t('edit.nav.model')">
          <div :class="rowGrid">
            <template v-if="takesProvider">
              <Label for="inst-provider">{{ t("edit.model.provider") }}</Label>
              <div>
                <Input id="inst-provider" v-model="form.provider" />
                <p class="mt-1 text-[13px] text-muted-foreground">
                  {{ t("edit.model.providerHint") }}
                </p>
              </div>
            </template>
            <template v-else>
              <Label>{{ t("edit.model.provider") }}</Label>
              <p class="pt-2 text-[13px] text-muted-foreground">
                {{ t("edit.model.providerEnvOnly") }}
              </p>
            </template>

            <Label for="inst-model">{{ t("edit.model") }}</Label>
            <Input id="inst-model" v-model="form.model" />
          </div>
        </GroupBox>

        <!-- Runtime / environment override -->
        <GroupBox v-if="section === 'runtime'" :title="t('edit.nav.runtime')">
          <div :class="rowGrid">
            <Label for="inst-engine">{{ t("edit.runtime.engine") }}</Label>
            <div>
              <Select id="inst-engine" v-model="form.engine" :options="engineOptions" />
              <p class="mt-1 text-[13px] text-muted-foreground">
                {{ t("edit.runtime.engineHint") }}
                <template v-if="selectedEngine && !selectedEngine.installed">
                  · <span class="text-destructive">{{ t("edit.runtime.engineMissing") }}</span>
                </template>
                <template v-else-if="selectedEngine && selectedEngine.path">
                  · <span class="font-mono">{{ selectedEngine.path }}</span>
                </template>
              </p>
            </div>

            <!-- Run shape, stated for every engine — otherwise it is only implied
                 by a field that exists for dsh alone. -->
            <Label>{{ t("edit.runtime.shape") }}</Label>
            <div class="pt-2">
              <span class="text-[14px]">
                {{ runsWeb ? t("edit.runtime.shape.web") : t("edit.runtime.shape.headless") }}
              </span>
              <p class="mt-1 text-[13px] text-muted-foreground">
                {{
                  selectedEngine && selectedEngine.web
                    ? t("edit.runtime.shapeHintProfile")
                    : t("edit.runtime.shapeHintNoWeb")
                }}
              </p>
            </div>

            <Label for="inst-envpolicy">{{ t("edit.runtime.envPolicy") }}</Label>
            <div>
              <Select
                id="inst-envpolicy"
                v-model="form.env_policy"
                :options="envPolicyOptions"
              />
              <p class="mt-1 text-[13px] text-muted-foreground">
                {{ t("edit.runtime.envPolicyHint") }}
              </p>
            </div>

            <Label for="inst-custombin" class="self-start">
              {{ t("edit.runtime.customBin") }}
            </Label>
            <div>
              <Input
                id="inst-custombin"
                v-model="form.custom_bin"
                :placeholder="`/usr/local/bin/${form.engine || 'dsh'}`"
              />
              <p class="mt-1 text-[13px] text-muted-foreground">
                {{ t("edit.runtime.customBinHint") }}
              </p>
            </div>
          </div>

          <!-- Knobs that exist only for the selected framework. Fenced off and
               named after it, so a vendor-specific field never reads as a
               launcher-wide one — dsh's `profile` is the only such knob today. -->
          <div v-if="isDsh" class="mt-4 border-t border-border pt-4">
            <p class="mb-3 text-[13px] text-muted-foreground">
              {{ t("edit.runtime.engineSpecific") }} ·
              <span class="font-mono">{{ selectedEngine?.display || form.engine }}</span>
            </p>
            <div :class="rowGrid">
              <Label for="inst-profile">{{ t("edit.profile") }}</Label>
              <div>
                <Select id="inst-profile" v-model="form.profile" :options="profileOptions" />
                <p class="mt-1 text-[13px] text-muted-foreground">
                  {{ t("edit.profileHint") }}
                </p>
              </div>
            </div>
          </div>
        </GroupBox>

        <!-- Task -->
        <GroupBox v-if="section === 'task'" :title="t('edit.nav.task')">
          <div class="grid gap-1.5">
            <Label for="inst-task">{{ t("edit.defaultTask") }}</Label>
            <Textarea id="inst-task" v-model="form.default_task" class="min-h-[140px]" />
            <p class="text-[13px] text-muted-foreground">
              {{ t("edit.defaultTaskHint") }}
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
