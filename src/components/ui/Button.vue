<script setup lang="ts">
import { computed } from "vue";
import { cn } from "@/lib/utils";

const props = withDefaults(
  defineProps<{
    variant?: "default" | "primary" | "secondary" | "destructive" | "outline" | "ghost";
    size?: "sm" | "md" | "lg" | "icon";
    class?: string;
    disabled?: boolean;
    title?: string;
  }>(),
  { variant: "default", size: "md" }
);

// Flat Qt-style push buttons: subtle raised fill, 1px border, tiny radius.
const variants: Record<string, string> = {
  default:
    "bg-secondary border border-border-strong text-foreground hover:bg-accent hover:border-border-strong",
  primary:
    "bg-selection border border-selection text-selection-foreground hover:bg-selection/90",
  secondary:
    "bg-muted border border-border text-foreground hover:bg-accent",
  destructive:
    "bg-destructive border border-destructive text-destructive-foreground hover:bg-destructive/90",
  outline:
    "border border-border-strong bg-transparent text-foreground hover:bg-accent",
  ghost: "bg-transparent text-foreground hover:bg-accent",
};

const sizes: Record<string, string> = {
  sm: "h-7 px-2.5 text-xs",
  md: "h-8 px-3.5 text-[13px]",
  lg: "h-9 px-5 text-sm",
  icon: "h-8 w-8",
};

const classes = computed(() =>
  cn(
    "inline-flex items-center justify-center gap-1.5 rounded font-normal transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-40 select-none",
    variants[props.variant],
    sizes[props.size],
    props.class
  )
);
</script>

<template>
  <button :class="classes" :disabled="disabled" :title="title">
    <slot />
  </button>
</template>
