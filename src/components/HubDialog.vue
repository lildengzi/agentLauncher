<script setup lang="ts">
import { computed, ref, watch } from "vue";
import {
  Sparkles,
  LayoutGrid,
  Puzzle,
  GraduationCap,
  Star,
  GitFork,
  Copy,
  Check,
} from "lucide-vue-next";
import Dialog from "@/components/ui/Dialog.vue";
import Button from "@/components/ui/Button.vue";
import Input from "@/components/ui/Input.vue";
import Badge from "@/components/ui/Badge.vue";
import Avatar from "@/components/ui/Avatar.vue";
import Select, { type SelectOption } from "@/components/ui/Select.vue";
import { api } from "@/lib/api";
import { brandForProvider, type Brand } from "@/lib/brand";
import { useI18n } from "@/lib/i18n";
import { getMarketData } from "@/lib/market/data";
import { search } from "@/lib/market/search";
import { recommend } from "@/lib/market/recommend";
import { hotTags } from "@/lib/market/tags";
import {
  installSpec,
  skillInstallCommand,
  isInstallable,
  isInstalled as isInstalledPkg,
  matchedDep,
} from "@/lib/market/install";
import type { MarketPlugin, PluginType } from "@/lib/market/types";
import type { FunctionalComponent } from "vue";

const { t } = useI18n();

const open = defineModel<boolean>("open", { default: false });
const props = defineProps<{ profile?: string }>();
const profile = computed(() => props.profile || "headless");

type View = "recommend" | "all" | "cordis" | "skill";

const plugins = ref<MarketPlugin[]>([]);
const installedPkgs = ref<string[]>([]);
const loading = ref(false);
const loadError = ref("");
const stale = ref(false);
const busy = ref(false);
const opError = ref("");
const query = ref("");
const debouncedQuery = ref("");
const view = ref<View>("recommend");
const sortBy = ref<"relevance" | "score" | "newest">("relevance");
const sortOptions = computed<SelectOption[]>(() => [
  { value: "relevance", label: t("hub.sort.relevance") },
  { value: "score", label: t("hub.sort.score") },
  { value: "newest", label: t("hub.sort.newest") },
]);
function setSort(v: string): void {
  sortBy.value = v as typeof sortBy.value;
}
const noConfigOnly = ref(false);
const selectedTags = ref<string[]>([]);
const selectedId = ref<string | null>(null);
const copied = ref(false);

const views = computed<{ key: View; label: string; icon: FunctionalComponent }[]>(
  () => [
    { key: "recommend", label: t("hub.view.recommend"), icon: Sparkles },
    { key: "all", label: t("hub.view.all"), icon: LayoutGrid },
    { key: "cordis", label: t("hub.view.cordis"), icon: Puzzle },
    { key: "skill", label: t("hub.view.skill"), icon: GraduationCap },
  ]
);

let debounceTimer: ReturnType<typeof setTimeout> | undefined;
watch(query, (q) => {
  clearTimeout(debounceTimer);
  debounceTimer = setTimeout(() => (debouncedQuery.value = q), 200);
});

async function refreshInstalled(): Promise<void> {
  try {
    installedPkgs.value = await api.listInstalledPlugins(profile.value);
  } catch {
    installedPkgs.value = [];
  }
}

async function load(): Promise<void> {
  loading.value = true;
  loadError.value = "";
  try {
    const res = await getMarketData();
    plugins.value = res.data.plugins;
    stale.value = res.stale;
    await refreshInstalled();
  } catch (e) {
    loadError.value = String(e);
    plugins.value = [];
  } finally {
    loading.value = false;
  }
}

watch(open, (isOpen) => {
  if (!isOpen) return;
  opError.value = "";
  if (plugins.value.length === 0) load();
  else refreshInstalled();
});

// Top usable tags as quick AND-filter chips.
const tagChips = computed(() => hotTags(plugins.value, 14).map((tc) => tc.tag));

function toggleTag(tag: string): void {
  const next = new Set(selectedTags.value);
  next.has(tag) ? next.delete(tag) : next.add(tag);
  selectedTags.value = [...next];
}

interface Row {
  plugin: MarketPlugin;
  relevance: number;
  reason: string;
}

