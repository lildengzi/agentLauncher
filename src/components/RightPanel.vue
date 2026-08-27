<script setup lang="ts">
import { computed } from "vue";
import Avatar from "@/components/ui/Avatar.vue";
import StatusDot from "@/components/ui/StatusDot.vue";
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
  manageMcp: [];
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
  <aside class="flex w-60 shrink-0 flex-col border-l border-border bg-toolbar">
    <div
      v-if="!instance"
      class="flex flex-1 items-center justify-center px-6 text-center text-[13px] text-muted-foreground"
    >
      {{ t('right.selectHint') }}
    </div>

    <template v-else>
      <!-- header: avatar + name + subtitle -->
      <div class="flex flex-col items-center gap-2 px-4 pb-4 pt-6">
        <Avatar :seed="instance.id" :icon="instance.icon" :brand="brandForModel(instance.model)" :size="80" />
        <div class="mt-1 text-center text-[15px] font-semibold leading-snug text-foreground">
          {{ instance.name }}
        </div>
        <div class="text-center text-[12px] text-muted-foreground">
          {{ instance.model }}
        </div>
        <StatusDot :status="status" label class="mt-0.5" />
      </div>

      <div class="mx-3 border-t border-border" />

      <!-- run controls -->
      <div class="py-1.5">
        <ActionButton
          icon="play"
          :label="t('right.start')"
          accel="L"
          emphasized
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

      <div class="mx-3 border-t border-border" />

      <!-- management -->
      <div class="py-1.5">
        <ActionButton icon="pencil" :label="t('right.edit')" accel="E" @click="emit('edit')" />
        <ActionButton icon="puzzle" :label="t('right.mcp')" accel="C" @click="emit('manageMcp')" />
        <ActionButton icon="folder-open" :label="t('right.folder')" accel="F" @click="emit('openFolder')" />
        <ActionButton icon="package" :label="t('right.export')" accel="X" split @click="emit('export')" @arrow="emit('export')" />
        <ActionButton icon="copy" :label="t('right.duplicate')" accel="Y" @click="emit('duplicate')" />
        <ActionButton icon="trash-2" :label="t('right.delete')" accel="T" @click="emit('remove')" />
      </div>

      <div class="flex-1" />
    </template>
  </aside>
</template>
