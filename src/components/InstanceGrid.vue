<script setup lang="ts">
// Center canvas — Prism Launcher's grouped icon grid, unchanged in structure:
// group header, then 92px icon tiles (icon + name), collapse per group from the
// persisted overlay. A tile carries the icon and the name and nothing else, the
// way the reference does; the bound model lives in the edit dialog. Selection is
// the reference's own: the NAME gets a filled label in the theme's selection
// colour. No coloured box around the tile, and no invented accent hue.
import { computed } from "vue";
import { ChevronDown, ChevronRight } from "lucide-vue-next";
import AppIcon from "@/components/ui/AppIcon.vue";
import Avatar from "@/components/ui/Avatar.vue";
import { useI18n } from "@/lib/i18n";
import { applyOverlay, isCollapsed, toggleCollapsed } from "@/lib/instGroups";
import type { Instance } from "@/types";

const { t } = useI18n();
const props = defineProps<{
  instances: Instance[];
  selectedId: string | null;
  runningIds: string[];
}>();
const emit = defineEmits<{ select: [id: string]; activate: [id: string] }>();

// Group order, collapse and intra-group ordering come from the persisted overlay
// (instgroups.json); membership stays owned by each instance's `group` field.
const groups = computed(() => applyOverlay(props.instances));

function isRunning(id: string): boolean {
  return props.runningIds.includes(id);
}
</script>

<template>
  <section class="min-w-0 flex-1 overflow-y-auto bg-background">
    <div
      v-if="instances.length === 0"
      class="flex h-full flex-col items-center justify-center gap-2 px-6 text-center"
    >
      <AppIcon name="package-open" class="h-8 w-8 text-muted-foreground/50" />
      <p class="text-[14px] text-foreground/80">{{ t('grid.empty.title') }}</p>
      <p class="font-mono text-[12px] text-muted-foreground">{{ t('grid.empty.hint') }}</p>
    </div>

    <template v-else>
      <div v-for="group in groups" :key="group.name">
        <button
          type="button"
          class="group flex w-full items-center gap-1.5 px-2.5 pb-1 pt-2 text-left"
          @click="toggleCollapsed(group.name)"
        >
          <ChevronRight v-if="isCollapsed(group.name)" class="h-3 w-3 text-muted-foreground" />
          <ChevronDown v-else class="h-3 w-3 text-muted-foreground" />
          <span
            class="shrink-0 font-mono text-[12px] text-foreground/75 group-hover:text-foreground"
          >
            {{ group.name }}
          </span>
          <span class="ml-1 h-px flex-1 bg-border" />
        </button>

        <div v-show="!isCollapsed(group.name)" class="flex flex-wrap gap-1 px-2.5 pb-3 pt-1">
          <button
            v-for="inst in group.items"
            :key="inst.id"
            type="button"
            class="group relative flex w-[92px] flex-col items-center gap-1.5 rounded-sm border border-transparent px-1.5 py-2 text-center transition-colors duration-75 hover:border-border-strong hover:bg-accent/60"
            :title="inst.description || inst.name"
            @click="emit('select', inst.id)"
            @dblclick="emit('activate', inst.id)"
          >
            <span class="relative">
              <Avatar
                :seed="inst.id"
                :icon="inst.icon"
                :size="56"
              />
              <span
                v-if="isRunning(inst.id)"
                class="absolute -right-0.5 -top-0.5 h-2.5 w-2.5 rounded-full border-2 border-background bg-selection"
                :title="t('grid.running')"
              />
            </span>
            <span
              class="line-clamp-2 rounded-sm px-1 text-[13px] leading-tight"
              :class="
                inst.id === selectedId
                  ? 'bg-selection text-selection-foreground'
                  : 'text-foreground/85'
              "
            >
              {{ inst.name }}
            </span>
          </button>
        </div>
      </div>
    </template>
  </section>
</template>
