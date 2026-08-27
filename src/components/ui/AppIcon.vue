<script setup lang="ts">
import { computed, type FunctionalComponent } from "vue";
import * as icons from "lucide-vue-next";

const props = defineProps<{ name: string; class?: string }>();

// kebab-case lucide name ("flask-conical") -> PascalCase component, fallback Bot.
const comp = computed<FunctionalComponent>(() => {
  const pascal = props.name
    .split("-")
    .filter(Boolean)
    .map((p) => p.charAt(0).toUpperCase() + p.slice(1))
    .join("");
  const registry = icons as unknown as Record<string, FunctionalComponent>;
  return registry[pascal] ?? (icons.Bot as FunctionalComponent);
});
</script>

<template>
  <component :is="comp" :class="props.class" :stroke-width="1.75" />
</template>
