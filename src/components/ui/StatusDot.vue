<script setup lang="ts">
// A hardware-style LED status indicator: a solid center dot with a pinging halo
// ring while the process is live (running/starting), plus an optional uppercase
// mono label ("SYSTEM READY" etc.). Colors are the conventional status palette
// (emerald/amber/rose) — the one place we intentionally step outside theme tokens,
// because these read as physical indicator LEDs, not UI chrome.
import { computed } from "vue";
import type { RunStatus } from "@/types";
import { useI18n } from "@/lib/i18n";

const { t } = useI18n();
const props = withDefaults(
  defineProps<{ status: RunStatus; label?: boolean; size?: number }>(),
  { label: false, size: 8 }
);

interface Spec {
  color: string;
  ping: boolean;
  label: string;
}
const spec = computed<Spec>(() => {
  switch (props.status) {
    case "starting":
      return { color: "#fbbf24", ping: true, label: t("status.starting") };
    case "running":
      return { color: "#34d399", ping: true, label: t("status.running") };
    case "exited":
      return { color: "#71717a", ping: false, label: t("status.exited") };
    case "error":
      return { color: "#fb7185", ping: false, label: t("status.error") };
    default:
      return { color: "#52525b", ping: false, label: t("status.idle") };
  }
});

const dotStyle = computed(() => ({ width: `${props.size}px`, height: `${props.size}px` }));
</script>

<template>
  <span class="inline-flex items-center gap-2">
    <span class="relative inline-flex" :style="dotStyle">
      <span
        v-if="spec.ping"
        class="absolute inset-0 animate-ping rounded-full opacity-75"
        :style="{ backgroundColor: spec.color }"
      />
      <span
        class="relative inline-flex rounded-full"
        :style="{ ...dotStyle, backgroundColor: spec.color, boxShadow: `0 0 6px ${spec.color}` }"
      />
    </span>
    <span
      v-if="label"
      class="font-mono text-[11px] uppercase tracking-wider"
      :style="{ color: spec.color }"
    >
      {{ spec.label }}
    </span>
  </span>
</template>
