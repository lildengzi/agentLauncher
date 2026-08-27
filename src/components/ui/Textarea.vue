<script setup lang="ts">
import { cn } from "@/lib/utils";
const props = defineProps<{ class?: string; placeholder?: string; disabled?: boolean }>();
const model = defineModel<string>();
</script>

<template>
  <textarea
    :value="model"
    :placeholder="placeholder"
    :disabled="disabled"
    @input="model = ($event.target as HTMLTextAreaElement).value"
    :class="
      cn(
        'flex min-h-[80px] w-full rounded-sm border border-input bg-[hsl(var(--input))] px-2.5 py-2 text-[13px] leading-relaxed transition-colors placeholder:text-muted-foreground focus:border-selection focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-45',
        props.class
      )
    "
  />
</template>
