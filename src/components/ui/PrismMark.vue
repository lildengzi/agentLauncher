<script setup lang="ts">
// The launcher's mark: a prism edge splitting one incoming ray. Three bands in
// the current text colour at descending opacity — no hue of its own, so the mark
// inherits whatever theme it sits in.
withDefaults(defineProps<{ size?: number }>(), { size: 16 });
</script>

<template>
  <svg
    :width="size"
    :height="size"
    viewBox="0 0 16 16"
    fill="none"
    aria-hidden="true"
    class="shrink-0"
  >
    <!-- incoming ray -->
    <path d="M0.5 7.5H4" stroke="currentColor" stroke-opacity="0.55" stroke-width="1" />
    <!-- the prism -->
    <path
      d="M8 2.2 13.4 12.6H2.6L8 2.2Z"
      stroke="currentColor"
      stroke-opacity="0.75"
      stroke-width="1"
      stroke-linejoin="round"
    />
    <!-- refracted bands -->
    <path d="M12.6 6.2 15.5 4.6" stroke="currentColor" stroke-opacity="0.7" stroke-width="1" />
    <path d="M12.9 8H15.5" stroke="currentColor" stroke-opacity="0.5" stroke-width="1" />
    <path d="M12.6 9.8 15.5 11.4" stroke="currentColor" stroke-opacity="0.3" stroke-width="1" />
  </svg>
</template>
