<script setup lang="ts">
// Settings → 模型与 API, all of it in one box, and — since 「从本源上精简」 — asking
// for only the four things a user can be expected to know: **服务商、Base URL、
// API Key、模型列表**.
//
// Everything else the provider record needs is now *derived* rather than typed. The
// id is slugified from the name (`My Proxy 2` → `my-proxy-2`), both env var names
// follow from the id (`MY_PROXY_2_API_KEY` / `MY_PROXY_2_BASE_URL`), and the auth
// scheme is worked out by trying bearer and retrying as x-api-key. All of that lives
// in `providers::save` / `providers::detect`, not here, so a row that arrives by any
// other route gets the same treatment. 「高级」 reveals those three as overrides,
// along with the key list, 启用, and dsh's credential file: nothing became
// unreachable, it just stopped being the first thing on screen.
//
// The shape is master-detail: the 服务商 dropdown picks a row, and only that row's
// fields exist on screen. 「＋ 新增服务商」 and 「探测本机运行时」 are items in that
// same dropdown, which is what keeps the master row one control wide; both leave the
// rows unsaved, so neither can change how anything launches until 保存 is pressed.
//
// The invariant from the previous layout is unchanged and worth restating: **the
// frontend never holds a key value.** `getProviders` returns fingerprints,
// `setProviders` sends metadata only, and a value reaches disk by exactly one
// route — `setProviderKey`. 可见/不可见 toggles fingerprint ↔ dots because there is
// no plaintext on this side to reveal. Changing the selection clears the key drafts
// for the same reason: a half-typed secret has no business following the selection
// around the list.
import { computed, onMounted, ref, watch } from "vue";
import {
  Check,
  ChevronDown,
  ChevronRight,
  Eye,
  EyeOff,
  Globe,
  Plus,
  Trash2,
} from "lucide-vue-next";
import Button from "@/components/ui/Button.vue";
import GroupBox from "@/components/ui/GroupBox.vue";
import Input from "@/components/ui/Input.vue";
import Select, { type SelectOption } from "@/components/ui/Select.vue";
import { api } from "@/lib/api";
import { useI18n } from "@/lib/i18n";
import { modelConfig } from "@/lib/settings";
import type { LocalLlm, ProviderView } from "@/types";

const { t } = useI18n();

/** Sentinel dropdown values. The two "add" actions live inside the list so the
 *  master row stays exactly one control wide. */
const ADD = "__add";
const PROBE = "__probe";
/** The alias a first key gets. A key needs *a* name to be addressable, and asking
 *  the user to invent one was among the things nobody understood. */
const PRIMARY = "main";

const rows = ref<ProviderView[]>([]);
const loading = ref(true);
const saving = ref(false);
const savedFlash = ref(false);
const error = ref("");
/** The list as last read from disk, for a cheap dirty check. */
const baseline = ref("");

/** Which row is expanded. An *index*, not an id: a row the user just added has no
 *  id yet and still has to be selectable. */
const selIndex = ref(0);
const selected = computed<ProviderView | null>(() => rows.value[selIndex.value] ?? null);
/** Everything the four visible fields do not cover. Off screen until asked for. */
const advanced = ref(false);
/** Draft value for one key, keyed by alias. Only one provider is on screen, so the
 *  provider no longer needs to be part of the key. Never persisted, never part of
 *  `rows`, and dropped whenever the selection moves. */
const keyDraft = ref<Record<string, string>>({});
const keyAlias = ref("");
const shown = ref<Record<string, boolean>>({});
const fetching = ref(false);
const fetchNote = ref("");
const probing = ref(false);
const probeNote = ref("");
const newModel = ref("");
const addingModel = ref(false);

// dsh's own credential file, as one line rather than a box. It is a different store
// with a different reader — dsh resolves credential *references* out of
// ~/.dsh/.credentials.yaml, which holds one value per name and so cannot express the
// multi-key rotation above.
const dshStored = ref<string[]>([]);
const dshDraft = ref("");
const dshFlash = ref(false);

/** Empty is the normal answer now: the backend tries bearer and retries as
 *  x-api-key, so this is only here for a provider that needs to be pinned. */
const authOptions = computed<SelectOption[]>(() => [
  { value: "", label: t("providers.authAuto") },
  { value: "bearer", label: "bearer" },
  { value: "x-api-key", label: "x-api-key" },
]);

