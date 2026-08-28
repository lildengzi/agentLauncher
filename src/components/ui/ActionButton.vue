<script setup lang="ts">
// One row in the right-hand action dock: icon · label · right-aligned keycap.
// Two intents —
//   default     quiet row, hover lifts the surface;
//   danger      destructive text on hover only, so it never shouts at rest.
// The reference gives 启动 no special paint, so neither do we.
import AppIcon from "@/components/ui/AppIcon.vue";
import Kbd from "@/components/ui/Kbd.vue";
import { ChevronDown } from "lucide-vue-next";

withDefaults(
  defineProps<{
    icon: string;
    label: string;
    accel?: string;
    split?: boolean;
    disabled?: boolean;
    danger?: boolean;
  }>(),
  { split: false, disabled: false, danger: false }
);

defineEmits<{ click: []; arrow: [] }>();
</script>

<template>
  <div class="flex items-stretch">
    <button
      type="button"
      :disabled="disabled"
      class="group flex h-[27px] flex-1 items-center gap-2 border-l-2 border-l-transparent pl-2.5 pr-2 text-left text-[13.5px] text-foreground/85 transition-colors duration-75 hover:border-l-border-strong hover:bg-accent hover:text-accent-foreground disabled:pointer-events-none disabled:opacity-35"
      :class="danger ? 'hover:!border-l-destructive/60 hover:!text-destructive' : ''"
      @click="$emit('click')"
    >
      <AppIcon
        :name="icon"
        class="h-[14px] w-[14px] shrink-0 text-muted-foreground group-hover:text-current"
      />
      <span class="flex-1 truncate">{{ label }}</span>
      <Kbd v-if="accel" :keys="accel" />
    </button>
    <button
      v-if="split"
      type="button"
      :disabled="disabled"
      class="flex items-center px-1.5 text-muted-foreground transition-colors duration-75 hover:bg-accent hover:text-accent-foreground disabled:pointer-events-none disabled:opacity-35"
      @click="$emit('arrow')"
    >
      <ChevronDown class="h-3 w-3" />
    </button>
  </div>
</template>
