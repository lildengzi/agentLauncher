<script setup lang="ts">
// A "physical" identity avatar. Priority:
//   1. `image` (real PNG/SVG/logo URL) — takes over entirely.
//   2. `brand` (an official simple-icons brand mark) — solid brand-color chip
//      with the white vector logo (e.g. the DeepSeek whale).
//   3. otherwise — a deterministic dual-tone gradient-mesh chip seeded by
//      `seed`, with a centered Lucide vector icon.
// Every variant shares the same glossy inset-highlight treatment.
import { computed } from "vue";
import AppIcon from "@/components/ui/AppIcon.vue";
import type { Brand } from "@/lib/brand";

const props = withDefaults(
  defineProps<{
    seed: string;
    icon?: string;
    image?: string | null;
    brand?: Brand | null;
    size?: number;
    /** selected/active surfaces get a brighter ring. */
    active?: boolean;
    /** rounded-square (default) or full circle. */
    round?: boolean;
  }>(),
  { icon: "bot", image: null, brand: null, size: 56, active: false, round: false }
);

// Stable string hash → hue 0..359.
function hue(s: string, salt = 0): number {
  let h = 2166136261 ^ salt;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return ((h >>> 0) % 360 + 360) % 360;
}

const style = computed(() => {
  const px = `${props.size}px`;
  const radius = props.round ? "9999px" : `${Math.round(props.size * 0.26)}px`;
  let backgroundImage: string;
  if (props.image) {
    backgroundImage = `url("${props.image}")`;
  } else if (props.brand) {
    // solid official brand color + a soft top highlight
    backgroundImage =
      `radial-gradient(circle at 30% 22%, hsl(0 0% 100% / 0.22), transparent 60%),` +
      `linear-gradient(160deg, #${props.brand.hex}, #${props.brand.hex})`;
  } else {
    const h1 = hue(props.seed, 0);
    const h2 = (h1 + 38 + (hue(props.seed, 99) % 40)) % 360;
    backgroundImage =
      `radial-gradient(circle at 30% 22%, hsl(0 0% 100% / 0.30), transparent 58%),` +
      `linear-gradient(142deg, hsl(${h1} 66% 54%), hsl(${h2} 70% 42%))`;
  }
  return {
    width: px,
    height: px,
    borderRadius: radius,
    backgroundImage,
    backgroundColor: props.brand ? `#${props.brand.hex}` : undefined,
    backgroundSize: "cover",
    backgroundPosition: "center",
  };
});

const iconPx = computed(() => Math.round(props.size * 0.48));
const brandPx = computed(() => Math.round(props.size * 0.56));
</script>

<template>
  <span
    class="relative inline-flex shrink-0 items-center justify-center overflow-hidden"
    :class="[
      'shadow-[inset_0_1px_0_hsl(0_0%_100%/0.28),inset_0_-10px_18px_hsl(0_0%_0%/0.20),0_1px_2px_hsl(0_0%_0%/0.35)]',
      active
        ? 'ring-2 ring-selection-foreground/60'
        : 'ring-1 ring-black/20',
    ]"
    :style="style"
  >
    <!-- official brand logo -->
    <svg
      v-if="brand && !image"
      :width="brandPx"
      :height="brandPx"
      viewBox="0 0 24 24"
      role="img"
      :aria-label="brand.title"
      class="drop-shadow-[0_1px_1px_rgba(0,0,0,0.35)]"
    >
      <path :d="brand.path" fill="#ffffff" />
    </svg>
    <!-- generated identity icon -->
    <AppIcon
      v-else-if="!image"
      :name="icon || 'bot'"
      class="text-white/95 drop-shadow-[0_1px_1px_rgba(0,0,0,0.4)]"
      :style="{ width: `${iconPx}px`, height: `${iconPx}px` }"
    />
  </span>
</template>
