<script setup lang="ts">
import AppIcon from "@/components/ui/AppIcon.vue";
import { ChevronDown } from "lucide-vue-next";

withDefaults(
  defineProps<{
    icon: string;
    label: string;
    accel?: string;
    split?: boolean;
    disabled?: boolean;
    /** slightly emphasized row (used for the primary Launch action). */
    emphasized?: boolean;
  }>(),
  { split: false, disabled: false, emphasized: false }
);

defineEmits<{ click: []; arrow: [] }>();
</script>

<template>
  <div class="flex items-stretch">
    <button
      type="button"
      :disabled="disabled"
      class="flex flex-1 items-center gap-2.5 px-3 text-left text-[13px] hover:bg-accent hover:text-accent-foreground disabled:pointer-events-none disabled:opacity-40 transition-colors"
      :class="emphasized ? 'py-2 font-medium text-foreground' : 'py-1.5 text-foreground/90'"
      @click="$emit('click')"
    >
      <AppIcon :name="icon" class="h-4 w-4 shrink-0 opacity-90" />
      <span>{{ label }}<span v-if="accel" class="opacity-70"> (<span class="mnemonic">{{ accel }}</span>)</span></span>
    </button>
    <button
      v-if="split"
      type="button"
      :disabled="disabled"
      class="flex items-center px-1.5 hover:bg-accent hover:text-accent-foreground disabled:pointer-events-none disabled:opacity-40 transition-colors"
      @click="$emit('arrow')"
    >
      <ChevronDown class="h-3.5 w-3.5 opacity-70" />
    </button>
  </div>
</template>
