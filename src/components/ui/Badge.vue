<script setup lang="ts">
import { cn } from "@/lib/utils";
const props = withDefaults(
  defineProps<{
    variant?: "default" | "secondary" | "outline";
    class?: string;
  }>(),
  { variant: "default" }
);
const variants: Record<string, string> = {
  default: "bg-selection/20 text-link border-selection/40",
  secondary: "bg-muted text-muted-foreground border-transparent",
  outline: "bg-transparent text-muted-foreground border-border-strong",
};
</script>

<template>
  <span
    :class="
      cn(
        'inline-flex items-center rounded-sm border px-1.5 py-0.5 text-[11px] font-medium leading-none',
        variants[props.variant],
        props.class
      )
    "
  >
    <slot />
  </span>
</template>
