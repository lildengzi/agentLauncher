<script setup lang="ts">
// A physical keycap. Flat, 1px, mono — the accelerator hints in the toolbar and
// the action dock all render through this so they stay visually identical.
defineProps<{ keys: string }>();
</script>

<template>
  <kbd class="kbd">{{ keys }}</kbd>
</template>
