<script setup lang="ts">
import { ref, computed, watch } from "vue";
import { Palette, SlidersHorizontal, KeyRound, Info, Check } from "lucide-vue-next";
import Dialog from "@/components/ui/Dialog.vue";
import Button from "@/components/ui/Button.vue";
import Input from "@/components/ui/Input.vue";
import GroupBox from "@/components/ui/GroupBox.vue";
import { useTheme } from "@/lib/theme";
import { useI18n, type Locale } from "@/lib/i18n";
import { modelConfig, saveModelConfig, PROVIDERS } from "@/lib/settings";
import { api } from "@/lib/api";

const open = defineModel<boolean>("open", { default: false });
const { t, locale, setLocale } = useI18n();
const { current, themes, setTheme } = useTheme();

type Section = "appearance" | "general" | "model" | "about";
const section = ref<Section>("appearance");

const nav: { id: Section; icon: any; key: string }[] = [
  { id: "appearance", icon: Palette, key: "settings.nav.appearance" },
  { id: "general", icon: SlidersHorizontal, key: "settings.nav.general" },
  { id: "model", icon: KeyRound, key: "settings.nav.model" },
  { id: "about", icon: Info, key: "settings.nav.about" },
];

// Real dsh credential state: which key envs are already stored on disk.
const storedKeys = ref<string[]>([]);
const saveError = ref("");
const activeEnv = computed(
  () => PROVIDERS.find((p) => p.id === modelConfig.provider)?.apiKeyEnv ?? "DEEPSEEK_API_KEY"
);
const keyStored = computed(() => storedKeys.value.includes(activeEnv.value));

async function refreshKeys(): Promise<void> {
  try {
    storedKeys.value = await api.listCredentialKeys();
  } catch {
    storedKeys.value = [];
  }
}
watch(open, (v) => {
  if (v) refreshKeys();
});

const savedFlash = ref(false);
async function onSaveModel(): Promise<void> {
  saveError.value = "";
  saveModelConfig();
  const key = modelConfig.apiKey.trim();
  if (key) {
    try {
      await api.setCredential(activeEnv.value, key);
      modelConfig.apiKey = ""; // secret now lives in ~/.dsh/.credentials.yaml
      await refreshKeys();
    } catch (e) {
      saveError.value = String(e);
      return;
    }
  }
  savedFlash.value = true;
  setTimeout(() => (savedFlash.value = false), 1500);
}

function swatch(vars: Record<string, string>, key: string): string {
  return `hsl(${vars[key]})`;
}

function pickProvider(id: string): void {
  const p = PROVIDERS.find((x) => x.id === id);
  if (!p) return;
  modelConfig.provider = id;
  if (p.baseUrl) modelConfig.baseUrl = p.baseUrl;
  if (p.models.length && !p.models.includes(modelConfig.defaultModel)) {
    modelConfig.defaultModel = p.models[0];
  }
}
</script>

