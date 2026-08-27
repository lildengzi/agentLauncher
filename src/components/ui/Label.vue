<script setup lang="ts">
import { cn } from "@/lib/utils";
const props = defineProps<{ class?: string; for?: string }>();
</script>

<template>
  <label
    :for="props.for"
    :class="cn('text-[13px] leading-none text-foreground/90', props.class)"
  >
    <slot />
  </label>
</template>
