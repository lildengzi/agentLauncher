<script setup lang="ts">
// SEAM PLACEHOLDER — owned by Stream D (下载弹窗 / market dialog).
//
// One widget, three kinds: the plugin / skill / MCP dialogs differ only by the
// `kind` prop, exactly as the three PrismLauncher marketplace dialogs do.
//
// Contract this file must keep, because EditInstanceDialog.vue mounts it:
//   v-model:open  — dialog visibility
//   :kind         — "plugin" | "skill" | "mcp" (picks the title and the query)
//   :instance-id  — install target; "" ⇒ the install button must stay disabled
//   @installed    — emitted after a successful install so the calling section
//                   can re-read `api.readInstanceExtensions`
//
// Stream D implements the Prism layout: source tabs left, list middle, Markdown
// detail right, sort + version dropdown + install button along the bottom. All
// data comes from `api.marketFetch` / `marketReadme` / `marketInstall`; this
// component must NOT import from `src/lib/market/*` — the data layer is the
// backend's (Stream C), reached only through `api`.
import { computed } from "vue";
import Dialog from "@/components/ui/Dialog.vue";
import Button from "@/components/ui/Button.vue";
import { useI18n } from "@/lib/i18n";
import type { ExtensionKind } from "@/types";

const { t } = useI18n();
const open = defineModel<boolean>("open", { default: false });
const props = defineProps<{ kind: ExtensionKind; instanceId: string }>();
defineEmits<{ installed: [] }>();

const title = computed(() => t(`market.title.${props.kind}`));
</script>

<template>
  <Dialog v-model:open="open" width="max-w-5xl" class="h-[80vh]" :title="title">
    <div class="flex h-full items-center justify-center px-6 text-[13px] text-muted-foreground">
      {{ t('market.noSources') }}
    </div>
    <template #footer>
      <div class="flex-1" />
      <Button variant="outline" @click="open = false">{{ t('common.close') }}</Button>
    </template>
  </Dialog>
</template>
