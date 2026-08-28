<script setup lang="ts">
// Prism Launcher's two-row footer, structure intact: the launcher line with the
// news link, then the context line for the selected instance. Only the paint
// changed — and 更多信息 now actually goes somewhere.
import PrismMark from "@/components/ui/PrismMark.vue";
import { Newspaper } from "lucide-vue-next";
import { useI18n } from "@/lib/i18n";

const { t } = useI18n();
defineProps<{
  /** the launcher's own version — engines are per-instance, so no engine version here. */
  appVersion: string;
  runningCount: number;
  contextLine: string;
}>();
defineEmits<{ more: [] }>();
</script>

<template>
  <footer class="shrink-0 border-t border-border bg-toolbar text-[12px] text-muted-foreground">
    <div class="flex h-[22px] items-center gap-2 px-2.5">
      <PrismMark :size="11" class="shrink-0 text-foreground/50" />
      <span class="text-foreground/75">agentLauncher</span>
      <span class="font-mono text-muted-foreground/70">{{ appVersion }}</span>
      <div class="flex-1" />
      <span v-if="runningCount > 0" class="font-mono text-foreground/75">
        {{ t('status.runningN') }} {{ runningCount }}
      </span>
      <button
        type="button"
        class="flex items-center gap-1 hover:text-foreground"
        @click="$emit('more')"
      >
        <Newspaper class="h-3 w-3" />
        {{ t('status.more') }}
      </button>
    </div>
    <div
      class="truncate border-t border-border/60 bg-background px-2.5 py-1 font-mono text-muted-foreground/80"
    >
      {{ contextLine }}
    </div>
  </footer>
</template>
