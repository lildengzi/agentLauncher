<script setup lang="ts">
// An instance icon — just the artwork, the way a launcher shows it. No chip, no
// frame, no tinted plate, no gloss: the mark sits directly on the tile.
//
// Priority:
//   1. `image` (a real PNG/SVG URL) — drawn as-is.
//   2. a brand the *user chose* — `icon` of the form `brand:deepseek`.
//   3. `brand` (a mark inferred from the model) — the vector logo in its own color.
//   4. otherwise — a Lucide glyph, hue-derived from `seed` so instances stay
//      distinguishable at a glance.
//
// 2 sits above 3 on purpose: a mark picked in 选择图标 is a decision, and a mark
// derived from the model id is a guess. The guess must not overrule the decision when
// the two disagree — which they will, the moment somebody points an OpenAI-compatible
// gateway at a Qwen model.
import { computed } from "vue";
import AppIcon from "@/components/ui/AppIcon.vue";
import { brandForIcon, type Brand } from "@/lib/brand";

const props = withDefaults(
  defineProps<{
    seed: string;
    icon?: string;
    image?: string | null;
    brand?: Brand | null;
    size?: number;
    /** rounded-square (default) or full circle — only affects `image`. */
    round?: boolean;
  }>(),
  { icon: "bot", image: null, brand: null, size: 56, round: false }
);

// Stable string hash → hue 0..359.
function hue(s: string, salt = 0): number {
  let h = 2166136261 ^ salt;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return (((h >>> 0) % 360) + 360) % 360;
}

const seedHue = computed(() => hue(props.seed));

const box = computed(() => ({
  width: `${props.size}px`,
  height: `${props.size}px`,
}));
const imageStyle = computed(() => ({
  ...box.value,
  borderRadius: props.round ? "9999px" : "2px",
  backgroundImage: `url("${props.image}")`,
  backgroundSize: "cover",
  backgroundPosition: "center",
}));

const artPx = computed(() => Math.round(props.size * 0.86));
const iconColor = computed(() => `hsl(${seedHue.value} 55% 68%)`);

/** The chosen mark, else the inferred one. */
const mark = computed<Brand | null>(() => brandForIcon(props.icon) ?? props.brand);
</script>

<template>
  <span class="relative inline-flex shrink-0 items-center justify-center" :style="box">
    <!-- real artwork -->
    <span v-if="image" :style="imageStyle" />
    <!-- official brand logo, in its own color -->
    <svg
      v-else-if="mark"
      :width="artPx"
      :height="artPx"
      viewBox="0 0 24 24"
      role="img"
      :aria-label="mark.title"
    >
      <path :d="mark.path" :fill="`#${mark.hex}`" />
    </svg>
    <!-- generated identity glyph -->
    <AppIcon
      v-else
      :name="icon || 'bot'"
      :style="{ width: `${artPx}px`, height: `${artPx}px`, color: iconColor }"
    />
  </span>
</template>
