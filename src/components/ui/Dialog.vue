<script setup lang="ts">
import { cn } from "@/lib/utils";
import DialogPanel from "@/components/ui/DialogPanel.vue";

// The modal container: a backdrop that closes on an outside click, wrapping the
// shared DialogPanel chrome. Teleported to <body> so a dialog opened from inside a
// scroll container or another dialog is not clipped by it.
const props = withDefaults(
  defineProps<{
    /** dialog panel width class, e.g. "max-w-lg", "max-w-4xl". */
    width?: string;
    class?: string;
    title?: string;
  }>(),
  { width: "max-w-lg" }
);

const open = defineModel<boolean>("open", { default: false });

function close() {
  open.value = false;
}
</script>

<template>
  <Teleport to="body">
    <Transition
      enter-active-class="transition duration-150 ease-out"
      enter-from-class="opacity-0"
      leave-active-class="transition duration-100 ease-in"
      leave-to-class="opacity-0"
    >
      <div
        v-if="open"
        class="fixed inset-0 z-50 flex items-center justify-center bg-black/65 p-6"
        @click.self="close"
      >
        <DialogPanel
          v-model:open="open"
          :title="title"
          :class="
            cn(
              'w-full max-h-[92vh] rounded-sm border border-border-strong shadow-[0_16px_48px_-12px_rgba(0,0,0,0.85)]',
              props.width,
              props.class
            )
          "
        >
          <!-- Forwarded conditionally: an unconditional <template #footer> would
               make the panel's own `$slots.footer` check always true and draw an
               empty footer bar under every dialog that has none. -->
          <template v-if="$slots.header" #header><slot name="header" /></template>
          <slot />
          <template v-if="$slots.footer" #footer><slot name="footer" /></template>
        </DialogPanel>
      </div>
    </Transition>
  </Teleport>
</template>
