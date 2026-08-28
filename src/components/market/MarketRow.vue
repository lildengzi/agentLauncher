<script setup lang="ts">
// One row of the market list — icon, name, installed marker, one-line summary.
//
// Every string here arrives from a third-party feed, so it is bound as text and
// clamped: `truncate` is not decoration, it is what keeps a 4 KB "name" from
// pushing the detail pane off the dialog. The icon is a lucide *name* run through
// AppIcon (unknown names fall back), never a URL — a feed does not get to make the
// launcher fetch an image.
import AppIcon from "@/components/ui/AppIcon.vue";
import { useI18n } from "@/lib/i18n";
import type { MarketItem } from "@/types";

const { t } = useI18n();

const props = defineProps<{
  item: MarketItem;
  selected: boolean;
  /** already present in the target instance, matched per install method. */
  installed: boolean;
  /** human label of the source this row came from, id as fallback. */
  sourceLabel: string;
}>();
defineEmits<{ select: [] }>();

// A selected row is painted with the selection colour, so its secondary text has
// to come off the selection foreground rather than the muted grey.
const dim = () => (props.selected ? "text-selection-foreground/80" : "text-muted-foreground");
</script>

<template>
  <button
    type="button"
    class="flex w-full items-start gap-3 border-b border-border/60 px-3 py-2 text-left transition-colors"
    :class="selected ? 'bg-selection text-selection-foreground' : 'hover:bg-accent'"
    @click="$emit('select')"
  >
    <AppIcon :name="item.icon || 'package'" class="mt-0.5 h-5 w-5 shrink-0" />
    <span class="min-w-0 flex-1">
      <span class="flex items-baseline gap-2">
        <span class="truncate text-[14px] font-semibold">{{ item.name }}</span>
        <span v-if="installed" class="shrink-0 text-[12px]" :class="dim()">
          {{ t("market.installed") }}
        </span>
        <span class="ml-auto max-w-[8rem] shrink-0 truncate text-[12px]" :class="dim()">
          {{ sourceLabel }}
        </span>
      </span>
      <span v-if="item.description" class="block truncate text-[13px]" :class="dim()">
        {{ item.description }}
      </span>
    </span>
  </button>
</template>
