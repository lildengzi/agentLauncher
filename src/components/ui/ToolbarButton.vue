<script setup lang="ts">
// A top-bar segment. Flat, 34px tall to match the reference toolbar; the
// accelerator is a Qt-style mnemonic printed in the label's own colour, not a
// dimmed hint — in Prism Launcher `添加实例 (E)` reads as one bright string.
// One button, one action: no split caret, because a caret with no menu behind it
// reads as a dropdown that never opens, and its divider reads as a toolbar
// separator. Hover is the only thing that delineates segments.
import AppIcon from "@/components/ui/AppIcon.vue";

withDefaults(
  defineProps<{
    icon?: string;
    label: string;
    accel?: string;
    disabled?: boolean;
    /** the segment reads as toggled-on (a filter / mode is active). */
    active?: boolean;
  }>(),
  { disabled: false, active: false }
);

defineEmits<{ click: [] }>();
</script>

<template>
  <button
    type="button"
    :disabled="disabled"
    :title="label"
    class="flex h-[34px] shrink-0 items-center gap-2 px-3 text-[13.5px] transition-colors duration-75 hover:bg-accent hover:text-accent-foreground disabled:pointer-events-none disabled:opacity-35"
    :class="active ? 'bg-accent text-accent-foreground' : 'text-foreground'"
    @click="$emit('click')"
  >
    <AppIcon v-if="icon" :name="icon" class="h-4 w-4 shrink-0" />
    <span class="whitespace-nowrap"
      >{{ label
      }}<span v-if="accel">
        (<span class="mnemonic">{{ accel }}</span
        >)</span
      ></span
    >
  </button>
</template>
