<script setup lang="ts">
// The 下载/市场 dialog: browse a decentralized market, install into one instance.
//
// One widget, three kinds — the plugin / skill / MCP dialogs differ only by the
// `kind` prop, exactly as the three PrismLauncher marketplace dialogs do.
//
// Contract EditInstanceDialog.vue mounts this against:
//   v-model:open  — dialog visibility
//   :kind         — "plugin" | "skill" | "mcp" (picks the title and the query)
//   :instance-id  — install target; "" ⇒ the install button stays *disabled* rather
//                   than hidden, because a greyed control says why it cannot be
//                   used and a missing one just looks broken
//   @installed    — emitted after a successful install so the calling section
//                   re-reads `api.readInstanceExtensions` from disk instead of
//                   guessing what the install did
//
// All data arrives through `api` (market_fetch / market_readme / market_install …),
// which is to say through the backend: a URL a user typed into sources.json owes us
// no CORS header, and the results are disk-cached so this dialog opens offline.
// Nothing is imported from `src/lib/market/*` — that webview-fetch layer is gone.
//
// The state handling below is the substance of this file. A market that half-works
// is the normal case, so "these came from the cache", "some sources failed" and
// "no source serves this kind" each say their own sentence instead of collapsing
// into one polite "no results".
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { Check, Copy, RefreshCw } from "lucide-vue-next";
import Dialog from "@/components/ui/Dialog.vue";
import Button from "@/components/ui/Button.vue";
import Input from "@/components/ui/Input.vue";
import Select, { type SelectOption } from "@/components/ui/Select.vue";
import MarketRow from "@/components/market/MarketRow.vue";
import MarketDetail from "@/components/market/MarketDetail.vue";
import { api } from "@/lib/api";
import { useI18n } from "@/lib/i18n";
import type {
  ExtensionKind,
  InstallSpec,
  MarketItem,
  MarketQuery,
  SourceDef,
  SourceStatus,
} from "@/types";

const { t } = useI18n();
const open = defineModel<boolean>("open", { default: false });
const props = defineProps<{ kind: ExtensionKind; instanceId: string }>();
const emit = defineEmits<{ installed: [] }>();

const title = computed(() => t(`market.title.${props.kind}`));

// ---- page state -----------------------------------------------------------
// `limit` is ours rather than the backend's own page size, because the `offset`
// arithmetic behind market.loadMore has to be predictable from this side.
const PAGE = 30;

const items = ref<MarketItem[]>([]);
const total = ref(0);
const stale = ref(false);
const statuses = ref<SourceStatus[]>([]);
const sources = ref<SourceDef[]>([]);
const loading = ref(false);
const loadingMore = ref(false);
const refreshing = ref(false);
const loadError = ref("");
/** a fetch has *completed*. Before that, "no sources" and "no results" are both
 *  only "not known yet", and neither may be printed. */
const fetched = ref(false);

const search = ref("");
const debounced = ref("");
const sourceFilter = ref("");
const sort = ref("relevance");
const activeTags = ref<string[]>([]);

const selectedId = ref("");
const versionIdx = ref("0");

const readme = ref("");
const readmeLoading = ref(false);
const readmeError = ref("");
const readmeCache = new Map<string, string>();

const busyKind = ref<"" | "install" | "uninstall">("");
const busy = computed(() => busyKind.value !== "");
const note = ref("");
const noteIsInstall = ref(false);
const opError = ref("");
const copied = ref(false);

function errText(e: unknown): string {
  // Tauri rejects with the Rust `Err(String)`, so this is the backend's own words —
  // which is the only text that knows *why* an install failed.
  return e instanceof Error ? e.message : String(e);
}

// ---- fetching -------------------------------------------------------------
// A slow answer to a query the user has already replaced must not overwrite the
// newer one, so every request takes a ticket and only the current ticket may write.
let seq = 0;

