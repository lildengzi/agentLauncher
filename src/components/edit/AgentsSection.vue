<script setup lang="ts">
// The 备注 → 人设与契约 page: an editor for the instance's own AGENTS.md.
//
// It loads and saves itself instead of riding on `readInstanceExtensions` like
// the other three sections. Two reasons, both about not losing typing: that call
// blocks on a dsh plugin probe this page has no use for, and the parent re-issues
// it whenever the engine or profile picker changes — which would replace a
// half-written prompt with whatever is on disk.
//
// AGENTS.md is prose, not config, so nothing here reformats it. What the user
// typed is what lands on disk, byte for byte.
import { computed, ref, watch } from "vue";
import { RefreshCw, Check, Undo2 } from "lucide-vue-next";
import ExtSection from "@/components/edit/ExtSection.vue";
import Button from "@/components/ui/Button.vue";
import Textarea from "@/components/ui/Textarea.vue";
import { api } from "@/lib/api";
import { useI18n } from "@/lib/i18n";

const { t } = useI18n();
const props = defineProps<{ instanceId: string }>();

const text = ref("");
const baseline = ref("");
/** false = the instance has no AGENTS.md yet, so saving will create one. Distinct
 *  from an empty file, which is a deliberate state. */
const exists = ref(true);
const loading = ref(false);
const failed = ref(false);
const saving = ref(false);
const savedFlash = ref(false);
const error = ref("");

const dirty = computed(() => text.value !== baseline.value);

async function load(): Promise<void> {
  if (!props.instanceId) return;
  loading.value = true;
  failed.value = false;
  error.value = "";
  try {
    const doc = await api.readInstanceAgents(props.instanceId);
    text.value = doc.text;
    baseline.value = doc.text;
    exists.value = doc.exists;
  } catch (e) {
    // A read that failed is not an empty prompt: leave the editor untrusted
    // rather than inviting the user to save a blank file over a real one.
    failed.value = true;
    console.error("read AGENTS.md failed", e);
  } finally {
    loading.value = false;
  }
}
watch(() => props.instanceId, load, { immediate: true });

async function save(): Promise<void> {
  error.value = "";
  savedFlash.value = false;
  saving.value = true;
  try {
    await api.writeInstanceAgents(props.instanceId, text.value);
    baseline.value = text.value;
    exists.value = true;
    savedFlash.value = true;
    setTimeout(() => (savedFlash.value = false), 1500);
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    saving.value = false;
  }
}

function revert(): void {
  text.value = baseline.value;
}
</script>

<template>
  <!-- `empty` is always false: an empty AGENTS.md is still a buffer to type into,
       not an empty list to report. The rest of the shell — loading, read failure
       with retry, write error — is exactly what the other sections need. -->
  <ExtSection
    :title="t('ext.agents.title')"
    :desc="t('ext.agents.desc')"
    :loading="loading"
    :failed="failed"
    :empty="false"
    empty-label=""
    :error="error"
    @retry="load"
  >
    <template #actions>
      <Button variant="ghost" size="sm" :title="t('ext.refresh')" @click="load">
        <RefreshCw class="h-3.5 w-3.5" />
      </Button>
    </template>

    <template #notice>
      <p v-if="!exists" class="mt-3 text-[13px] text-muted-foreground">
        {{ t("ext.agents.absent") }}
      </p>
    </template>

    <Textarea
      v-model="text"
      class="mt-3 min-h-[280px] font-mono text-[13px] leading-relaxed"
      :placeholder="t('ext.agents.placeholder')"
      spellcheck="false"
    />
    <p class="mt-1 text-[13px] text-muted-foreground">{{ t("ext.agents.hint") }}</p>

    <template #footer>
      <div class="mt-3 flex items-center gap-3">
        <Button variant="primary" size="sm" :disabled="saving || !dirty" @click="save">
          {{ t("common.save") }}
        </Button>
        <Button variant="ghost" size="sm" :disabled="saving || !dirty" @click="revert">
          <Undo2 class="h-3.5 w-3.5" />
          {{ t("ext.agents.revert") }}
        </Button>
        <span v-if="savedFlash" class="flex items-center gap-1 text-[13px] text-muted-foreground">
          <Check class="h-3.5 w-3.5" />
          {{ t("common.saved") }}
        </span>
      </div>
    </template>
  </ExtSection>
</template>
