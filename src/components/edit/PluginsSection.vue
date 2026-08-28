<script setup lang="ts">
// SEAM PLACEHOLDER — owned by Stream B (编辑页承接插件).
//
// Shared contract for all three extension sections, because
// EditInstanceDialog.vue does the one `api.readInstanceExtensions` call and
// passes the result down:
//   :instance-id  — "" while creating an unsaved instance ⇒ show `ext.saveFirst`
//   :extensions   — the whole InstanceExtensions, or null while loading/failed
//   :loading      — a read is in flight
//   @changed      — this section wrote something; parent re-reads
//   @browse       — open the market dialog for this section's kind
//
// Stream B implements: the installed-plugin list, and the honest scope notice —
// plugins are dsh-**profile**-scoped, so `plugin_scope` must be surfaced rather
// than letting the list read as per-instance ownership.
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
  <GroupBox :title="t('ext.plugins.title')">
    <p class="text-[13px] text-muted-foreground">{{ t('ext.plugins.desc') }}</p>
    <p class="mt-3 text-[13px] text-muted-foreground">
      {{ loading ? t('ext.loading') : t('ext.plugins.empty') }}
    </p>
  </GroupBox>
</template>