const results = computed<Row[]>(() => {
  const q = debouncedQuery.value.trim();
  const tags = selectedTags.value;
  const typeFilter: PluginType | undefined =
    view.value === "cordis"
      ? "cordis-plugin"
      : view.value === "skill"
        ? "skill"
        : undefined;

  // Recommend surface only when nothing is being actively filtered.
  if (view.value === "recommend" && !q && tags.length === 0) {
    return recommend(plugins.value, null, { limit: 40 }).map((r) => ({
      plugin: r.plugin,
      relevance: 0,
      reason: r.reasons[0] ?? "",
    }));
  }

  return search(plugins.value, q, {
    type: typeFilter,
    tags,
    sortBy: sortBy.value,
    noConfigOnly: noConfigOnly.value,
    limit: 200,
  }).map((r) => ({ plugin: r.plugin, relevance: r.relevance, reason: "" }));
});

const selected = computed<MarketPlugin | null>(() => {
  const list = results.value;
  if (list.length === 0) return null;
  return (
    list.find((r) => r.plugin.id === selectedId.value)?.plugin ??
    list[0].plugin
  );
});

watch(results, (list) => {
  if (!list.some((r) => r.plugin.id === selectedId.value)) {
    selectedId.value = list[0]?.plugin.id ?? null;
  }
  copied.value = false;
});

function isInstalled(p: MarketPlugin): boolean {
  return isInstalledPkg(p, installedPkgs.value);
}

/** DeepSeek-authored plugins carry the official whale mark. */
function pluginBrand(p: MarketPlugin): Brand | null {
  return /deepseek/i.test(p.owner) || /deepseek/i.test(p.name)
    ? brandForProvider("deepseek")
    : null;
}

/** Five-dimension score as labelled 0-100 bars. */
const scoreBars = computed(() => {
  const p = selected.value;
  if (!p) return [];
  const b = p.score.breakdown;
  return [
    { key: t("hub.score.maintain"), v: b.maintain },
    { key: t("hub.score.practical"), v: b.practical },
    { key: t("hub.score.popularity"), v: b.popularity },
    { key: t("hub.score.ease"), v: b.ease },
    { key: t("hub.score.signal"), v: b.signal },
  ];
});

async function install(p: MarketPlugin): Promise<void> {
  if (!isInstallable(p) || busy.value) return;
  busy.value = true;
  opError.value = "";
  try {
    await api.pluginAdd(profile.value, installSpec(p));
    await refreshInstalled();
  } catch (e) {
    opError.value = String(e);
  } finally {
    busy.value = false;
  }
}

async function uninstall(p: MarketPlugin): Promise<void> {
  if (busy.value) return;
  const dep = matchedDep(p, installedPkgs.value) ?? installSpec(p);
  busy.value = true;
  opError.value = "";
  try {
    await api.pluginRemove(profile.value, dep);
    await refreshInstalled();
  } catch (e) {
    opError.value = String(e);
  } finally {
    busy.value = false;
  }
}

async function copyCommand(p: MarketPlugin): Promise<void> {
  try {
    await navigator.clipboard.writeText(skillInstallCommand(p));
    copied.value = true;
    setTimeout(() => (copied.value = false), 1500);
  } catch {
    /* clipboard unavailable */
  }
}
</script>

<template>
  <Dialog
    v-model:open="open"
    width="max-w-5xl"
    class="h-[82vh]"
    :title="t('hub.title')"
  >
    <div class="flex h-full min-h-0">
      <!-- LEFT: views + hot-tag quick filters -->
      <nav class="flex w-48 shrink-0 flex-col overflow-y-auto border-r border-border bg-toolbar">
        <div class="py-2">
          <button
            v-for="v in views"
            :key="v.key"
            class="flex w-full items-center gap-2.5 px-3 py-2 text-left text-[14px] transition-colors"
            :class="
              view === v.key
                ? 'bg-selection text-selection-foreground'
                : 'text-foreground/85 hover:bg-accent'
            "
            @click="view = v.key"
          >
            <component :is="v.icon" class="h-4 w-4 shrink-0" :stroke-width="1.75" />
            <span class="truncate">{{ v.label }}</span>
          </button>
        </div>
        <div v-if="tagChips.length" class="border-t border-border px-3 py-2">
          <p class="mb-1.5 text-[12px] font-medium text-muted-foreground">
            {{ t('hub.hotTags') }}
          </p>
          <div class="flex flex-wrap gap-1">
            <button
              v-for="tag in tagChips"
              :key="tag"
              class="rounded px-1.5 py-0.5 text-[12px] transition-colors"
              :class="
                selectedTags.includes(tag)
                  ? 'bg-selection text-selection-foreground'
                  : 'bg-accent/60 text-foreground/80 hover:bg-accent'
              "
              @click="toggleTag(tag)"
            >
              {{ tag }}
            </button>
          </div>
        </div>
      </nav>
