<script setup lang="ts">
// The Settings 数据源 section: the decentralized source list as an editable table.
// SettingsDialog mounts this with no props, so the section owns its own load/save
// cycle and the dialog keeps no market state.
//
// Three decisions worth knowing about:
//
//  * Nothing is fetched on mount. A refresh is a real network round trip — the one
//    feed we ship is ~11 MB of JSON — and opening a settings pane must not spend
//    that by itself. Rows start at `sources.never`; the per-row button is the ask.
//  * Refresh saves first when the form is dirty, because the backend refreshes the
//    row as written in ~/.agentlauncher/sources.json. Testing a URL you have just
//    typed would otherwise quietly test the previous one.
//  * `builtin` is never set here. It is asserted server-side from the shipped id
//    list, so rows carry whatever flag the backend gave them and new rows say
//    `false` only because the wire shape requires the field.
import { computed, onMounted, ref } from "vue";
import { Check, Plus, RefreshCw, Trash2 } from "lucide-vue-next";
import Button from "@/components/ui/Button.vue";
import GroupBox from "@/components/ui/GroupBox.vue";
import Input from "@/components/ui/Input.vue";
import Select, { type SelectOption } from "@/components/ui/Select.vue";
import { api } from "@/lib/api";
import { useI18n } from "@/lib/i18n";
import type { SourceDef, SourceStatus } from "@/types";

const { t } = useI18n();

const rows = ref<SourceDef[]>([]);
const formatVersion = ref(1);
/** Only ever what a refresh actually reported, keyed by source id. */
const statuses = ref<Record<string, SourceStatus>>({});
/** The id currently being refreshed — one at a time, so a slow feed is obvious. */
const busy = ref("");
const loading = ref(true);
const saving = ref(false);
const savedFlash = ref(false);
const error = ref("");
/** The list as last read from disk, for a cheap dirty check. */
const baseline = ref("");

const KINDS = ["plugin", "skill", "mcp"] as const;
/** Payload shapes this build knows how to normalise. */
const ADAPTERS = ["agentlauncher", "dsh-market", "mcp-registry"];

const kindOptions = computed<SelectOption[]>(() => [
  { value: "http", label: t("sources.kind.http") },
  { value: "dir", label: t("sources.kind.dir") },
]);

/** Adapter names are identifiers, not prose, so they are shown verbatim. A shape a
 *  hand-edited sources.json names but this build does not know stays in the list —
 *  opening Settings must not silently rewrite it. */
function adapterOptions(row: SourceDef): SelectOption[] {
  const known = ADAPTERS.map((a) => ({ value: a, label: a }));
  if (row.adapter && !ADAPTERS.includes(row.adapter)) {
    known.push({ value: row.adapter, label: row.adapter });
  }
  return known;
}

function kindLabel(kind: string): string {
  if (kind === "plugin") return t("ext.plugins.title");
  if (kind === "skill") return t("ext.skills.title");
  return t("ext.mcp.title");
}

/** Exactly what `sources::save` would drop or reject, reported next to the field
 *  instead of after a round trip. */
const errors = computed(() =>
  rows.value.map((row, i) => {
    const out: { id: string; url: string } = { id: "", url: "" };
    const id = row.id.trim();
    if (!id) out.id = t("sources.idRequired");
    else if (rows.value.some((o, j) => j !== i && o.id.trim() === id)) {
      out.id = t("sources.idDuplicate");
    }
    // A blank `dir` url is not missing: it selects the default drop-in directory.
    if (row.kind !== "dir" && !row.url.trim()) out.url = t("sources.urlRequired");
    return out;
  })
);

const valid = computed(() => errors.value.every((e) => !e.id && !e.url));
const dirty = computed(() => JSON.stringify(rows.value) !== baseline.value);
/** Parallel to `rows`, so the template indexes status the same way it does errors. */
const rowStatus = computed(() =>
  rows.value.map((r): SourceStatus | undefined => statuses.value[r.id.trim()])
);

