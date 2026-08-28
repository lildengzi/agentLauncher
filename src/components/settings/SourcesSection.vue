<script setup lang="ts">
// SEAM PLACEHOLDER — owned by Stream C (去中心化数据源).
//
// The Settings 数据源 section. SettingsDialog.vue mounts this with no props, so
// the section owns its own load/save cycle:
//   api.getMarketSources / api.setMarketSources — the ~/.agentlauncher/sources.json
//   api.marketRefresh(id)                       — per-row "test / refresh" button
//
// Stream C implements: the row editor (id / label / kind / url / adapter / kinds /
// enabled), add + delete, and the per-row status line from SourceStatus. Built-in
// rows are disable-able but not delete-able — the backend re-seeds them anyway, so
// the UI must not offer a delete that silently comes back.
import GroupBox from "@/components/ui/GroupBox.vue";
import { useI18n } from "@/lib/i18n";

const { t } = useI18n();
</script>

<template>
  <GroupBox :title="t('sources.title')">
    <p class="text-[13px] text-muted-foreground">{{ t('sources.desc') }}</p>
    <p class="mt-3 text-[13px] text-muted-foreground">{{ t('common.loading') }}</p>
  </GroupBox>
</template>
