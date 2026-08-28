<script setup lang="ts">
// Right action dock — Prism Launcher's list, unchanged: the preview is the icon
// and the name only (the reference shows nothing else there), then 启动/结束进程
// and the management rows with their original mnemonics. Run state is carried by
// the tile LED, the footer count and the enabled/disabled state of these rows,
// so it needs no line of its own. Only the paint changed.
import { computed } from "vue";
import Avatar from "@/components/ui/Avatar.vue";
import ActionButton from "@/components/ui/ActionButton.vue";
import { brandForModel } from "@/lib/brand";
import { useI18n } from "@/lib/i18n";
import type { Instance, RunStatus } from "@/types";

const { t } = useI18n();
const props = defineProps<{ instance: Instance | null; status: RunStatus }>();
const emit = defineEmits<{
  start: [];
  stop: [];
  edit: [];
  changeGroup: [];
  openFolder: [];
  duplicate: [];
  remove: [];
  export: [];
}>();

const runActive = computed(
  () => props.status === "running" || props.status === "starting"
);
</script>

<template>
  <aside class="flex w-60 shrink-0 flex-col border-l border-border bg-panel">
    <div
      v-if="!instance"
      class="flex flex-1 items-center justify-center px-6 text-center text-[13px] text-muted-foreground"
    >
      {{ t('right.selectHint') }}
    </div>

    <template v-else>
      <!-- preview: icon + name, nothing else -->
      <div class="flex flex-col items-center gap-1.5 px-4 pb-3 pt-4">
        <Avatar
          :seed="instance.id"
          :icon="instance.icon"
          :brand="brandForModel(instance.model)"
          :size="72"
        />
        <div class="mt-1 break-all text-center text-[15px] font-semibold leading-tight text-foreground">
          {{ instance.name }}
        </div>
      </div>

      <!-- run controls -->
      <div class="border-t border-border py-1">
        <ActionButton
          icon="play"
          :label="t('right.start')"
          accel="L"
          split
          :disabled="runActive"
          @click="emit('start')"
          @arrow="emit('start')"
        />
        <ActionButton
          icon="square"
          :label="t('right.stop')"
          accel="K"
          :disabled="!runActive"
          @click="emit('stop')"
        />
      </div>

      <!-- management -->
      <div class="border-t border-border py-1">
        <ActionButton icon="pencil" :label="t('right.edit')" accel="E" @click="emit('edit')" />
        <ActionButton
          icon="folder-tree"
          :label="t('right.changeGroup')"
          accel="C"
          @click="emit('changeGroup')"
        />
        <ActionButton
          icon="folder-open"
          :label="t('right.folder')"
          accel="F"
          @click="emit('openFolder')"
        />
        <ActionButton
          icon="package"
          :label="t('right.export')"
          accel="X"
          split
          @click="emit('export')"
          @arrow="emit('export')"
        />
        <ActionButton icon="copy" :label="t('right.duplicate')" accel="Y" @click="emit('duplicate')" />
        <ActionButton
          icon="trash-2"
          :label="t('right.delete')"
          accel="T"
          danger
          @click="emit('remove')"
        />
      </div>

      <div class="flex-1" />
    </template>
  </aside>
</template>
