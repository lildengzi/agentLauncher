<script setup lang="ts">
// Owned by Stream B (编辑页承接插件).
// Same props/emits contract as PluginsSection.vue (see its header).
//
// The one real editor of the three. Two things shape it:
//
//   `api.setInstanceMcp` replaces the whole `mcpServers` map, so this sends every
//   row on every save and a deleted row is simply absent from the payload. There
//   is no per-entry command to patch with, and inventing a merge policy here
//   would let the file and this dialog drift apart.
//
//   The backend refuses a blank name, a duplicate name and a blank command. The
//   same three checks run here so the user reads the reason next to the field
//   instead of a raw command failure after the fact.
//
// Secrets do not belong in `mcp.json`: it is a plain config file with no
// protection, sitting in the instance directory. `ext.mcp.envHint` says so beside
// the env editor and must stay there; nothing in this component surfaces an env
// *value* anywhere but that editor.
import { computed, ref, watch } from "vue";
import { Store, RefreshCw, Plus, Trash2, Check } from "lucide-vue-next";
import ExtSection from "@/components/edit/ExtSection.vue";
import Button from "@/components/ui/Button.vue";
import Input from "@/components/ui/Input.vue";
import Label from "@/components/ui/Label.vue";
import Textarea from "@/components/ui/Textarea.vue";
import { api } from "@/lib/api";
import { useI18n } from "@/lib/i18n";
import type { ExtensionKind, InstanceExtensions, McpServerEntry } from "@/types";

const { t } = useI18n();
const props = defineProps<{
  instanceId: string;
  extensions: InstanceExtensions | null;
  loading: boolean;
}>();
const emit = defineEmits<{ changed: []; browse: [kind: ExtensionKind] }>();

/** One row being edited. `args`/`env` are the textarea text, not the parsed
 *  values: parsing on every keystroke would fight the user mid-line. `key` is a
 *  local identity for `v-for`, because `name` may be blank or duplicated while
 *  being typed and so cannot be the key. */
interface Draft {
  key: number;
  name: string;
  command: string;
  args: string;
  env: string;
  enabled: boolean;
}

let nextKey = 0;
const rows = ref<Draft[]>([]);
const baseline = ref("");
const saving = ref(false);
const savedFlash = ref(false);
const error = ref("");

const failed = computed(() => !props.loading && props.extensions === null);

function toDraft(s: McpServerEntry): Draft {
  return {
    key: nextKey++,
    name: s.name,
    command: s.command,
    args: s.args.join("\n"),
    env: Object.entries(s.env)
      .map(([k, v]) => `${k}=${v}`)
      .join("\n"),
    // The file stores `disabled`; the UI asks the positive question.
    enabled: !s.disabled,
  };
}

/** Serialised draft, for telling "the user has edited this" from "this is what
 *  disk said". Cheaper and more honest than a flag set from every input. */
function snapshot(list: Draft[]): string {
  return JSON.stringify(list.map((r) => [r.name, r.command, r.args, r.env, r.enabled]));
}
const dirty = computed(() => snapshot(rows.value) !== baseline.value);

function ingest(list: McpServerEntry[] | undefined): void {
  rows.value = (list ?? []).map(toDraft);
  baseline.value = snapshot(rows.value);
}

// The parent re-reads on its own schedule (dialog open, profile switch, a market
// install), so re-ingesting unconditionally would throw away half-typed rows.
// While the draft is dirty it wins; the next save replaces the whole map anyway.
watch(() => props.extensions?.mcp, ingest, { immediate: true });

// ---- validation (mirrors instance_ext.rs::write_mcp) -----------------------

const errors = computed(() => {
  const seen = new Map<string, number>();
  for (const r of rows.value) {
    const name = r.name.trim();
    if (name) seen.set(name, (seen.get(name) ?? 0) + 1);
  }
  return rows.value.map((r) => {
    const name = r.name.trim();
    return {
      name: !name
        ? t("ext.mcp.nameRequired")
        : (seen.get(name) ?? 0) > 1
          ? t("ext.mcp.duplicateName")
          : "",
      command: r.command.trim() ? "" : t("ext.mcp.commandRequired"),
    };
  });
});
const valid = computed(() => errors.value.every((e) => !e.name && !e.command));

// ---- text ⇄ value ---------------------------------------------------------

/** One argument per line, as `ext.mcp.argsHint` promises — not shell-split, so a
 *  path with a space survives being one argument. */
function parseArgs(text: string): string[] {
  return text
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line !== "");
}

/** One `KEY=VALUE` per line. A line with no `=` becomes a name with an empty
 *  value rather than being dropped: that is how a user notes which variables a
 *  server needs while leaving the values in the instance `.env`, where they
 *  belong. */
function parseEnv(text: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const raw of text.split("\n")) {
    const line = raw.trim();
    if (!line) continue;
    const eq = line.indexOf("=");
    const key = (eq === -1 ? line : line.slice(0, eq)).trim();
    if (!key) continue;
    out[key] = eq === -1 ? "" : line.slice(eq + 1).trim();
  }
  return out;
}

// ---- actions --------------------------------------------------------------

