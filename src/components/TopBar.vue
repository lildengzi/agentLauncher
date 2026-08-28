<script setup lang="ts">
// Qt-style toolbar, same actions and same mnemonics as the reference. No drag
// grip, no vertical rules: Prism Launcher's bar starts flush at the left edge
// with the first button, and hover is what separates segments. The bar ends the
// way the reference does, with the account button pushed to the right.
//
// The label is 账户 / Account, exactly as Prism Launcher prints it — not a
// username, because agentLauncher has no account to name yet (model access is a
// per-instance API key). The button is inert until there is something behind it.
import ToolbarButton from "@/components/ui/ToolbarButton.vue";
import { UserRound } from "lucide-vue-next";
import { useI18n } from "@/lib/i18n";

const { t } = useI18n();
defineEmits<{ add: []; folder: []; settings: []; help: [] }>();
</script>

<template>
  <header class="flex h-[34px] shrink-0 items-stretch border-b border-border bg-toolbar">
    <ToolbarButton icon="plus-square" :label="t('top.add')" accel="E" @click="$emit('add')" />
    <ToolbarButton icon="folder" :label="t('top.folder')" accel="O" @click="$emit('folder')" />
    <ToolbarButton icon="settings" :label="t('top.settings')" accel="N" @click="$emit('settings')" />
    <ToolbarButton icon="circle-help" :label="t('top.help')" @click="$emit('help')" />

    <div class="flex-1" />

    <button
      type="button"
      :title="t('top.account')"
      class="flex shrink-0 items-center gap-2 px-3 text-[13.5px] text-foreground transition-colors duration-75 hover:bg-accent hover:text-accent-foreground"
    >
      <span
        class="flex h-5 w-5 items-center justify-center rounded-sm border border-border-strong text-muted-foreground"
      >
        <UserRound class="h-3.5 w-3.5" />
      </span>
      <span class="whitespace-nowrap">{{ t('top.account') }}</span>
    </button>
  </header>
</template>