async function load(more = false): Promise<void> {
  const ticket = ++seq;
  if (more) {
    loadingMore.value = true;
  } else {
    loading.value = true;
    loadError.value = "";
  }
  const query: MarketQuery = {
    // Never widened: this dialog shows the one kind it was opened for.
    kind: props.kind,
    query: debounced.value,
    sources: sourceFilter.value ? [sourceFilter.value] : [],
    tags: [...activeTags.value],
    sort: sort.value,
    offset: more ? items.value.length : 0,
    limit: PAGE,
  };
  try {
    const page = await api.marketFetch(query);
    if (ticket !== seq) return;
    items.value = more ? [...items.value, ...page.items] : page.items;
    total.value = page.total;
    stale.value = page.stale;
    statuses.value = page.statuses;
    fetched.value = true;
  } catch (e) {
    if (ticket !== seq) return;
    loadError.value = errText(e);
    if (!more) {
      items.value = [];
      total.value = 0;
    }
  } finally {
    if (ticket === seq) {
      loading.value = false;
      loadingMore.value = false;
    }
  }
}

async function refresh(): Promise<void> {
  if (loading.value || refreshing.value) return;
  refreshing.value = true;
  try {
    await api.marketRefresh();
  } catch {
    // A refresh that could not even start is not fatal: the fetch below still
    // serves whatever the cache holds, and its own statuses report the failure.
  } finally {
    refreshing.value = false;
  }
  await load();
}

async function loadSources(): Promise<void> {
  try {
    sources.value = (await api.getMarketSources()).sources;
  } catch {
    // Only the source *filter* and the row labels want this; without it the list
    // still works and falls back to the bare source id.
    sources.value = [];
  }
}

// Which sources can answer for this kind at all — mirrors SourceDef::serves.
const kindSources = computed(() =>
  sources.value.filter((s) => s.enabled && s.kinds.includes(props.kind))
);
function sourceLabel(id: string): string {
  return sources.value.find((s) => s.id === id)?.label || id;
}

// ---- installed state ------------------------------------------------------
// A MarketItem carries no `installed` flag — a feed cannot know what is on this
// machine — so the marker is derived from the instance's own extension state, the
// same read the edit dialog's three sections do.
const installedPlugins = ref<string[]>([]);
const installedSkills = ref<string[]>([]);
const installedMcp = ref<string[]>([]);

async function loadInstalled(): Promise<void> {
  installedPlugins.value = [];
  installedSkills.value = [];
  installedMcp.value = [];
  if (!props.instanceId) return;
  try {
    const ext = await api.readInstanceExtensions(props.instanceId);
    installedPlugins.value = ext.plugins;
    installedSkills.value = ext.skills.map((s) => s.name);
    installedMcp.value = ext.mcp.map((s) => s.name);
  } catch {
    // Unreadable extension state means "cannot say", which shows as no marker
    // rather than as "not installed".
  }
}

/** Mirrors market/install.rs::slug — the directory a git-clone skill lands in.
 *  Used only to *report* installed state; an uninstall re-derives the real
 *  directory backend-side, so drift here can mislabel a row but can never make the
 *  launcher delete the wrong one. */
function slug(name: string): string {
  const s = name
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]/gu, "-")
    .replace(/-+/g, "-")
    .replace(/^-+|-+$/g, "");
  return s || "skill";
}

/** The methods the launcher can actually carry out; anything else is manual. */
const KNOWN_METHODS = ["pnpm-profile", "git-clone", "mcp-config"];

function isInstalled(item: MarketItem, spec: InstallSpec | null): boolean {
  if (!spec) return false;
  switch (spec.method) {
    case "pnpm-profile":
      return (
        spec.package.trim() !== "" && installedPlugins.value.includes(spec.package.trim())
      );
    case "git-clone":
      return installedSkills.value.includes(slug(item.name));
    case "mcp-config": {
      const name = spec.mcp?.name ?? "";
      return name !== "" && installedMcp.value.includes(name);
    }
    default:
      return false;
  }
}

/** A row cannot know which version the user will pick, so it judges by the newest. */
function rowInstalled(item: MarketItem): boolean {
  return isInstalled(item, item.versions[0]?.install ?? null);
}

