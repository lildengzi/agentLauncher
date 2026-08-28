<script setup lang="ts">
// SEAM PLACEHOLDER — owned by Stream B (编辑页承接插件).
// Same props/emits contract as PluginsSection.vue (see its header).
//
// Stream B implements: the editable server list (name / command / args / env /
// enabled) persisted with `api.setInstanceMcp`, which replaces the whole
// `mcpServers` map — so the editor must send every row, not a delta. Validation
// mirrors the backend's: no blank names, no duplicates, no blank command.
// Secrets do not belong in this file; `ext.mcp.envHint` says so and must stay.
import GroupBox from "@/components/ui/GroupBox.vue";
import { useI18n } from "@/lib/i18n";
import type { ExtensionKind, InstanceExtensions } from "@/types";

const { t } = useI18n();
defineProps<{
  instanceId: string;
  extensions: InstanceExtensions | null;
  loading: boolean;
}>();
defineEmits<{ changed: []; browse: [kind: ExtensionKind] }>();
</script>

<template>
  <GroupBox :title="t('ext.mcp.title')">
    <p class="text-[13px] text-muted-foreground">{{ t('ext.mcp.desc') }}</p>
    <p class="mt-3 text-[13px] text-muted-foreground">
      {{ loading ? t('ext.loading') : t('ext.mcp.empty') }}
    </p>
  </GroupBox>
</template>