<!-- @@MIDDLE@@ -->
      <!-- MIDDLE: filter bar + scrollable list -->
      <div class="flex min-w-0 flex-1 flex-col border-r border-border">
        <div class="flex items-center gap-2 border-b border-border px-3 py-2">
          <Input v-model="query" :placeholder="t('hub.search')" class="flex-1" />
          <Select
            :model-value="sortBy"
            :options="sortOptions"
            class="w-auto shrink-0"
            @update:model-value="setSort"
          />
          <label class="flex shrink-0 items-center gap-1 text-[13px] text-muted-foreground">
            <input v-model="noConfigOnly" type="checkbox" class="accent-primary" />
            {{ t('hub.noConfig') }}
          </label>
        </div>

        <div class="flex-1 overflow-y-auto">
          <p v-if="loading" class="py-10 text-center text-[14px] text-muted-foreground">
            {{ t('hub.loading') }}
          </p>
          <div v-else-if="loadError" class="flex flex-col items-center gap-2 py-10 text-center">
            <p class="px-4 text-[14px] text-destructive">{{ t('hub.loadError') }}</p>
            <p class="px-6 text-[12px] text-muted-foreground">{{ loadError }}</p>
            <Button variant="outline" size="sm" @click="load()">{{ t('hub.retry') }}</Button>
          </div>
          <p v-else-if="results.length === 0" class="py-10 text-center text-[14px] text-muted-foreground">
            {{ t('hub.empty') }}
          </p>
          <template v-else>
            <p v-if="stale" class="border-b border-border/60 bg-accent/40 px-3 py-1 text-[12px] text-muted-foreground">
              {{ t('hub.staleNotice') }}
            </p>
            <div
              v-for="row in results"
              :key="row.plugin.id"
              class="flex cursor-pointer items-start gap-3 border-b border-border/60 px-3 py-2"
              :class="
                selected && selected.id === row.plugin.id
                  ? 'bg-selection text-selection-foreground'
                  : 'hover:bg-accent'
              "
              @click="selectedId = row.plugin.id"
            >
              <Avatar :seed="row.plugin.id" :brand="pluginBrand(row.plugin)" :size="36" />
              <div class="min-w-0 flex-1">
                <div class="flex items-center gap-2">
                  <span class="truncate text-[14px] font-semibold">{{ row.plugin.name }}</span>
                  <Badge v-if="isInstalled(row.plugin)" variant="default">{{ t('hub.installed') }}</Badge>
                  <span class="ml-auto shrink-0 text-[12px] tabular-nums text-muted-foreground">
                    ★ {{ row.plugin.stars }}
                  </span>
                </div>
                <p
                  class="truncate text-[13px]"
                  :class="
                    selected && selected.id === row.plugin.id
                      ? 'text-selection-foreground/80'
                      : 'text-muted-foreground'
                  "
                >
                  {{ row.reason || row.plugin.descriptionZh || row.plugin.description }}
                </p>
              </div>
            </div>
          </template>
        </div>
      </div>