// ---- selection, versions, the one action the footer offers -----------------
const selected = computed<MarketItem | null>(
  () => items.value.find((i) => i.id === selectedId.value) ?? null
);
const versions = computed(() => selected.value?.versions ?? []);
// Keyed by index, not by version string: a thin feed can publish the same string
// twice, and duplicate option values would make the picker jump.
const versionOptions = computed<SelectOption[]>(() =>
  versions.value.map((v, i) => ({ value: String(i), label: v.version || t("common.none") }))
);
const spec = computed<InstallSpec | null>(
  () => versions.value[Number(versionIdx.value)]?.install ?? null
);
// `InstallSpec.method` is a plain string on purpose: an unknown method degrades to
// manual — copy a command — instead of an Install button the backend would reject.
const manual = computed(() => !!spec.value && !KNOWN_METHODS.includes(spec.value.method));
const installed = computed(() =>
  selected.value ? isInstalled(selected.value, spec.value) : false
);
// An item with no versions is not installable at all; the button stays disabled and
// visible for the same reason it does when there is no instance to install into.
const canOperate = computed(
  () => !!props.instanceId && !!spec.value && !manual.value && !busy.value
);

/** A feed's URL is a string a stranger wrote: only http(s) is worth handing to the
 *  OS opener, and it goes through the backend command rather than becoming an href
 *  the webview would follow itself. */
function isHttp(url: string): boolean {
  return /^https?:\/\//i.test(url.trim());
}
const links = computed(() => {
  const item = selected.value;
  if (!item) return [];
  const out: { label: string; url: string }[] = [];
  if (isHttp(item.homepage)) out.push({ label: t("market.homepage"), url: item.homepage.trim() });
  if (isHttp(item.repo)) out.push({ label: t("market.repo"), url: item.repo.trim() });
  return out;
});
function openLink(url: string): void {
  if (!isHttp(url)) return;
  api.openUrl(url.trim()).catch(() => {
    /* whether a browser exists is the OS's business, not a banner in this dialog */
  });
}

// Tag chips come from the items actually on screen: there is no tag-index command,
// and a fixed tag vocabulary would lie about what these feeds carry. A chip that is
// switched on stays listed even when the filtered page no longer contains it —
// otherwise it could not be switched off again.
const tagChips = computed(() => {
  const counts = new Map<string, number>();
  for (const item of items.value) {
    for (const raw of item.tags) {
      const tag = raw.trim();
      if (tag) counts.set(tag, (counts.get(tag) ?? 0) + 1);
    }
  }
  for (const tag of activeTags.value) if (!counts.has(tag)) counts.set(tag, 0);
  const on = (tag: string) => (activeTags.value.includes(tag) ? 1 : 0);
  return [...counts.entries()]
    .sort((a, b) => on(b[0]) - on(a[0]) || b[1] - a[1] || a[0].localeCompare(b[0]))
    .slice(0, 12)
    .map(([tag]) => tag);
});

// ---- filters --------------------------------------------------------------
// The filters fetch explicitly instead of through a watcher on the whole set: the
// reset that happens when the dialog opens would otherwise fire a second, identical
// request a moment later.
let debounceTimer: ReturnType<typeof setTimeout> | undefined;
watch(search, (q) => {
  clearTimeout(debounceTimer);
  debounceTimer = setTimeout(() => {
    const next = q.trim();
    if (next === debounced.value) return;
    debounced.value = next;
    void load();
  }, 250);
});

function setSource(v: string): void {
  sourceFilter.value = v;
  void load();
}
function setSort(v: string): void {
  sort.value = v;
  void load();
}
function toggleTag(tag: string): void {
  activeTags.value = activeTags.value.includes(tag)
    ? activeTags.value.filter((x) => x !== tag)
    : [...activeTags.value, tag];
  void load();
}

// ---- the detail pane's Markdown, fetched lazily ---------------------------
// One item's README at a time: a list payload carrying every item's Markdown would
// be a far bigger request for text nobody read. Same ticket rule as the list — a
// slow README cannot land in a pane that has moved on to another item.
let readmeSeq = 0;
watch(selectedId, (id) => {
  versionIdx.value = "0";
  note.value = "";
  opError.value = "";
  copied.value = false;
  readme.value = "";
  readmeError.value = "";
  if (!id) return;
  const inline = items.value.find((i) => i.id === id)?.readme ?? "";
  if (inline) {
    readme.value = inline;
    return;
  }
  const cached = readmeCache.get(id);
  if (cached !== undefined) {
    readme.value = cached;
    return;
  }
  const ticket = ++readmeSeq;
  readmeLoading.value = true;
  api
    .marketReadme(id)
    .then((md) => {
      if (ticket !== readmeSeq) return;
      readmeCache.set(id, md);
      readme.value = md;
    })
    .catch((e) => {
      if (ticket !== readmeSeq) return;
      readmeError.value = errText(e);
    })
    .finally(() => {
      if (ticket === readmeSeq) readmeLoading.value = false;
    });
});

