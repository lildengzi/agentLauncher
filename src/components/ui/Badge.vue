<script setup lang="ts">
// A metadata chip: engine, profile, model family, plugin count. Always 1px,
// always 2px radius, and `mono` for anything a machine produced.
import { cn } from "@/lib/utils";
const props = withDefaults(
  defineProps<{
    variant?: "default" | "secondary" | "outline" | "accent" | "warn";
    /** render the content in the mono stack (model ids, counts, paths). */
    mono?: boolean;
    class?: string;
  }>(),
  { variant: "default", mono: false }
);
const variants: Record<string, string> = {
  default: "bg-selection/15 text-link border-selection/35",
  secondary: "bg-muted text-muted-foreground border-border",
  outline: "bg-transparent text-muted-foreground border-border-strong",
  accent: "bg-selection/10 text-selection border-selection/40",
  warn: "bg-destructive/10 text-destructive border-destructive/35",
};
</script>

<template>
  <span
    :class="
      cn(
        'inline-flex items-center gap-1 rounded-sm border px-1.5 py-px text-[12px] leading-[15px]',
        props.mono ? 'font-mono tracking-tight' : 'font-medium',
        variants[props.variant],
        props.class
      )
    "
  >
    <slot />
  </span>
</template>
