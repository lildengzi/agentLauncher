<script setup lang="ts">
import AppIcon from "@/components/ui/AppIcon.vue";
import { ChevronDown } from "lucide-vue-next";

withDefaults(
  defineProps<{
    icon?: string;
    label: string;
    accel?: string;
    split?: boolean;
    disabled?: boolean;
  }>(),
  { split: false, disabled: false }
);

defineEmits<{ click: []; arrow: [] }>();
</script>

<template>
  <div class="flex items-stretch">
    <button
      type="button"
      :disabled="disabled"
      class="group flex items-center gap-1.5 px-2.5 py-1.5 text-[13px] text-foreground/90 hover:bg-accent hover:text-accent-foreground disabled:pointer-events-none disabled:opacity-40 transition-colors"
      @click="$emit('click')"
    >
      <AppIcon v-if="icon" :name="icon" class="h-4 w-4 shrink-0 opacity-90" />
      <span>{{ label }}<span v-if="accel" class="opacity-70"> (<span class="mnemonic">{{ accel }}</span>)</span></span>
    </button>
    <button
      v-if="split"
      type="button"
      :disabled="disabled"
      class="flex items-center px-1 hover:bg-accent hover:text-accent-foreground border-l border-border/60 disabled:pointer-events-none disabled:opacity-40 transition-colors"
      @click="$emit('arrow')"
    >
      <ChevronDown class="h-3.5 w-3.5 opacity-70" />
    </button>
  </div>
</template>