const ENV_RE = /^[A-Za-z_][A-Za-z0-9_]*$/;

/** Mirrors `providers::slug` in the backend, which is what actually assigns the id.
 *  Two things on this side need the same answer: telling duplicate names apart before
 *  a save, and addressing `setProviderKey` for a row that has never been saved. */
function slugify(name: string): string {
  let out = "";
  for (const ch of name.trim()) {
    if (/[a-zA-Z0-9]/.test(ch)) out += ch.toLowerCase();
    else if (!out.endsWith("-")) out += "-";
  }
  return out.replace(/^-+/, "").replace(/-+$/, "");
}
/** The id this row has, or will have once it is saved. */
function effId(row: ProviderView): string {
  return row.id.trim() || slugify(row.label);
}

const errors = computed(() =>
  rows.value.map((row, i) => {
    const id = effId(row);
    let name = "";
    if (!id) name = t("providers.nameRequired");
    else if (rows.value.some((o, j) => j !== i && effId(o) === id))
      name = t("providers.nameDuplicate");
    let env = "";
    for (const v of [row.api_key_env.trim(), row.base_url_env.trim()])
      if (!env && v && !ENV_RE.test(v)) env = t("providers.envInvalid");
    return { name, env };
  })
);
const valid = computed(() => errors.value.every((e) => !e.name && !e.env));
const dirty = computed(() => JSON.stringify(rows.value) !== baseline.value);
const selErr = computed(() => errors.value[selIndex.value] ?? { name: "", env: "" });
/** Rows that block 保存 but are not on screen — otherwise the button is disabled for
 *  a reason the user cannot see. */
const otherInvalid = computed(() =>
  rows.value
    .map((r, i) => ({ r, i }))
    .filter(({ i }) => i !== selIndex.value && (errors.value[i].name || errors.value[i].env))
    .map(({ r, i }) => r.label || r.id || `#${i + 1}`)
);
const providerOptions = computed<SelectOption[]>(() => {
  const out: SelectOption[] = rows.value.map((r, i) => ({
    value: String(i),
    label: r.label || r.id || t("providers.unnamed"),
    hint: !r.enabled
      ? t("providers.disabledHint")
      : effId(r) && effId(r) === modelConfig.provider
        ? t("providers.defaultBadge")
        : undefined,
    warn: !r.enabled,
  }));
  // The two actions, last. `Select` has no separator concept, so the labels carry
  // the distinction (a leading ＋ / 🔍) instead of a divider line.
  out.push({ value: ADD, label: `＋ ${t("providers.add")}` });
  out.push({ value: PROBE, label: `🔍 ${t("providers.probeItem")}`, hint: t("providers.localPorts") });
  return out;
});

const isDefault = computed(
  () => !!selected.value && !!effId(selected.value) && effId(selected.value) === modelConfig.provider
);
/** The selected provider's fetched list. The launcher default is only *shown* here
 *  when this row is the default one — a model belonging to another provider has no
 *  business appearing in this list, and an empty box is the honest reading of "this
 *  provider is not what new instances start from". */
const modelOptions = computed<SelectOption[]>(() => {
  const list = selected.value?.models ?? [];
  const out: SelectOption[] = list.map((m) => ({ value: m, label: m }));
  const cur = modelConfig.defaultModel.trim();
  if (isDefault.value && cur && !list.includes(cur))
    out.unshift({ value: cur, label: cur, hint: t("providers.modelNotListed") });
  return out;
});
/** Picking a model out of this provider's list says "new instances start from *this*
 *  provider and this model" — so it carries the provider with it. Anything else lets
 *  the pair drift into a default that names provider A and a model only B serves. */
function onPickModel(m: string): void {
  modelConfig.defaultModel = m;
  const row = selected.value;
  if (row && effId(row)) modelConfig.provider = effId(row);
}

/** The 「API Key」 field addresses the first key in the list. Somebody who never opens
 *  高级 has exactly one; somebody who wants rotation adds the rest there, and the
 *  backend walks all of them in turn at launch. */
const primary = computed(() => selected.value?.keys[0] ?? null);
const primaryAlias = computed(() => primary.value?.alias ?? PRIMARY);

/** What the backend will fill in when these are left empty, shown as the placeholder
 *  so 「空」 reads as 「自动」 rather than 「坏了」. Mirrors `providers::derive_envs`. */
