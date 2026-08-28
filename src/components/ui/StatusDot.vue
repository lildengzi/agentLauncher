<script setup lang="ts">
// A hardware-style LED. A live process reads in the theme's own selection
// colour, amber for the transient starting state (the only thing in the UI that
// pulses), theme destructive for errors, muted grey for idle/exited. Flat fill
// only: no halo, no glow, and no accent hue invented outside the theme.
import { computed } from "vue";
import type { RunStatus } from "@/types";
import { useI18n } from "@/lib/i18n";

const { t } = useI18n();
const props = withDefaults(
  defineProps<{ status: RunStatus; label?: boolean; size?: number }>(),
  { label: false, size: 7 }
);

interface Spec {
  color: string;
  pulse: boolean;
  label: string;
}
const spec = computed<Spec>(() => {
  switch (props.status) {
    case "starting":
      return { color: "hsl(38 92% 58%)", pulse: true, label: t("status.starting") };
    case "running":
      return { color: "hsl(var(--selection))", pulse: false, label: t("status.running") };
    case "exited":
      return { color: "hsl(var(--muted-foreground) / 0.8)", pulse: false, label: t("status.exited") };
    case "error":
      return { color: "hsl(var(--destructive))", pulse: false, label: t("status.error") };
    default:
      return { color: "hsl(var(--muted-foreground) / 0.45)", pulse: false, label: t("status.idle") };
  }
});

const dotStyle = computed(() => ({
  width: `${props.size}px`,
  height: `${props.size}px`,
  backgroundColor: spec.value.color,
}));
</script>

<template>
  <span class="inline-flex items-center gap-1.5">
    <span
      class="inline-flex shrink-0 rounded-full"
      :class="spec.pulse ? 'animate-pulse' : ''"
      :style="dotStyle"
    />
    <span
      v-if="label"
      class="font-mono text-[11px] uppercase leading-none tracking-[0.08em]"
      :style="{ color: spec.color }"
    >
      {{ spec.label }}
    </span>
  </span>
</template>
