<script setup lang="ts">
import { Boxes, Newspaper } from "lucide-vue-next";
import { useI18n } from "@/lib/i18n";

const { t } = useI18n();
defineProps<{
  engineVersion: string;
  runningCount: number;
  defaultModel: string;
  contextLine?: string;
}>();
</script>

<template>
  <footer class="border-t border-border bg-toolbar text-[12px] text-muted-foreground">
    <div class="flex items-center gap-2 px-3 py-1">
      <Boxes class="h-3.5 w-3.5 opacity-80" />
      <span class="font-medium text-foreground/80">dsh Launcher</span>
      <span class="opacity-70">dsh Core Engine {{ engineVersion }}</span>
      <div class="flex-1" />
      <span v-if="runningCount > 0" class="text-link">{{ t('status.runningN') }} {{ runningCount }}</span>
      <button class="flex items-center gap-1 hover:text-foreground">
        <Newspaper class="h-3.5 w-3.5" />
        {{ t('status.more') }}
      </button>
    </div>
    <div class="border-t border-border/60 bg-background px-3 py-1 text-[11px]">
      {{ contextLine ?? `${t('status.defaultModel')}：${defaultModel}` }}
    </div>
  </footer>
</template>
