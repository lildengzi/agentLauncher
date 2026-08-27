<script setup lang="ts">
import { ref, computed } from "vue";
import { Square, Eraser, Globe } from "lucide-vue-next";
import Dialog from "@/components/ui/Dialog.vue";
import Button from "@/components/ui/Button.vue";
import Avatar from "@/components/ui/Avatar.vue";
import StatusDot from "@/components/ui/StatusDot.vue";
import LogTerminal from "@/components/LogTerminal.vue";
import { brandForModel } from "@/lib/brand";
import { useI18n } from "@/lib/i18n";
import type { Instance, RunStatus } from "@/types";

const { t } = useI18n();
const props = defineProps<{
  instance: Instance | null;
  status: RunStatus;
  url: string | null;
}>();
const emit = defineEmits<{ stop: []; openWeb: [] }>();
const open = defineModel<boolean>("open", { default: false });

const termRef = ref<InstanceType<typeof LogTerminal> | null>(null);

const busy = computed(
  () => props.status === "running" || props.status === "starting"
);
</script>

<template>
  <Dialog v-model:open="open" width="max-w-4xl" class="h-[80vh]">
    <template #header>
      <div class="flex items-center gap-2">
        <Avatar v-if="instance" :seed="instance.id" :icon="instance.icon" :brand="brandForModel(instance.model)" :size="20" />
        <span class="text-[15px] font-semibold text-foreground">
          {{ instance?.name ?? t('console.title') }}
        </span>
        <span class="text-[12px] text-muted-foreground">· {{ t('console.title') }}</span>
        <StatusDot :status="status" label class="ml-2" />
      </div>
    </template>

    <div class="flex h-full flex-col">
      <div class="min-h-0 flex-1 bg-[#0c0c0e]">
        <LogTerminal
          v-if="instance"
          ref="termRef"
          :instanceId="instance.id"
          class="h-full"
        />
      </div>
      <div class="flex items-center gap-2 border-t border-border bg-toolbar px-3 py-2">
        <span class="flex-1 text-[12px] text-muted-foreground">{{ t('console.readonlyHint') }}</span>
        <Button v-if="url" variant="primary" @click="emit('openWeb')">
          <Globe class="h-4 w-4" /> {{ t('console.openWeb') }}
        </Button>
        <Button v-if="busy" variant="destructive" @click="emit('stop')">
          <Square class="h-4 w-4" /> {{ t('console.stop') }}
        </Button>
      </div>
    </div>

    <template #footer>
      <Button variant="ghost" size="sm" @click="termRef?.clear()">
        <Eraser class="h-4 w-4" /> {{ t('console.clear') }}
      </Button>
      <div class="flex-1" />
      <Button variant="outline" @click="open = false">{{ t('console.close') }}</Button>
    </template>
  </Dialog>
</template>