<!-- TEMPLATE_PLACEHOLDER -->
<template>
  <Dialog v-model:open="open" width="max-w-3xl" class="h-[78vh]" :title="t('settings.title')">
    <div class="flex h-full min-h-0">
      <!-- left nav -->
      <nav class="w-44 shrink-0 border-r border-border bg-toolbar py-2">
        <button
          v-for="n in nav"
          :key="n.id"
          type="button"
          class="flex w-full items-center gap-2.5 px-3 py-2 text-left text-[13px] transition-colors"
          :class="section === n.id ? 'bg-selection text-selection-foreground' : 'text-foreground/85 hover:bg-accent'"
          @click="section = n.id"
        >
          <component :is="n.icon" class="h-4 w-4" :stroke-width="1.75" />
          {{ t(n.key) }}
        </button>
      </nav>

      <!-- content -->
      <div class="min-w-0 flex-1 overflow-y-auto px-5 py-4">
        <!-- APPEARANCE -->
        <template v-if="section === 'appearance'">
          <GroupBox :title="t('settings.theme')">
            <p class="mb-3 text-[12px] text-muted-foreground">{{ t('settings.theme.desc') }}</p>
            <div class="grid grid-cols-3 gap-2">
              <button
                v-for="th in themes"
                :key="th.id"
                type="button"
                class="group relative flex flex-col gap-2 rounded border p-2 text-left transition-colors"
                :class="current === th.id ? 'border-selection ring-1 ring-selection' : 'border-border hover:border-border-strong'"
                @click="setTheme(th.id)"
              >
                <span class="flex h-9 overflow-hidden rounded-sm border border-border-strong">
                  <span class="flex-1" :style="{ background: swatch(th.vars, '--background') }" />
                  <span class="flex-1" :style="{ background: swatch(th.vars, '--panel') }" />
                  <span class="flex-1" :style="{ background: swatch(th.vars, '--selection') }" />
                  <span class="flex-1" :style="{ background: swatch(th.vars, '--link') }" />
                </span>
                <span class="flex items-center justify-between text-[12px] text-foreground/90">
                  {{ th.label }}
                  <Check v-if="current === th.id" class="h-3.5 w-3.5 text-selection" />
                </span>
              </button>
            </div>
          </GroupBox>
        </template>

        <!-- GENERAL -->
        <template v-else-if="section === 'general'">
          <GroupBox :title="t('settings.language')">
            <p class="mb-3 text-[12px] text-muted-foreground">{{ t('settings.language.desc') }}</p>
            <div class="flex gap-2">
              <Button
                v-for="l in (['zh','en'] as Locale[])"
                :key="l"
                :variant="locale === l ? 'primary' : 'outline'"
                @click="setLocale(l)"
              >
                {{ l === 'zh' ? t('settings.lang.zh') : t('settings.lang.en') }}
              </Button>
            </div>
          </GroupBox>
        </template>

        <!-- MODEL & API -->
        <template v-else-if="section === 'model'">
          <GroupBox :title="t('settings.model.title')">
            <p class="mb-3 text-[12px] text-muted-foreground">{{ t('settings.model.desc') }}</p>
            <div class="grid grid-cols-[120px_1fr] items-center gap-x-3 gap-y-3">
              <label class="text-[13px] text-foreground/85">{{ t('settings.model.provider') }}</label>
              <select
                :value="modelConfig.provider"
                class="h-8 rounded-sm border border-input bg-[hsl(var(--input))] px-2 text-[13px] focus:border-selection focus:outline-none"
                @change="pickProvider(($event.target as HTMLSelectElement).value)"
              >
                <option v-for="p in PROVIDERS" :key="p.id" :value="p.id">{{ p.label }}</option>
              </select>

              <label class="text-[13px] text-foreground/85">{{ t('settings.model.apiKey') }}</label>
              <div class="flex flex-col gap-1">
                <Input
                  v-model="modelConfig.apiKey"
                  type="password"
                  :placeholder="keyStored ? '•••••••• (已保存到 ~/.dsh)' : 'sk-...'"
                  class="font-mono"
                />
                <span class="inline-flex items-center gap-1 text-[11px]" :class="keyStored ? 'text-emerald-400' : 'text-muted-foreground'">
                  <template v-if="keyStored">
                    <Check class="h-3 w-3 shrink-0" />
                    {{ activeEnv }} 已写入 ~/.dsh/.credentials.yaml（输入新值可覆盖）
                  </template>
                  <template v-else>将写入 dsh 凭据文件 {{ activeEnv }}</template>
                </span>
              </div>

              <label class="text-[13px] text-foreground/85">{{ t('settings.model.baseUrl') }}</label>
              <Input v-model="modelConfig.baseUrl" placeholder="https://api.deepseek.com" />

              <label class="text-[13px] text-foreground/85">{{ t('settings.model.defaultModel') }}</label>
              <Input v-model="modelConfig.defaultModel" placeholder="deepseek-v4-flash" />
            </div>
            <div class="mt-4 flex items-center gap-3">
              <Button variant="primary" @click="onSaveModel">{{ t('settings.model.save') }}</Button>
              <span v-if="savedFlash" class="flex items-center gap-1 text-[12px] text-emerald-400">
                <Check class="h-3.5 w-3.5" /> {{ t('settings.model.saved') }}
              </span>
              <span v-if="saveError" class="text-[12px] text-destructive">{{ saveError }}</span>
            </div>
          </GroupBox>
        </template>

        <!-- ABOUT -->
        <template v-else>
          <GroupBox title="dsh Launcher">
            <p class="text-[13px] leading-relaxed text-foreground/85">{{ t('settings.about.desc') }}</p>
            <p class="mt-2 text-[12px] text-muted-foreground">Tauri 2 · Vue 3 · v0.1.0</p>
          </GroupBox>
        </template>
      </div>
    </div>

    <template #footer>
      <div class="flex-1" />
      <Button variant="outline" @click="open = false">{{ t('common.close') }}</Button>
    </template>
  </Dialog>
</template>