async function load(silent = false) {
  if (!silent) loading.value = true;
  try {
    const doc = await api.getMarketSources();
    formatVersion.value = doc.format_version || 1;
    rows.value = doc.sources;
    baseline.value = JSON.stringify(rows.value);
  } catch (e) {
    // The backend's message names the file it could not read, which is more useful
    // than a generic string of ours would be.
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}
onMounted(() => load());

async function persist(): Promise<boolean> {
  if (!valid.value) return false;
  saving.value = true;
  error.value = "";
  try {
    await api.setMarketSources({
      format_version: formatVersion.value,
      sources: rows.value.map((r) => ({ ...r, id: r.id.trim(), url: r.url.trim() })),
    });
    // `save` re-seeds any built-in the list lost and re-asserts `builtin`, so what
    // we keep editing is the list on disk rather than the one we sent.
    await load(true);
    savedFlash.value = true;
    window.setTimeout(() => (savedFlash.value = false), 1600);
    return true;
  } catch (e) {
    error.value = String(e);
    return false;
  } finally {
    saving.value = false;
  }
}

function addRow() {
  rows.value.push({
    id: "",
    label: "",
    kind: "http",
    url: "",
    adapter: "agentlauncher",
    kinds: ["plugin"],
    enabled: true,
    builtin: false,
  });
}

/** Built-ins are re-seeded by the backend, so the template offers no delete for
 *  them; this guard exists so the list cannot lose one by any other route. */
function removeRow(i: number) {
  if (rows.value[i]?.builtin) return;
  rows.value.splice(i, 1);
}

function toggleKind(row: SourceDef, kind: string, on: boolean) {
  if (on) {
    if (!row.kinds.includes(kind)) row.kinds = [...row.kinds, kind];
  } else {
    row.kinds = row.kinds.filter((k) => k !== kind);
  }
}

async function refresh(row: SourceDef) {
  if (!valid.value || busy.value) return;
  const id = row.id.trim();
  // Saving first replaces `rows` with the reloaded list, so the id is captured above.
  if (dirty.value && !(await persist())) return;
  busy.value = id;
  error.value = "";
  try {
    for (const st of await api.marketRefresh(id)) statuses.value[st.id] = st;
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = "";
  }
}

/** `fetched_at` is RFC3339 from the backend; show it in the user's own locale, and
 *  fall back to the raw stamp rather than to "Invalid Date". */
function shortTime(stamp: string): string {
  const d = new Date(stamp);
  return Number.isNaN(d.getTime()) ? stamp : d.toLocaleString();
}
</script>

<template>
  <GroupBox :title="t('sources.title')">
    <p class="text-[13px] text-muted-foreground">{{ t('sources.desc') }}</p>

    <p v-if="loading" class="mt-3 text-[13px] text-muted-foreground">{{ t('common.loading') }}</p>

    <div v-else class="mt-3 flex flex-col gap-2.5">
      <div
        v-for="(row, i) in rows"
        :key="i"
        class="rounded border border-border px-3 py-2.5"
      >
        <div class="flex items-center gap-2">
          <label class="flex shrink-0 items-center gap-1.5 text-[13px] text-foreground/85">
            <input type="checkbox" v-model="row.enabled" class="h-3.5 w-3.5 accent-selection" />
            {{ t('sources.enabled') }}
          </label>
          <Input
            v-model="row.label"
            class="h-7 min-w-0 flex-1 text-[13px]"
            :placeholder="t('sources.label')"
          />
          <span
            v-if="row.builtin"
            class="shrink-0 rounded-sm border border-border px-1.5 py-0.5 text-[11px] text-muted-foreground"
          >
            {{ t('sources.builtin') }}
          </span>
          <Button
            size="sm"
            variant="outline"
            :disabled="!valid || !!busy || saving"
            @click="refresh(row)"
          >
            <RefreshCw class="h-3.5 w-3.5" :class="busy === row.id.trim() ? 'animate-spin' : ''" />
            {{ busy === row.id.trim() ? t('sources.refreshing') : t('sources.refresh') }}
          </Button>
          <Button
            v-if="!row.builtin"
            size="sm"
            variant="ghost"
            :title="t('common.delete')"
            @click="removeRow(i)"
          >
            <Trash2 class="h-3.5 w-3.5" />
          </Button>
        </div>

        <div class="mt-2 grid grid-cols-[92px_1fr] items-center gap-x-3 gap-y-2">
          <label class="text-[13px] text-foreground/85">{{ t('sources.id') }}</label>
          <div class="flex min-w-0 flex-col gap-1">
            <!-- A built-in is identified by its id, so re-pointing one means editing
                 its url, not its name: an edited id would come back as a new row. -->
            <Input
              v-model="row.id"
              class="h-7 font-mono text-[13px]"
              :disabled="row.builtin"
              placeholder="team-feed"
            />
            <span v-if="errors[i].id" class="text-[12px] text-destructive">{{ errors[i].id }}</span>
          </div>

          <label class="text-[13px] text-foreground/85">{{ t('sources.kind') }}</label>
          <Select v-model="row.kind" :options="kindOptions" class="h-7 text-[13px]" />

          <label class="text-[13px] text-foreground/85">{{ t('sources.url') }}</label>
          <div class="flex min-w-0 flex-col gap-1">
            <Input
              v-model="row.url"
              class="h-7 font-mono text-[13px]"
              :placeholder="row.kind === 'dir' ? '~/.agentlauncher/sources' : 'https://…'"
            />
            <span class="text-[12px] text-muted-foreground">
              {{ row.kind === 'dir' ? t('sources.urlHintDir') : t('sources.urlHintHttp') }}
            </span>
            <span v-if="errors[i].url" class="text-[12px] text-destructive">{{ errors[i].url }}</span>
          </div>

          <label class="text-[13px] text-foreground/85">{{ t('sources.adapter') }}</label>
          <Select v-model="row.adapter" :options="adapterOptions(row)" class="h-7 text-[13px]" />

          <label class="text-[13px] text-foreground/85">{{ t('sources.kinds') }}</label>
          <div class="flex flex-wrap items-center gap-x-4 gap-y-1.5">
            <label
              v-for="k in KINDS"
              :key="k"
              class="flex items-center gap-1.5 text-[13px] text-foreground/85"
            >
              <input
                type="checkbox"
                class="h-3.5 w-3.5 accent-selection"
                :checked="row.kinds.includes(k)"
                @change="toggleKind(row, k, ($event.target as HTMLInputElement).checked)"
              />
              {{ kindLabel(k) }}
            </label>
          </div>
        </div>

        <div class="mt-2 flex flex-wrap items-center gap-x-2 gap-y-1 text-[12px] text-muted-foreground">
          <template v-if="rowStatus[i]">
            <span
              class="inline-flex h-[7px] w-[7px] shrink-0 rounded-full"
              :class="rowStatus[i]!.ok ? 'bg-selection' : 'bg-destructive'"
            />
            <span :class="rowStatus[i]!.ok ? '' : 'text-destructive'">
              {{ rowStatus[i]!.ok ? t('sources.ok') : t('sources.failed') }}
            </span>
            <span>·</span>
            <span>{{ rowStatus[i]!.item_count }} {{ t('sources.items') }}</span>
            <template v-if="rowStatus[i]!.fetched_at">
              <span>·</span>
              <span>{{ t('sources.lastFetched') }} {{ shortTime(rowStatus[i]!.fetched_at) }}</span>
            </template>
            <span v-if="rowStatus[i]!.error" class="w-full break-all text-destructive">
              {{ rowStatus[i]!.error }}
            </span>
          </template>
          <template v-else>
            <span class="inline-flex h-[7px] w-[7px] shrink-0 rounded-full bg-muted-foreground/45" />
            <span>{{ t('sources.never') }}</span>
          </template>
        </div>
      </div>

      <p class="text-[12px] text-muted-foreground">{{ t('sources.builtinHint') }}</p>

      <div class="mt-1 flex items-center gap-3">
        <Button variant="outline" size="sm" @click="addRow">
          <Plus class="h-3.5 w-3.5" />
          {{ t('sources.add') }}
        </Button>
        <div class="flex-1" />
        <span v-if="savedFlash" class="flex items-center gap-1 text-[13px] text-emerald-400">
          <Check class="h-3.5 w-3.5" />
          {{ t('common.saved') }}
        </span>
        <Button variant="primary" size="sm" :disabled="!valid || saving || !dirty" @click="persist()">
          {{ t('common.save') }}
        </Button>
      </div>

      <p v-if="error" class="break-all text-[12px] text-destructive">{{ error }}</p>
    </div>
  </GroupBox>
</template>