// ---- install / uninstall / copy -------------------------------------------
async function runInstall(): Promise<void> {
  const item = selected.value;
  const sp = spec.value;
  if (!item || !sp || !props.instanceId || busy.value) return;
  busyKind.value = "install";
  opError.value = "";
  note.value = "";
  try {
    // The note is a path, a package or a server name — what actually landed where.
    note.value = await api.marketInstall(props.instanceId, item.name, sp);
    noteIsInstall.value = true;
    await loadInstalled();
    emit("installed");
  } catch (e) {
    opError.value = errText(e);
  } finally {
    busyKind.value = "";
  }
}

async function runUninstall(): Promise<void> {
  const item = selected.value;
  const sp = spec.value;
  if (!item || !sp || !props.instanceId || busy.value) return;
  busyKind.value = "uninstall";
  opError.value = "";
  note.value = "";
  try {
    note.value = await api.marketUninstall(props.instanceId, item.name, sp);
    noteIsInstall.value = false;
    await loadInstalled();
    // The instance's extension state changed either way, so the calling section
    // has to re-read even though nothing was added.
    emit("installed");
  } catch (e) {
    opError.value = errText(e);
  } finally {
    busyKind.value = "";
  }
}

let copyTimer: ReturnType<typeof setTimeout> | undefined;
async function copyCommand(): Promise<void> {
  const cmd = spec.value?.command ?? "";
  if (!cmd) return;
  try {
    await navigator.clipboard.writeText(cmd);
    copied.value = true;
    clearTimeout(copyTimer);
    copyTimer = setTimeout(() => (copied.value = false), 1500);
  } catch {
    /* no clipboard access: the command is on screen to select by hand */
  }
}

// ---- open / close ---------------------------------------------------------
function resetFilters(): void {
  clearTimeout(debounceTimer);
  search.value = "";
  debounced.value = "";
  sourceFilter.value = "";
  sort.value = "relevance";
  activeTags.value = [];
}

// `kind` is watched alongside `open` because the parent keeps one instance of this
// dialog and swaps the kind just before showing it. The filters reset with it: a tag
// or a source chosen in the plugin dialog means nothing in the MCP one.
watch(
  () => [open.value, props.kind] as const,
  ([isOpen]) => {
    if (!isOpen) return;
    resetFilters();
    selectedId.value = "";
    items.value = [];
    total.value = 0;
    statuses.value = [];
    stale.value = false;
    fetched.value = false;
    note.value = "";
    opError.value = "";
    void loadSources();
    void loadInstalled();
    void load();
  }
);

onBeforeUnmount(() => {
  clearTimeout(debounceTimer);
  clearTimeout(copyTimer);
});

// ---- what the list is currently telling the truth about --------------------
const failedSources = computed(() => statuses.value.filter((s) => !s.ok));
const okSources = computed(() => statuses.value.filter((s) => s.ok).length);
/** No enabled source serves this kind — there was nothing to ask, which is a very
 *  different thing from having asked and matched nothing. */
const noSources = computed(() => fetched.value && statuses.value.length === 0);
/** Every source we asked failed: the list is empty because the fetch broke. */
const allFailed = computed(
  () => fetched.value && statuses.value.length > 0 && okSources.value === 0
);
/** Some worked and some did not: show the items *and* say so. Throwing a whole page
 *  away because one feed 404'd would be the worse answer. */
const partialFailure = computed(() => okSources.value > 0 && failedSources.value.length > 0);
const hasMore = computed(() => items.value.length > 0 && items.value.length < total.value);

const sourceOptions = computed<SelectOption[]>(() => [
  { value: "", label: t("market.allSources") },
  ...kindSources.value.map((s) => ({ value: s.id, label: s.label || s.id })),
]);
const sortOptions = computed<SelectOption[]>(() => [
  { value: "relevance", label: t("market.sort.relevance") },
  { value: "name", label: t("market.sort.name") },
  { value: "downloads", label: t("market.sort.downloads") },
  { value: "updated", label: t("market.sort.updated") },
]);