const derivedEnv = computed(() => {
  const row = selected.value;
  if (!row) return { key: "", base: "" };
  let prefix = effId(row).replace(/[^A-Za-z0-9]/g, "_").toUpperCase();
  if (!prefix) return { key: "", base: "" };
  if (/^[0-9]/.test(prefix)) prefix = `_${prefix}`;
  const key = row.api_key_env.trim() || `${prefix}_API_KEY`;
  const stem = key.endsWith("_API_KEY") ? key.slice(0, -"_API_KEY".length) : prefix;
  return { key, base: `${stem}_BASE_URL` };
});
/** The variable dsh would have to hold the key under. Derived like everything else,
 *  so the dsh line still works for a row that never typed one. */
const activeEnv = computed(() => derivedEnv.value.key);
const dshHasIt = computed(() => !!activeEnv.value && dshStored.value.includes(activeEnv.value));

async function load(silent = false): Promise<void> {
  if (!silent) loading.value = true;
  error.value = "";
  try {
    rows.value = await api.getProviders();
    baseline.value = JSON.stringify(rows.value);
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
  // Names already in dsh's file, so the line under 高级 can say 已写入 rather than ask.
  dshStored.value = await api.listCredentialKeys().catch(() => []);
}
onMounted(async () => {
  await load();
  // Open on the launcher default when there is one. Nothing is *written* here.
  const i = rows.value.findIndex((r) => effId(r) && effId(r) === modelConfig.provider);
  selIndex.value = i >= 0 ? i : 0;
});

// Every draft is scoped to the row on screen; none of it follows the selection.
watch(selIndex, () => {
  keyDraft.value = {};
  keyAlias.value = "";
  shown.value = {};
  fetchNote.value = "";
  newModel.value = "";
  addingModel.value = false;
  dshDraft.value = "";
  dshFlash.value = false;
});

async function persist(): Promise<void> {
  if (!valid.value) return;
  saving.value = true;
  error.value = "";
  // Reselect by id afterwards: `set_providers` derives an id for a new row, dedupes,
  // and re-appends any builtin the user dropped — so an index does not survive the
  // round trip, but the id this row is *going* to have does.
  const wasId = selected.value ? effId(selected.value) : "";
  try {
    await api.setProviders(rows.value);
    await load(true);
    if (wasId) {
      const i = rows.value.findIndex((r) => r.id.trim() === wasId);
      if (i >= 0) selIndex.value = i;
    }
    savedFlash.value = true;
    setTimeout(() => (savedFlash.value = false), 1600);
  } catch (e) {
    error.value = String(e);
  } finally {
    saving.value = false;
  }
}

function addRow(seed?: Partial<ProviderView>): number {
  rows.value.push({
    id: "",
    label: "",
    api_key_env: "",
    base_url: "",
    base_url_env: "",
    // Empty means "work it out": bearer, retried as x-api-key if that is refused.
    auth_style: "",
    models: [],
    enabled: true,
    builtin: false,
    keys: [],
    ...seed,
  });
  selIndex.value = rows.value.length - 1;
  return selIndex.value;
}
function onPickProvider(v: string): void {
  if (v === ADD) {
    probeNote.value = "";
    addRow();
    return;
  }
  if (v === PROBE) {
    void probeLocal();
    return;
  }
  const i = Number(v);
  if (!Number.isInteger(i) || !rows.value[i]) return;
  probeNote.value = "";
  selIndex.value = i;
  // Picking a provider here *is* picking the launcher default — that fusion is the
  // whole point of folding the old 全局默认 LLM box into this one, and it is why there
  // is no 设为默认 button to press afterwards.
  applyAsDefault(rows.value[i]);
}

/** Make `row` the prefill for new instances. Only ever moves the model when the one
 *  on record cannot belong to this provider. */
function applyAsDefault(row: ProviderView): void {
  const id = effId(row);
  if (!id) return;
  modelConfig.provider = id;
  if (row.models.length && !row.models.includes(modelConfig.defaultModel))
    modelConfig.defaultModel = row.models[0];
}

function removeRow(): void {
  const row = selected.value;
  if (!row || row.builtin) return;
  rows.value.splice(selIndex.value, 1);
  selIndex.value = Math.max(0, Math.min(selIndex.value, rows.value.length - 1));
}

function addKey(): void {
  const row = selected.value;
  const alias = keyAlias.value.trim();
  if (!row || !alias) return;
  if (row.keys.some((k) => k.alias === alias)) return;
  row.keys.push({ alias, enabled: true, fingerprint: "", has_value: false });
  keyAlias.value = "";
  keyDraft.value[alias] = "";
}

/** The one route a secret takes to disk. Metadata is flushed first when dirty, because
 *  `set_provider_key` addresses the key by `(provider id, alias)` — a row that only
 *  exists in this tab has no id on disk to attach it to. The backend creates the key
 *  row if it is new, which is what lets the 「API Key」 field work on a provider that
 *  has never had one without anybody naming it. */
async function saveKey(alias: string): Promise<void> {
  const row = selected.value;
  const value = (keyDraft.value[alias] ?? "").trim();
  if (!row || !value) return;
  const id = effId(row);
  if (!id) return;
  error.value = "";
  try {
    if (dirty.value) await persist();
    await api.setProviderKey(id, alias, value);
    keyDraft.value[alias] = "";
    await load(true);
  } catch (e) {
    error.value = String(e);
  }
}
/** Dropping the row locally would only strip the metadata and orphan the value on
 *  disk, so send an empty value — the backend treats that as "delete this key". */
async function removeKey(alias: string): Promise<void> {
  const row = selected.value;
  if (!row) return;
  error.value = "";
  try {
    if (row.id.trim() && row.keys.some((k) => k.alias === alias && k.has_value)) {
      await api.setProviderKey(row.id.trim(), alias, "");
    }
    row.keys = row.keys.filter((k) => k.alias !== alias);
    delete keyDraft.value[alias];
    if (dirty.value) await persist();
  } catch (e) {
    error.value = String(e);
  }
}

/** For an endpoint with no `/v1/models` (a private gateway, most 兼容 setups), the
 *  list has to be typeable — otherwise 拉取 is the only way in and some providers
 *  simply have no door. */
function addModel(): void {
  const row = selected.value;
  const m = newModel.value.trim();
  if (!row || !m) return;
  if (!row.models.includes(m)) row.models.push(m);
  onPickModel(m);
  newModel.value = "";
  addingModel.value = false;
}

/** Ask the provider what it serves. The key never crosses back to this side: the
 *  backend reads it from providers.json, sends one non-redirecting request, and
 *  returns names only. */
async function pullModels(): Promise<void> {
  const row = selected.value;
  if (!row) return;
  const id = effId(row);
  if (!id) return;
  fetching.value = true;
  fetchNote.value = "";
  error.value = "";
  try {
    // The request is served from disk, so an unsaved Base URL or a fresh row would be
    // fetched against stale metadata.
    if (dirty.value) await persist();
    const found = await api.fetchProviderModels(id, "");
    const live = rows.value[selIndex.value];
    if (live) live.models = found;
    fetchNote.value = t("providers.pulled").replace("{n}", String(found.length));
    if (found.length && !found.includes(modelConfig.defaultModel) && isDefault.value)
      modelConfig.defaultModel = found[0];
  } catch (e) {
    fetchNote.value = "";
    error.value = String(e);
  } finally {
    fetching.value = false;
  }
}
/** The dropdown's last item. Probes loopback ports, appends a row per runtime that
 *  answered, and selects the first — so the result lands in the list the user is
 *  already looking at instead of a box of its own. Nothing is saved: the rows are
 *  drafts until 保存, so a probe can never change how anything launches. */
async function probeLocal(): Promise<void> {
  if (probing.value) return;
  probing.value = true;
  // The probe can take the better part of a second, and the dropdown snaps shut the
  // moment it is picked — without this the click looks like it did nothing.
  probeNote.value = t("providers.probing");
  error.value = "";
  try {
    const found = await api.detectLocalLlms();
    if (!found.length) {
      probeNote.value = t("providers.localNone");
      return;
    }
    let first = -1;
    for (const llm of found) {
      const i = adoptLocal(llm);
      if (first < 0) first = i;
    }
    if (first >= 0) selIndex.value = first;
    probeNote.value = t("providers.probed").replace("{n}", String(found.length));
  } catch (e) {
    error.value = String(e);
  } finally {
    probing.value = false;
  }
}

/** Fold one detected runtime into the list, reusing its row if it is already there
 *  (re-probing must not multiply rows). Returns the row's index. */
function adoptLocal(llm: LocalLlm): number {
  const i = rows.value.findIndex((r) => r.id.trim() === llm.id);
  if (i >= 0) {
    const row = rows.value[i];
    row.base_url = llm.base_url;
    if (llm.models.length) row.models = [...llm.models];
    row.enabled = true;
    return i;
  }
  // A loopback runtime wants no credential. What keeps `dispatch` out of its way is
  // having no key at all, not an empty variable name — that one gets derived now.
  return addRow({
    id: llm.id,
    label: llm.label,
    base_url: llm.base_url,
    models: [...llm.models],
  });
}

/** dsh only. Writes one `NAME: value` line into ~/.dsh/.credentials.yaml (0600) —
 *  a different file from providers.json, because dsh reads that one and nothing else. */
async function writeDsh(): Promise<void> {
  const name = activeEnv.value;
  const value = dshDraft.value.trim();
  if (!name || !value) return;
  error.value = "";
  try {
    await api.setCredential(name, value);
    dshDraft.value = "";
    dshStored.value = await api.listCredentialKeys().catch(() => dshStored.value);
    dshFlash.value = true;
    setTimeout(() => (dshFlash.value = false), 1600);
  } catch (e) {
    error.value = String(e);
  }
}
</script>

<template>
  <GroupBox :title="t('providers.title')">
    <p class="text-[12px] text-muted-foreground">{{ t("providers.storeHint") }}</p>

    <p v-if="loading" class="mt-3 text-[13px] text-muted-foreground">{{ t("common.loading") }}</p>

    <template v-else>
      <!-- MASTER — one control wide. 新增 and 探测 are items in this list, not
           buttons beside it; that is what keeps the whole page to a single box. -->
      <div class="mt-3 flex items-center gap-2">
        <label class="shrink-0 text-[14px] text-foreground/85" for="prov-pick">
          {{ t("settings.model.provider") }}
        </label>
        <Select
          id="prov-pick"
          class="min-w-0 flex-1"
          :model-value="String(selIndex)"
          :options="providerOptions"
          @update:model-value="onPickProvider"
        />
        <Button
          v-if="selected && !selected.builtin"
          variant="ghost"
          size="icon"
          :title="t('common.delete')"
          @click="removeRow"
        >
          <Trash2 class="h-4 w-4" />
        </Button>
      </div>
      <p v-if="probeNote" class="mt-1 text-[12px] text-muted-foreground">{{ probeNote }}</p>
      <p class="mt-1 text-[12px] text-muted-foreground">{{ t("providers.defaultHint") }}</p>

      <!-- DETAIL — four fields, which is the whole point: 名称, Base URL, API Key,
           模型列表. Every other column of the record is derived on save; 高级 below
           is where they can still be overridden. -->
      <div
        v-if="selected"
        class="mt-2.5 grid grid-cols-[96px_1fr] items-start gap-x-3 gap-y-2 border-t border-border pt-3"
      >
        <label class="pt-1.5 text-[14px] text-foreground/85">{{ t("providers.name") }}</label>
        <div class="flex min-w-0 flex-col gap-1">
          <Input v-model="selected.label" class="h-7" :placeholder="t('providers.namePlaceholder')" />
          <span v-if="selErr.name" class="text-[12px] text-destructive">{{ selErr.name }}</span>
        </div>

        <label class="pt-1.5 text-[14px] text-foreground/85">Base URL</label>
        <Input
          v-model="selected.base_url"
          class="h-7 min-w-0 font-mono"
          placeholder="https://api.example.com/v1"
        />

        <label class="pt-1.5 text-[14px] text-foreground/85">API Key</label>
        <div class="flex min-w-0 items-center gap-2">
          <!-- The most this side can ever show. There is no plaintext here to reveal,
               so 可见 flips a fingerprint against dots. -->
          <span class="w-24 shrink-0 truncate font-mono text-[12px] text-muted-foreground">
            <template v-if="!primary?.has_value">{{ t("providers.keyEmpty") }}</template>
            <template v-else>{{ shown[primaryAlias] ? primary.fingerprint : "••••••••" }}</template>
          </span>
          <Button
            v-if="primary?.has_value"
            variant="ghost"
            size="icon"
            class="h-6 w-6 shrink-0"
            :title="t('providers.reveal')"
            @click="shown[primaryAlias] = !shown[primaryAlias]"
          >
            <EyeOff v-if="shown[primaryAlias]" class="h-3.5 w-3.5" />
            <Eye v-else class="h-3.5 w-3.5" />
          </Button>
          <Input
            v-model="keyDraft[primaryAlias]"
            type="password"
            class="h-7 min-w-0 flex-1 font-mono"
            :placeholder="primary?.has_value ? t('providers.keyReplace') : 'sk-...'"
            @keydown.enter.prevent="saveKey(primaryAlias)"
          />
          <Button
            variant="outline"
            size="sm"
            class="shrink-0"
            :disabled="!(keyDraft[primaryAlias] ?? '').trim()"
            @click="saveKey(primaryAlias)"
          >
            {{ t("common.save") }}
          </Button>
        </div>

        <label class="pt-1.5 text-[14px] text-foreground/85">{{ t("providers.models") }}</label>
        <div class="flex min-w-0 flex-col gap-1">
          <div class="flex gap-2">
            <Select
              class="h-7 min-w-0 flex-1"
              :model-value="isDefault ? modelConfig.defaultModel : ''"
              :options="modelOptions"
              :placeholder="
                selected.models.length ? t('providers.modelPick') : t('providers.modelsEmpty')
              "
              @update:model-value="onPickModel"
            />
            <Button
              variant="outline"
              size="sm"
              class="shrink-0"
              :title="t('providers.pullHint')"
              :disabled="fetching || !selected.base_url.trim()"
              @click="pullModels"
            >
              <Globe class="h-3.5 w-3.5" />
              {{ fetching ? t("providers.pulling") : t("providers.pull") }}
            </Button>
            <Button
              variant="ghost"
              size="icon"
              class="h-7 w-7 shrink-0"
              :title="t('providers.addModel')"
              @click="addingModel = !addingModel"
            >
              <Plus class="h-4 w-4" />
            </Button>
          </div>
          <div v-if="addingModel" class="flex gap-2">
            <Input
              v-model="newModel"
              class="h-7 min-w-0 flex-1 font-mono"
              :placeholder="t('providers.addModelPlaceholder')"
              @keydown.enter.prevent="addModel"
            />
            <Button variant="outline" size="sm" :disabled="!newModel.trim()" @click="addModel">
              {{ t("providers.addModel") }}
            </Button>
          </div>
          <span v-if="fetchNote" class="text-[12px] text-emerald-400">{{ fetchNote }}</span>
        </div>
      </div>

      <!-- 高级 — the fields the form stopped asking for. Collapsed, because every one
           of them now has a derived answer that is right for a normal provider; open,
           because "derived" must not mean "unreachable". -->
      <button
        v-if="selected"
        type="button"
        class="mt-2 inline-flex items-center gap-1 text-[12px] text-muted-foreground hover:text-foreground"
        @click="advanced = !advanced"
      >
        <ChevronDown v-if="advanced" class="h-3 w-3" />
        <ChevronRight v-else class="h-3 w-3" />
        {{ t("providers.advanced") }}
      </button>
      <div
        v-if="advanced && selected"
        class="mt-2 grid grid-cols-[96px_1fr] items-start gap-x-3 gap-y-2 border-t border-border pt-3"
      >
        <label class="pt-0.5 text-[13px] text-foreground/85">{{ t("providers.enabled") }}</label>
        <div class="flex items-center gap-4">
          <input v-model="selected.enabled" type="checkbox" class="h-3.5 w-3.5 accent-selection" />
          <Select
            v-model="selected.auth_style"
            class="h-7 w-40"
            :options="authOptions"
            :title="t('providers.auth')"
          />
        </div>

        <label class="pt-1.5 text-[13px] text-foreground/85">{{ t("providers.envVars") }}</label>
        <div class="flex min-w-0 flex-col gap-1">
          <div class="flex gap-2">
            <Input
              v-model="selected.api_key_env"
              class="h-7 min-w-0 flex-1 font-mono"
              :placeholder="derivedEnv.key"
            />
            <Input
              v-model="selected.base_url_env"
              class="h-7 min-w-0 flex-1 font-mono"
              :placeholder="derivedEnv.base"
            />
          </div>
          <span v-if="selErr.env" class="text-[12px] text-destructive">{{ selErr.env }}</span>
          <span v-else class="text-[12px] text-muted-foreground">{{ t("providers.envDerived") }}</span>
        </div>
        <!-- Every key, not just the first. This is the rotation store: several keys
             under one provider are walked in turn at launch. -->
        <label class="pt-1.5 text-[13px] text-foreground/85">{{ t("providers.keys") }}</label>
        <div class="flex min-w-0 flex-col gap-1.5">
          <div v-for="k in selected.keys" :key="k.alias" class="flex items-center gap-2">
            <input
              v-model="k.enabled"
              type="checkbox"
              class="h-3.5 w-3.5 shrink-0 accent-selection"
              :title="t('providers.keyEnabled')"
            />
            <span class="w-20 shrink-0 truncate font-mono text-[13px]">{{ k.alias }}</span>
            <span class="w-24 shrink-0 truncate font-mono text-[12px] text-muted-foreground">
              <template v-if="!k.has_value">{{ t("providers.keyEmpty") }}</template>
              <template v-else>{{ shown[k.alias] ? k.fingerprint : "••••••••" }}</template>
            </span>
            <Input
              v-model="keyDraft[k.alias]"
              type="password"
              class="h-7 min-w-0 flex-1 font-mono"
              :placeholder="t('providers.keyReplace')"
            />
            <Button
              variant="outline"
              size="sm"
              class="shrink-0"
              :disabled="!(keyDraft[k.alias] ?? '').trim()"
              @click="saveKey(k.alias)"
            >
              {{ t("common.save") }}
            </Button>
            <Button
              variant="ghost"
              size="icon"
              class="h-6 w-6 shrink-0"
              :title="t('common.delete')"
              @click="removeKey(k.alias)"
            >
              <Trash2 class="h-3.5 w-3.5" />
            </Button>
          </div>
          <div class="flex gap-2">
            <Input
              v-model="keyAlias"
              class="h-7 w-36 font-mono"
              :placeholder="t('providers.aliasPlaceholder')"
              @keydown.enter.prevent="addKey"
            />
            <Button variant="outline" size="sm" :disabled="!keyAlias.trim()" @click="addKey">
              <Plus class="h-3.5 w-3.5" />
              {{ t("providers.addKey") }}
            </Button>
          </div>
          <span class="text-[12px] text-muted-foreground">{{ t("providers.rotateHint") }}</span>
        </div>
        <!-- dsh's credential file: a second store, not a duplicate — dsh resolves
             credential *names* out of ~/.dsh/.credentials.yaml, which holds one value
             per name and so cannot express the rotation above. -->
        <label class="pt-1.5 text-[13px] text-foreground/85">{{ t("providers.dshLabel") }}</label>
        <div class="flex min-w-0 flex-col gap-1">
          <div class="flex gap-2">
            <Input
              v-model="dshDraft"
              type="password"
              class="h-7 min-w-0 flex-1 font-mono"
              :placeholder="dshHasIt ? t('settings.dsh.savedPlaceholder') : 'sk-...'"
            />
            <Button
              variant="outline"
              size="sm"
              :title="t('settings.dsh.desc')"
              :disabled="!dshDraft.trim() || !activeEnv"
              @click="writeDsh"
            >
              {{ t("providers.dshWrite") }}
            </Button>
          </div>
          <span
            class="inline-flex items-center gap-1 text-[12px]"
            :class="dshHasIt ? 'text-emerald-400' : 'text-muted-foreground'"
          >
            <Check v-if="dshHasIt" class="h-3 w-3 shrink-0" />
            <template v-if="dshHasIt">{{ activeEnv }} {{ t("settings.dsh.stored") }}</template>
            <template v-else>{{ t("settings.dsh.willWrite") }} {{ activeEnv }}</template>
          </span>
        </div>


      </div>
      <div class="mt-3 flex flex-wrap items-center gap-3">
        <Button variant="primary" :disabled="!dirty || !valid || saving" @click="persist">
          {{ saving ? t("common.saving") : t("common.save") }}
        </Button>
        <span v-if="savedFlash" class="flex items-center gap-1 text-[13px] text-emerald-400">
          <Check class="h-3.5 w-3.5" /> {{ t("common.saved") }}
        </span>
        <span v-if="dshFlash" class="text-[13px] text-emerald-400">
          {{ t("providers.dshWrote") }}
        </span>
        <span v-if="otherInvalid.length" class="text-[13px] text-destructive">
          {{ t("providers.invalidRows").replace("{names}", otherInvalid.join("、")) }}
        </span>
        <span v-if="error" class="text-[13px] text-destructive">{{ error }}</span>
      </div>
    </template>
  </GroupBox>
</template>
