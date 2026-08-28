<script setup lang="ts">
// Themed dropdown replacing the native <select>. A native option list is painted
// by the OS (light background, grey text) and ignores the app theme entirely, so
// it is unreadable here — this renders the list ourselves.
//
// The panel is teleported to <body> with fixed positioning: the dialog body it
// lives in scrolls and clips overflow, and the dialog itself sits at z-50.
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { Check, ChevronDown } from "lucide-vue-next";
import { cn } from "@/lib/utils";

export interface SelectOption {
  value: string;
  label: string;
  /** muted suffix after the label, e.g. an install-status marker. */
  hint?: string;
  /** paint `hint` as a warning instead of muted. */
  warn?: boolean;
}

const props = defineProps<{
  options: SelectOption[];
  class?: string;
  disabled?: boolean;
  placeholder?: string;
}>();

const model = defineModel<string>({ default: "" });

// The template's root is a fragment (trigger + teleported panel), so attribute
// fallthrough cannot pick a host — forward $attrs (id, aria-*, …) to the trigger.
defineOptions({ inheritAttrs: false });

const open = ref(false);
const trigger = ref<HTMLButtonElement | null>(null);
const menu = ref<HTMLElement | null>(null);
const activeIndex = ref(-1);
const menuStyle = ref<Record<string, string>>({});

const selected = computed(
  () => props.options.find((o) => o.value === model.value) ?? null
);

/** Anchor the panel to the trigger, flipping above when it would overflow. */
function place() {
  const el = trigger.value;
  if (!el) return;
  const r = el.getBoundingClientRect();
  const gap = 4;
  const wanted = Math.min(props.options.length * 30 + 8, 280);
  const below = window.innerHeight - r.bottom - gap;
  const flip = below < wanted && r.top > below;
  menuStyle.value = {
    position: "fixed",
    left: `${r.left}px`,
    minWidth: `${r.width}px`,
    maxHeight: `${Math.max(96, flip ? r.top - gap : below)}px`,
    ...(flip
      ? { bottom: `${window.innerHeight - r.top + gap}px` }
      : { top: `${r.bottom + gap}px` }),
  };
}

function toggle() {
  if (props.disabled) return;
  open.value = !open.value;
}

function pick(value: string) {
  model.value = value;
  open.value = false;
  trigger.value?.focus();
}

function onKeydown(e: KeyboardEvent) {
  if (props.disabled) return;
  if (e.key === "Escape" && open.value) {
    open.value = false;
    return;
  }
  if (!open.value && (e.key === "Enter" || e.key === " " || e.key === "ArrowDown")) {
    e.preventDefault();
    open.value = true;
    return;
  }
  if (!open.value) return;
  if (e.key === "ArrowDown" || e.key === "ArrowUp") {
    e.preventDefault();
    const step = e.key === "ArrowDown" ? 1 : -1;
    const n = props.options.length;
    if (n) activeIndex.value = (activeIndex.value + step + n) % n;
  } else if (e.key === "Enter") {
    e.preventDefault();
    const opt = props.options[activeIndex.value];
    if (opt) pick(opt.value);
  }
}

function onDocPointer(e: PointerEvent) {
  const t = e.target as Node;
  if (trigger.value?.contains(t) || menu.value?.contains(t)) return;
  open.value = false;
}

watch(open, async (isOpen) => {
  if (isOpen) {
    activeIndex.value = props.options.findIndex((o) => o.value === model.value);
    place();
    document.addEventListener("pointerdown", onDocPointer, true);
    // Follow the trigger while the dialog body (or window) scrolls.
    window.addEventListener("scroll", place, true);
    window.addEventListener("resize", place);
    await nextTick();
    menu.value?.querySelector<HTMLElement>("[data-active='true']")?.scrollIntoView({
      block: "nearest",
    });
  } else {
    document.removeEventListener("pointerdown", onDocPointer, true);
    window.removeEventListener("scroll", place, true);
    window.removeEventListener("resize", place);
  }
});

onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", onDocPointer, true);
  window.removeEventListener("scroll", place, true);
  window.removeEventListener("resize", place);
});
</script>

<template>
  <button
    ref="trigger"
    type="button"
    role="combobox"
    v-bind="$attrs"
    :aria-expanded="open"
    :disabled="disabled"
    :class="
      cn(
        'flex h-8 w-full items-center justify-between gap-2 rounded-sm border border-input bg-[hsl(var(--input))] px-2.5 text-left text-[13px] text-foreground transition-colors hover:border-border-strong focus:border-selection focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-45',
        open && 'border-selection',
        props.class
      )
    "
    @click="toggle"
    @keydown="onKeydown"
  >
    <span class="min-w-0 flex-1 truncate">
      <template v-if="selected">
        {{ selected.label }}
        <span
          v-if="selected.hint"
          :class="selected.warn ? 'text-destructive' : 'text-muted-foreground'"
        >
          · {{ selected.hint }}
        </span>
      </template>
      <span v-else class="text-muted-foreground">{{ placeholder ?? "" }}</span>
    </span>
    <ChevronDown
      class="h-3.5 w-3.5 shrink-0 text-muted-foreground transition-transform"
      :class="open && 'rotate-180'"
      :stroke-width="2"
    />
  </button>

  <Teleport to="body">
    <div
      v-if="open"
      ref="menu"
      :style="menuStyle"
      class="z-[60] overflow-y-auto rounded-sm border border-border-strong bg-card py-1 shadow-2xl"
      role="listbox"
    >
      <button
        v-for="(opt, i) in options"
        :key="opt.value"
        type="button"
        role="option"
        :aria-selected="opt.value === model"
        :data-active="i === activeIndex"
        class="flex w-full items-center gap-2 px-2.5 py-1.5 text-left text-[13px] transition-colors"
        :class="
          i === activeIndex
            ? 'bg-selection text-selection-foreground'
            : 'text-card-foreground hover:bg-accent'
        "
        @mouseenter="activeIndex = i"
        @click="pick(opt.value)"
      >
        <Check
          class="h-3.5 w-3.5 shrink-0"
          :class="opt.value === model ? 'opacity-100' : 'opacity-0'"
          :stroke-width="2.25"
        />
        <span class="min-w-0 flex-1 truncate">
          {{ opt.label }}
          <span
            v-if="opt.hint"
            :class="
              i === activeIndex
                ? 'text-selection-foreground/75'
                : opt.warn
                  ? 'text-destructive'
                  : 'text-muted-foreground'
            "
          >
            · {{ opt.hint }}
          </span>
        </span>
      </button>
    </div>
  </Teleport>
</template>
