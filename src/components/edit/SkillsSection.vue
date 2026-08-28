<script setup lang="ts">
// Owned by Stream B (编辑页承接插件).
// Same props/emits contract as PluginsSection.vue (see its header).
//
// Skills are the one genuinely per-instance kind: one directory each under
// `instances/<id>/skills/`, so this section can both list and delete for real.
// `description` is the first prose line of the skill's own SKILL.md/README.md and
// is legitimately "" for a skill that ships no doc — that row shows a name and
// nothing else rather than a stand-in sentence the skill never wrote.
import { computed, ref } from "vue";
import { Store, RefreshCw, FolderOpen, Trash2 } from "lucide-vue-next";
import ExtSection from "@/components/edit/ExtSection.vue";
import Button from "@/components/ui/Button.vue";
import { api } from "@/lib/api";
import { useI18n } from "@/lib/i18n";
import type { ExtensionKind, InstanceExtensions } from "@/types";

const { t } = useI18n();
const props = defineProps<{
  instanceId: string;
  extensions: InstanceExtensions | null;
  loading: boolean;
}>();
const emit = defineEmits<{ changed: []; browse: [kind: ExtensionKind] }>();

const skills = computed(() => props.extensions?.skills ?? []);
const failed = computed(() => !props.loading && props.extensions === null);
const error = ref("");
const busy = ref("");

async function remove(name: string): Promise<void> {
  // Native confirm, as App.vue's instance delete does — this removes a directory
  // and everything under it, which no undo in the launcher can walk back.
  if (!window.confirm(t("ext.skills.confirmRemove"))) return;
  busy.value = name;
  error.value = "";
  try {
    await api.removeInstanceSkill(props.instanceId, name);
    emit("changed");
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    busy.value = "";
  }
}

async function openFolder(): Promise<void> {
  error.value = "";
  try {
    // The backend creates the directory if it is missing, so this works on an
    // instance that has no skills yet — which is when a user most wants it open.
    await api.openInstanceSubdir(props.instanceId, "skills");
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  }
}
</script>

<template>
  <ExtSection
    :title="t('ext.skills.title')"
    :desc="t('ext.skills.desc')"
    :loading="loading"
    :failed="failed"
    :empty="skills.length === 0"
    :empty-label="t('ext.skills.empty')"
    :error="error"
    @retry="emit('changed')"
  >
    <template #actions>
      <Button variant="outline" size="sm" @click="emit('browse', 'skill')">
        <Store class="h-3.5 w-3.5" />
        {{ t("ext.browse") }}
      </Button>
      <Button variant="outline" size="sm" @click="openFolder">
        <FolderOpen class="h-3.5 w-3.5" />
        {{ t("ext.skills.openFolder") }}
      </Button>
      <Button variant="ghost" size="sm" :title="t('ext.refresh')" @click="emit('changed')">
        <RefreshCw class="h-3.5 w-3.5" />
      </Button>
    </template>

    <ul class="mt-3 divide-y divide-border rounded border border-border">
      <li
        v-for="skill in skills"
        :key="skill.name"
        class="flex items-start gap-3 px-3 py-2"
        :title="skill.path"
      >
        <div class="min-w-0 flex-1">
          <p class="truncate text-[14px]">{{ skill.name }}</p>
          <!-- No doc, no summary. -->
          <p v-if="skill.description" class="mt-0.5 text-[13px] text-muted-foreground">
            {{ skill.description }}
          </p>
        </div>
        <Button
          variant="ghost"
          size="sm"
          :title="t('common.delete')"
          :disabled="busy !== ''"
          @click="remove(skill.name)"
        >
          <Trash2 class="h-3.5 w-3.5" />
        </Button>
      </li>
    </ul>
  </ExtSection>
</template>
