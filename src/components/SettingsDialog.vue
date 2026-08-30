<script setup lang="ts">
import { ref, watch } from "vue";
import { Palette, SlidersHorizontal, KeyRound, Database, Info, Check, Users, Server, Wrench, Network } from "lucide-vue-next";
import Dialog from "@/components/ui/Dialog.vue";
import Button from "@/components/ui/Button.vue";
import GroupBox from "@/components/ui/GroupBox.vue";
import SourcesSection from "@/components/settings/SourcesSection.vue";
import ProvidersSection from "@/components/settings/ProvidersSection.vue";
import { useTheme } from "@/lib/theme";
import { useI18n, type Locale } from "@/lib/i18n";

const open = defineModel<boolean>("open", { default: false });
/** Which page to show when the dialog opens. Set by whoever opens it — an editor
 *  window links straight to 模型与 API, because that page is the only editor of the
 *  app-level key store. An unknown name is ignored rather than blanking the body. */
const props = defineProps<{ page?: string }>();
const { t, locale, setLocale } = useI18n();
const { current, themes, setTheme } = useTheme();

/** The pages that exist. Named here rather than derived from `nav` below so the
 *  open-at-a-page watch cannot run before `nav` is initialised. */
const SECTIONS = ["appearance", "general", "model", "sources", "about"] as const;
type Section = (typeof SECTIONS)[number];
const section = ref<Section>("appearance");

function isSection(v: string | undefined): v is Section {
  return !!v && (SECTIONS as readonly string[]).includes(v);
}
// Jump on open, and also when the page changes while already open: a second click
// on 管理密钥… from an editor window must move the page, not do nothing.
watch(
  () => [open.value, props.page] as const,
  ([isOpen, page]) => {
    if (isOpen && isSection(page)) section.value = page;
  },
  { immediate: true }
);

/** One left-nav row. `id: null` marks a section that is spec'd but not built:
 *  it is listed, disabled, and badged 「规划中」 rather than hidden, so the
 *  sidebar shows the whole planned shape instead of implying these settings do
 *  not exist. A planned row has no page, which is why it is not a `Section`. */
type NavEntry = { id: Section | null; icon: any; key: string };

const nav: NavEntry[] = [
  { id: "appearance", icon: Palette, key: "settings.nav.appearance" },
  { id: "general", icon: SlidersHorizontal, key: "settings.nav.general" },
  { id: "model", icon: KeyRound, key: "settings.nav.model" },
  { id: "sources", icon: Database, key: "settings.nav.sources" },
  // Prism's Accounts / Services / Tools / Proxy pages, in its order. Still empty
  // here; each needs a data contract of its own (see docs/spec/step2.md 设置页).
  { id: null, icon: Users, key: "settings.nav.accounts" },
  { id: null, icon: Server, key: "settings.nav.remote" },
  { id: null, icon: Wrench, key: "settings.nav.tools" },
  { id: null, icon: Network, key: "settings.nav.proxy" },
  { id: "about", icon: Info, key: "settings.nav.about" },
];

function swatch(vars: Record<string, string>, key: string): string {
  return `hsl(${vars[key]})`;
}
</script>

<template>
  <Dialog v-model:open="open" width="max-w-3xl" class="h-[78vh]" :title="t('settings.title')">
    <div class="flex h-full min-h-0">
      <!-- left nav -->
      <nav class="w-44 shrink-0 border-r border-border bg-toolbar py-2">
        <button
          v-for="n in nav"
          :key="n.key"
          type="button"
          :disabled="!n.id"
          :title="n.id ? undefined : t('settings.plannedHint')"
          class="flex w-full items-center gap-2.5 px-3 py-2 text-left text-[14px] transition-colors"
          :class="
            !n.id
              ? 'cursor-not-allowed text-muted-foreground/60'
              : section === n.id
                ? 'bg-selection text-selection-foreground'
                : 'text-foreground/85 hover:bg-accent'
          "
          @click="n.id && (section = n.id)"
        >
          <component :is="n.icon" class="h-4 w-4 shrink-0" :stroke-width="1.75" />
          <span class="min-w-0 truncate">{{ t(n.key) }}</span>
          <span
            v-if="!n.id"
            class="ml-auto shrink-0 rounded-sm border border-border px-1 py-[1px] text-[11px] leading-none"
          >
            {{ t('settings.planned') }}
          </span>
        </button>
      </nav>

      <!-- content -->
      <div class="min-w-0 flex-1 overflow-y-auto px-5 py-4">
        <!-- APPEARANCE -->
        <template v-if="section === 'appearance'">
          <GroupBox :title="t('settings.theme')">
            <p class="mb-3 text-[13px] text-muted-foreground">{{ t('settings.theme.desc') }}</p>
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
                <span class="flex items-center justify-between text-[13px] text-foreground/90">
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
            <p class="mb-3 text-[13px] text-muted-foreground">{{ t('settings.language.desc') }}</p>
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

        <!-- MODEL & API — one box. The provider dropdown carries the launcher
             default, every provider's fields, the multi-key store, the local-runtime
             probe and dsh's credential line; the section owns its own load/save
             cycle, so this dialog holds no provider or key state at all. -->
        <template v-else-if="section === 'model'">
          <ProvidersSection />
        </template>

        <!-- MARKET SOURCES — the decentralized feed list; the section owns its
             own load/save cycle so this dialog holds no market state. -->
        <template v-else-if="section === 'sources'">
          <SourcesSection />
        </template>

        <!-- ABOUT -->
        <template v-else>
          <GroupBox title="agentLauncher">
            <p class="text-[14px] leading-relaxed text-foreground/85">{{ t('settings.about.desc') }}</p>
            <p class="mt-2 text-[13px] text-muted-foreground">Tauri 2 · Vue 3 · v0.1.0</p>
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
