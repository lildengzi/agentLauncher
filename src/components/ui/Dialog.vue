<script setup lang="ts">
import { cn } from "@/lib/utils";
import { X } from "lucide-vue-next";

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
        <div
          :class="
            cn(
              'relative w-full max-h-[92vh] overflow-hidden flex flex-col rounded-sm border border-border-strong bg-background shadow-[0_16px_48px_-12px_rgba(0,0,0,0.85)]',
              props.width,
              props.class
            )
          "
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
      </div>
    </Transition>
  </Teleport>
</template>
