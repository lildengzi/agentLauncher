<script setup lang="ts">
import { cn } from "@/lib/utils";
import { X } from "lucide-vue-next";

// The dialog *chrome* — optional header, scrolling body, optional footer — with no
// opinion about how it is positioned. Dialog.vue wraps it in a modal overlay; the
// standalone instance editor renders it directly, filling its own OS window. Both
// containers therefore share one implementation of the header/footer contract
// instead of two that drift apart.
//
// Positioning and sizing arrive entirely through `class`, so the caller decides
// between "centered panel with a max height" and "fills the window".
const props = defineProps<{ class?: string; title?: string }>();

// Always visible on its own; the model exists so the close button and the parent
// agree on one flag. In a window, "open = false" is what tells the shell it is done.
const open = defineModel<boolean>("open", { default: true });

function close() {
  open.value = false;
}
</script>

<template>
  <div
    :class="cn('relative flex flex-col overflow-hidden bg-background', props.class)"
  >
    <div
      v-if="title || $slots.header"
      class="flex items-center justify-between border-b border-border bg-toolbar px-3 py-2"
    >
      <slot name="header">
        <h2 class="text-[14px] font-semibold text-foreground">{{ title }}</h2>
      </slot>
      <button
        class="rounded-sm p-1 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
        @click="close"
      >
        <X class="h-4 w-4" />
      </button>
    </div>

    <div class="flex-1 min-h-0 overflow-y-auto">
      <slot />
    </div>

    <div
      v-if="$slots.footer"
      class="flex items-center gap-2 border-t border-border bg-toolbar px-3 py-2"
    >
      <slot name="footer" />
    </div>
  </div>
</template>