</script>

<template>
  <!-- Prism's mod-download dialog, kept to its own furniture: filters on top, a
       plain list on the left, a detail pane on the right, controls in the footer. -->
  <Dialog v-model:open="open" width="max-w-5xl" class="h-[80vh]" :title="title">
    <div class="flex h-full min-h-0 flex-col">
      <div class="shrink-0 border-b border-border bg-toolbar px-3 py-2">
        <div class="flex items-center gap-2">
          <Input v-model="search" :placeholder="t('market.search')" class="flex-1" />
          <span class="shrink-0 text-[13px] text-muted-foreground">{{ t("market.sources") }}</span>
          <Select
            :model-value="sourceFilter"
            :options="sourceOptions"
            class="w-40 shrink-0"
            @update:model-value="setSource"
          />
          <span class="shrink-0 text-[13px] text-muted-foreground">{{ t("market.sort") }}</span>
          <Select
            :model-value="sort"
            :options="sortOptions"
            class="w-32 shrink-0"
            @update:model-value="setSort"
          />
          <Button
            variant="outline"
            size="icon"
            :title="t('market.refresh')"
            :disabled="loading || refreshing"
            @click="refresh"
          >
            <RefreshCw class="h-4 w-4" :class="(loading || refreshing) && 'animate-spin'" />
          </Button>
        </div>
        <div v-if="tagChips.length" class="mt-2 flex flex-wrap items-center gap-1">
          <span class="text-[12px] text-muted-foreground">{{ t("market.tags") }}</span>
          <button
            v-for="tag in tagChips"
            :key="tag"
            type="button"
            class="max-w-[9rem] truncate rounded px-1.5 py-0.5 text-[12px] transition-colors"
            :class="
              activeTags.includes(tag)
                ? 'bg-selection text-selection-foreground'
                : 'bg-accent/60 text-foreground/80 hover:bg-accent'
            "
            @click="toggleTag(tag)"
          >
            {{ tag }}
          </button>
        </div>
      </div>

      <!-- Where the page came from, stated before the page itself. Cached data is
           not presented as live, and a partly-broken source list still shows the
           sources that worked. -->
      <p
        v-if="stale"
        class="shrink-0 border-b border-border/60 bg-accent/40 px-3 py-1 text-[12px] text-muted-foreground"
      >
        {{ t("market.stale") }}
      </p>
      <p
        v-if="partialFailure"
        class="shrink-0 border-b border-border/60 bg-accent/40 px-3 py-1 text-[12px] text-muted-foreground"
      >
        <span class="text-destructive">{{ t("market.sourceErrors") }}</span>
        <span v-for="s in failedSources" :key="s.id" class="ml-2 break-all">
          {{ sourceLabel(s.id) }}: {{ s.error }}
        </span>
      </p>

      <div class="flex min-h-0 flex-1">
        <div class="flex min-w-0 flex-1 flex-col overflow-y-auto border-r border-border">
          <p v-if="loading && !items.length" class="py-10 text-center text-[13px] text-muted-foreground">
            {{ t("market.loading") }}
          </p>
          <div v-else-if="loadError" class="flex flex-col items-center gap-2 py-10 text-center">
            <p class="px-4 text-[13px] text-destructive">{{ t("market.loadError") }}</p>
            <p class="px-6 text-[12px] text-muted-foreground">{{ loadError }}</p>
            <Button variant="outline" size="sm" @click="load()">{{ t("common.retry") }}</Button>
          </div>
          <!-- Nothing was asked, so nothing can be missing: point at the place the
               source list is edited. There is no seam for opening Settings from
               here, hence a written path rather than a button that lies. -->
          <div v-else-if="noSources" class="flex flex-col items-center gap-1 py-10 text-center">
            <p class="text-[13px] text-muted-foreground">{{ t("market.noSources") }}</p>
            <p class="px-6 text-[12px] text-muted-foreground">
              {{ t("market.manageSources") }} · {{ t("settings.title") }} →
              {{ t("settings.nav.sources") }}
            </p>
          </div>
          <div
            v-else-if="allFailed && !items.length"
            class="flex flex-col items-center gap-2 py-10 text-center"
          >
            <p class="text-[13px] text-destructive">{{ t("market.loadError") }}</p>
            <p
              v-for="s in failedSources"
              :key="s.id"
              class="break-all px-6 text-[12px] text-muted-foreground"
            >
              {{ sourceLabel(s.id) }}: {{ s.error }}
            </p>
            <Button variant="outline" size="sm" @click="refresh">{{ t("common.retry") }}</Button>
          </div>
          <!-- Asked, worked, matched nothing. Deliberately not the same sentence as
               "no sources": one is a query, the other is a configuration. -->
          <p v-else-if="!items.length" class="py-10 text-center text-[13px] text-muted-foreground">
            {{ t("market.empty") }}
          </p>
          <template v-else>
            <MarketRow
              v-for="item in items"
              :key="item.id"
              :item="item"
              :selected="item.id === selectedId"
              :installed="rowInstalled(item)"
              :source-label="sourceLabel(item.source)"
              @select="selectedId = item.id"
            />
            <div v-if="hasMore" class="px-3 py-2 text-center">
              <Button variant="outline" size="sm" :disabled="loadingMore" @click="load(true)">
                {{ loadingMore ? t("market.loading") : t("market.loadMore") }}
              </Button>
            </div>
          </template>
        </div>

        <div class="w-80 shrink-0 overflow-y-auto bg-panel">
          <p v-if="!selected" class="px-4 py-10 text-center text-[13px] text-muted-foreground">
            {{ t("market.selectHint") }}
          </p>
          <MarketDetail
            v-else
            :item="selected"
            :spec="spec"
            :manual="manual"
            :readme="readme"
            :readme-loading="readmeLoading"
            :readme-error="readmeError"
            :links="links"
            @open="openLink"
          />
        </div>
      </div>
    </div>

    <template #footer>
      <!-- One line about the last thing that happened, error text included: an
           install failure the user cannot read is an install failure they cannot
           fix. The empty-target notice comes first because it is the standing
           explanation for the disabled button beside it. -->
      <span v-if="!instanceId" class="min-w-0 truncate text-[13px] text-muted-foreground">
        {{ t("market.target") }}: {{ t("common.none") }}
      </span>
      <span
        v-else-if="opError"
        class="min-w-0 truncate text-[13px] text-destructive"
        :title="opError"
      >
        {{ t("market.installError") }}: {{ opError }}
      </span>
      <span v-else-if="busyKind" class="text-[13px] text-muted-foreground">
        {{ busyKind === "install" ? t("market.installing") : t("market.uninstalling") }}
      </span>
      <span
        v-else-if="note"
        class="min-w-0 truncate text-[13px] text-muted-foreground"
        :title="note"
      >
        <template v-if="noteIsInstall">
          {{ t("market.installed") }} · {{ t("market.installedTo") }} {{ note }}
        </template>
        <template v-else>{{ note }}</template>
      </span>
      <div class="flex-1" />
      <template v-if="selected">
        <span class="shrink-0 text-[13px] text-muted-foreground">{{ t("market.version") }}</span>
        <Select
          v-model="versionIdx"
          :options="versionOptions"
          :disabled="!versionOptions.length"
          :placeholder="t('common.none')"
          class="w-36 shrink-0"
        />
        <Button
          v-if="manual"
          variant="outline"
          :disabled="!spec || !spec.command"
          @click="copyCommand"
        >
          <Check v-if="copied" class="h-4 w-4" />
          <Copy v-else class="h-4 w-4" />
          {{ copied ? t("market.copied") : t("market.copyCommand") }}
        </Button>
        <Button
          v-else-if="installed"
          variant="destructive"
          :disabled="!canOperate"
          @click="runUninstall"
        >
          {{ t("market.uninstall") }}
        </Button>
        <Button v-else variant="primary" :disabled="!canOperate" @click="runInstall">
          {{ busyKind === "install" ? t("market.installing") : t("market.install") }}
        </Button>
      </template>
      <Button variant="outline" :disabled="busy" @click="open = false">
        {{ t("common.close") }}
      </Button>
    </template>
  </Dialog>
</template>