function addRow(): void {
  rows.value.push({ key: nextKey++, name: "", command: "", args: "", env: "", enabled: true });
}

function removeRow(index: number): void {
  rows.value.splice(index, 1);
}

async function save(): Promise<void> {
  error.value = "";
  savedFlash.value = false;
  if (!valid.value) return;
  const servers: McpServerEntry[] = rows.value.map((r) => ({
    name: r.name.trim(),
    command: r.command.trim(),
    args: parseArgs(r.args),
    env: parseEnv(r.env),
    disabled: !r.enabled,
  }));
  saving.value = true;
  try {
    await api.setInstanceMcp(props.instanceId, servers);
    // Clean again, so the parent's re-read is free to replace the draft with the
    // normalised on-disk form (trimmed, name-sorted) instead of being ignored.
    baseline.value = snapshot(rows.value);
    savedFlash.value = true;
    setTimeout(() => (savedFlash.value = false), 1500);
    emit("changed");
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    saving.value = false;
  }
}

// The label/field grid EditInstanceDialog uses for its own rows; the extension
// sections are pages of the same dialog and should read as such.
const rowGrid = "grid grid-cols-[120px_1fr] items-start gap-x-3 gap-y-3 [&>label]:pt-2";
</script>

<template>
  <ExtSection
    :title="t('ext.mcp.title')"
    :desc="t('ext.mcp.desc')"
    :loading="loading"
    :failed="failed"
    :empty="rows.length === 0"
    :empty-label="t('ext.mcp.empty')"
    :error="error"
    @retry="emit('changed')"
  >
    <template #actions>
      <Button variant="outline" size="sm" @click="emit('browse', 'mcp')">
        <Store class="h-3.5 w-3.5" />
        {{ t("ext.browse") }}
      </Button>
      <Button variant="outline" size="sm" @click="addRow">
        <Plus class="h-3.5 w-3.5" />
        {{ t("ext.mcp.add") }}
      </Button>
      <Button variant="ghost" size="sm" :title="t('ext.refresh')" @click="emit('changed')">
        <RefreshCw class="h-3.5 w-3.5" />
      </Button>
    </template>

    <div class="mt-3 grid gap-3">
      <div
        v-for="(row, i) in rows"
        :key="row.key"
        class="rounded border border-border bg-muted/20 px-3 py-3"
      >
        <div class="mb-3 flex items-center justify-between gap-3">
          <label class="flex items-center gap-2 text-[13px] text-foreground/90">
            <input
              type="checkbox"
              v-model="row.enabled"
              class="h-3.5 w-3.5 accent-selection"
            />
            {{ t("ext.mcp.enabled") }}
          </label>
          <Button variant="ghost" size="sm" :title="t('common.remove')" @click="removeRow(i)">
            <Trash2 class="h-3.5 w-3.5" />
          </Button>
        </div>

        <div :class="rowGrid">
          <Label :for="`mcp-name-${row.key}`">{{ t("ext.mcp.name") }}</Label>
          <div>
            <Input :id="`mcp-name-${row.key}`" v-model="row.name" />
            <p v-if="errors[i].name" class="mt-1 text-[13px] text-destructive">
              {{ errors[i].name }}
            </p>
          </div>

          <Label :for="`mcp-cmd-${row.key}`">{{ t("ext.mcp.command") }}</Label>
          <div>
            <Input :id="`mcp-cmd-${row.key}`" v-model="row.command" class="font-mono" />
            <p v-if="errors[i].command" class="mt-1 text-[13px] text-destructive">
              {{ errors[i].command }}
            </p>
          </div>

          <Label :for="`mcp-args-${row.key}`">{{ t("ext.mcp.args") }}</Label>
          <div>
            <Textarea
              :id="`mcp-args-${row.key}`"
              v-model="row.args"
              class="min-h-[64px] font-mono"
            />
            <p class="mt-1 text-[13px] text-muted-foreground">{{ t("ext.mcp.argsHint") }}</p>
          </div>

          <Label :for="`mcp-env-${row.key}`">{{ t("ext.mcp.env") }}</Label>
          <div>
            <Textarea
              :id="`mcp-env-${row.key}`"
              v-model="row.env"
              class="min-h-[64px] font-mono"
            />
            <!-- Stays put: this is the only place that tells the user mcp.json is
                 not a credential store, and it has to be where the temptation is. -->
            <p class="mt-1 text-[13px] text-muted-foreground">{{ t("ext.mcp.envHint") }}</p>
          </div>
        </div>
      </div>
    </div>

    <!-- Outside the list, because deleting the last row is itself a change worth
         saving — an empty draft still needs a way to reach disk. -->
    <template #footer>
      <div class="mt-3 flex items-center gap-3">
        <Button variant="primary" size="sm" :disabled="saving || !dirty || !valid" @click="save">
          {{ t("common.save") }}
        </Button>
        <span v-if="savedFlash" class="flex items-center gap-1 text-[13px] text-muted-foreground">
          <Check class="h-3.5 w-3.5" />
          {{ t("common.saved") }}
        </span>
      </div>
    </template>
  </ExtSection>
</template>

