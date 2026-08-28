<script setup lang="ts">
// SEAM PLACEHOLDER — owned by Stream A (侧栏 / 改变分组).
//
// Contract this file must keep, because App.vue is written against it:
//   v-model:open   — dialog visibility
//   :instance      — the instance whose group is being changed (null ⇒ no-op)
//   :groups        — group names already in use, for the picker
//   @saved         — emitted with the updated Instance after a successful write
//
// Stream A implements: the existing-group picker + new-group input, validation,
// and the `api.updateInstance` write. The group lives on the instance itself
// (`instance.json`'s `group`); `instgroups.json` is only a presentation overlay,
// so nothing here may write membership into it.
import Dialog from "@/components/ui/Dialog.vue";
import Button from "@/components/ui/Button.vue";
import { useI18n } from "@/lib/i18n";
import type { Instance } from "@/types";

const { t } = useI18n();
const open = defineModel<boolean>("open", { default: false });
defineProps<{ instance: Instance | null; groups: string[] }>();
defineEmits<{ saved: [instance: Instance] }>();
</script>

<template>
  <Dialog v-model:open="open" width="max-w-md" :title="t('group.title')">
    <div class="px-5 py-4">
      <p class="text-[13px] text-muted-foreground">{{ t('group.desc') }}</p>
    </div>
    <template #footer>
      <div class="flex-1" />
      <Button variant="outline" @click="open = false">{{ t('common.close') }}</Button>
    </template>
  </Dialog>
</template>
