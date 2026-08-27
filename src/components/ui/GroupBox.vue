<script setup lang="ts">
withDefaults(
  defineProps<{
    title: string;
    accel?: string;
    /** show a leading checkbox that enables/disables the group. */
    checkable?: boolean;
  }>(),
  { checkable: false }
);

const checked = defineModel<boolean>("checked", { default: true });
</script>

<template>
  <fieldset class="mb-4">
    <legend class="mb-1.5 flex items-center gap-2 px-0.5 text-[13px] font-medium text-foreground">
      <input
        v-if="checkable"
        type="checkbox"
        v-model="checked"
        class="h-3.5 w-3.5 accent-selection"
      />
      <span>{{ title }}<span v-if="accel" class="opacity-70"> (<span class="mnemonic">{{ accel }}</span>)</span></span>
    </legend>
    <div
      class="rounded border border-border bg-panel px-4 py-3.5"
      :class="checkable && !checked ? 'pointer-events-none opacity-45' : ''"
    >
      <slot />
    </div>
  </fieldset>
</template>
