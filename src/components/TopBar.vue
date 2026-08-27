<script setup lang="ts">
import ToolbarButton from "@/components/ui/ToolbarButton.vue";
import { UserRound } from "lucide-vue-next";
import { useI18n } from "@/lib/i18n";

const { t } = useI18n();
defineEmits<{ add: []; folder: []; settings: []; help: [] }>();
</script>

<template>
  <header class="flex items-stretch border-b border-border bg-toolbar">
    <!-- Qt-style drag grip -->
    <div class="flex w-3 items-center justify-center border-r border-border/60">
      <div class="flex flex-col gap-[3px]">
        <span class="h-[3px] w-[3px] rounded-full bg-border-strong" />
        <span class="h-[3px] w-[3px] rounded-full bg-border-strong" />
        <span class="h-[3px] w-[3px] rounded-full bg-border-strong" />
      </div>
    </div>

    <ToolbarButton icon="plus-square" :label="t('top.add')" accel="E" @click="$emit('add')" />
    <ToolbarButton icon="folder" :label="t('top.folder')" accel="O" split @click="$emit('folder')" @arrow="$emit('folder')" />
    <ToolbarButton icon="settings" :label="t('top.settings')" accel="N" @click="$emit('settings')" />
    <ToolbarButton icon="circle-help" :label="t('top.help')" split @click="$emit('help')" @arrow="$emit('help')" />

    <div class="flex-1" />

    <button
      type="button"
      class="flex items-center gap-2 border-l border-border/60 px-3 text-[13px] text-foreground/90 hover:bg-accent transition-colors"
    >
      <span class="flex h-5 w-5 items-center justify-center rounded-sm bg-selection/25 text-link">
        <UserRound class="h-3.5 w-3.5" />
      </span>
      <span class="font-medium">Lil_DengZi</span>
    </button>
  </header>
</template>
