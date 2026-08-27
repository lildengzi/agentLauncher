<script setup lang="ts">
import { cn } from "@/lib/utils";
const props = defineProps<{ class?: string; placeholder?: string; disabled?: boolean }>();
const model = defineModel<string | number>();
</script>

<template>
  <input
    :value="model"
    :placeholder="placeholder"
    :disabled="disabled"
    @input="model = ($event.target as HTMLInputElement).value"
    :class="
      cn(
        'flex h-8 w-full rounded-sm border border-input bg-[hsl(var(--input))] px-2.5 py-1 text-[13px] transition-colors placeholder:text-muted-foreground focus:border-selection focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-45',
        props.class
      )
    "
  />
</template>
