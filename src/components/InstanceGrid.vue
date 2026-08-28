<script setup lang="ts">
import { computed } from "vue";
import { ChevronDown, ChevronRight } from "lucide-vue-next";
import AppIcon from "@/components/ui/AppIcon.vue";
import Avatar from "@/components/ui/Avatar.vue";
import { brandForModel } from "@/lib/brand";
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
      class="flex h-full flex-col items-center justify-center gap-2 px-6 text-center text-muted-foreground"
    >
      <AppIcon name="package-open" class="h-10 w-10 opacity-60" />
      <p class="text-sm">{{ t('grid.empty.title') }}</p>
      <p class="text-xs">{{ t('grid.empty.hint') }}</p>
    </div>

    <template v-else>
      <div v-for="group in groups" :key="group.name" class="pt-1">
        <button
          type="button"
          class="group flex w-full items-center gap-1.5 px-3 py-1.5 text-left text-[13px] font-medium text-foreground/80 hover:text-foreground"
          @click="toggleCollapsed(group.name)"
        >
          <ChevronRight v-if="isCollapsed(group.name)" class="h-4 w-4 opacity-70" />
          <ChevronDown v-else class="h-4 w-4 opacity-70" />
          <span class="shrink-0">{{ group.name }}</span>
          <span class="ml-2 h-px flex-1 bg-border" />
        </button>

        <div v-show="!isCollapsed(group.name)" class="flex flex-wrap gap-1 px-3 pb-3 pt-1">
          <button
            v-for="inst in group.items"
            :key="inst.id"
            type="button"
            class="group relative flex w-[92px] flex-col items-center gap-1.5 rounded px-1.5 py-2 text-center transition-colors"
            :class="
              inst.id === selectedId
                ? 'bg-selection text-selection-foreground'
                : 'hover:bg-accent'
            "
            @click="emit('select', inst.id)"
            @dblclick="emit('activate', inst.id)"
          >
            <span class="relative">
              <Avatar
                :seed="inst.id"
                :icon="inst.icon"
                :brand="brandForModel(inst.model)"
                :size="56"
                :active="inst.id === selectedId"
              />
              <span
                v-if="isRunning(inst.id)"
                class="absolute -right-1 -top-1 inline-flex"
                :title="t('grid.running')"
              >
                <span class="absolute inset-0 animate-ping rounded-full bg-emerald-400 opacity-75" />
                <span class="relative h-3 w-3 rounded-full border-2 border-background bg-emerald-400" />
              </span>
            </span>
            <span
              class="line-clamp-2 text-[12px] leading-tight"
              :class="inst.id === selectedId ? 'text-selection-foreground' : 'text-foreground/85'"
            >
              {{ inst.name }}
            </span>
          </button>
        </div>
      </div>
    </template>
  </section>
</template>