<!-- @@DETAIL@@ -->
      <!-- RIGHT: detail pane -->
      <div class="w-80 shrink-0 overflow-y-auto bg-panel p-4">
        <p v-if="!selected" class="py-10 text-center text-[14px] text-muted-foreground">
          {{ t('right.selectHint') }}
        </p>
        <div v-else class="flex flex-col gap-3">
          <div class="flex items-center gap-3">
            <Avatar :seed="selected.id" :brand="pluginBrand(selected)" :size="44" />
            <div class="min-w-0">
              <h3 class="truncate text-[16px] font-semibold text-foreground">{{ selected.name }}</h3>
              <p class="truncate text-[13px] text-muted-foreground">{{ selected.owner }}</p>
            </div>
          </div>

          <div class="flex flex-wrap items-center gap-1.5 text-[12px] text-muted-foreground">
            <Badge variant="outline">{{ selected.type === 'skill' ? t('hub.type.skill') : t('hub.type.cordis') }}</Badge>
            <span class="inline-flex items-center gap-1"><Star class="h-3 w-3" /> {{ selected.stars }}</span>
            <span class="inline-flex items-center gap-1"><GitFork class="h-3 w-3" /> {{ selected.forks }}</span>
            <span v-if="selected.install.needsConfig" class="text-amber-500">{{ t('hub.needsConfig') }}</span>
          </div>

          <div v-if="selected.tags.length" class="flex flex-wrap gap-1">
            <span v-for="tag in selected.tags" :key="tag" class="rounded bg-accent/60 px-1.5 py-0.5 text-[12px] text-foreground/80">
              {{ tag }}
            </span>
          </div>

          <p class="text-[14px] leading-relaxed text-foreground/90">
            {{ selected.descriptionZh || selected.description || t('hub.noDescription') }}
          </p>

          <!-- five-dimension practical score -->
          <div class="flex flex-col gap-1.5 rounded border border-border/60 bg-background/40 p-2.5">
            <div class="flex items-center justify-between text-[13px]">
              <span class="font-medium text-foreground">{{ t('hub.score.title') }}</span>
              <span class="tabular-nums text-foreground">{{ selected.score.total }}<span class="text-muted-foreground">/100</span></span>
            </div>
            <div v-for="bar in scoreBars" :key="bar.key" class="flex items-center gap-2">
              <span class="w-12 shrink-0 text-[11px] text-muted-foreground">{{ bar.key }}</span>
              <span class="h-1.5 flex-1 overflow-hidden rounded-full bg-accent">
                <span class="block h-full rounded-full bg-primary" :style="{ width: bar.v + '%' }" />
              </span>
              <span class="w-6 shrink-0 text-right text-[11px] tabular-nums text-muted-foreground">{{ bar.v }}</span>
            </div>
            <p v-if="selected.score.explanation" class="mt-0.5 text-[12px] leading-relaxed text-muted-foreground">
              {{ selected.score.explanation }}
            </p>
          </div>

          <div class="flex flex-col gap-1 text-[14px]">
            <a :href="'https://github.com/' + selected.fullName" target="_blank" rel="noopener noreferrer" class="text-link hover:underline">
              {{ t('hub.repo') }}
            </a>
            <a v-if="selected.homepage" :href="selected.homepage" target="_blank" rel="noopener noreferrer" class="text-link hover:underline">
              {{ t('hub.homepage') }}
            </a>
          </div>

          <p class="text-[13px] text-muted-foreground">
            {{ t('hub.profile') }}: <code class="text-foreground/80">{{ profile }}</code>
          </p>
        </div>
      </div>
<!-- @@FOOTER@@ -->
    </div>

    <template #footer>
      <span v-if="opError" class="mr-2 truncate text-[13px] text-destructive" :title="opError">{{ opError }}</span>
      <div class="flex-1" />
      <template v-if="selected">
        <template v-if="selected.type === 'skill'">
          <span class="mr-1 self-center text-[13px] text-muted-foreground">{{ t('hub.skillManual') }}</span>
          <Button variant="outline" @click="copyCommand(selected)">
            <Check v-if="copied" class="h-4 w-4" />
            <Copy v-else class="h-4 w-4" />
            {{ copied ? t('hub.copied') : t('hub.copyCommand') }}
          </Button>
        </template>
        <Button
          v-else-if="isInstalled(selected)"
          variant="destructive"
          :disabled="busy"
          @click="uninstall(selected)"
        >
          {{ busy ? t('hub.working') : t('hub.remove') }}
        </Button>
        <Button v-else variant="primary" :disabled="busy" @click="install(selected)">
          {{ busy ? t('hub.working') : t('hub.confirm') }}
        </Button>
      </template>
      <Button variant="outline" :disabled="busy" @click="open = false">{{ t('hub.cancel') }}</Button>
    </template>
  </Dialog>
</template>


