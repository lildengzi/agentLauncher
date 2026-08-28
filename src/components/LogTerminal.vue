<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref, watch } from "vue";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { onRuntimeLog, onRuntimeStatus } from "@/lib/api";
import type { RunStatus } from "@/types";

const props = defineProps<{ instanceId: string | null }>();

const host = ref<HTMLDivElement | null>(null);

let term: Terminal | null = null;
let fitAddon: FitAddon | null = null;

const unlistenLog = ref<(() => void) | null>(null);
const unlistenStatus = ref<(() => void) | null>(null);
let resizeObserver: ResizeObserver | null = null;

function statusBanner(status: RunStatus, code?: number | null, message?: string | null): string {
  switch (status) {
    case "starting":
      return "\x1b[33m▶ 启动中...\x1b[0m";
    case "running":
      return "\x1b[32m● 运行中\x1b[0m";
    case "exited":
      return `\x1b[90m■ 已结束 (code=${code ?? "?"})\x1b[0m`;
    case "error":
      return `\x1b[31m✖ ${message ?? "未知错误"}\x1b[0m`;
    default:
      return "";
  }
}

function fit(): void {
  if (fitAddon) {
    try {
      fitAddon.fit();
    } catch {
      /* terminal may not be ready/visible yet */
    }
  }
}

function clear(): void {
  term?.clear();
}

onMounted(async () => {
  if (!host.value) return;

  term = new Terminal({
    fontFamily: "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace",
    fontSize: 13,
    convertEol: true,
    cursorBlink: false,
    scrollback: 5000,
    disableStdin: true,
    theme: {
      background: "#0c0c0e",
      foreground: "#d6dae2",
      cursor: "#5a9bd4",
      cursorAccent: "#0c0c0e",
      selectionBackground: "#3a5a8c66",
    },
  });

  fitAddon = new FitAddon();
  term.loadAddon(fitAddon);
  term.open(host.value);
  fit();

  unlistenLog.value = await onRuntimeLog((e) => {
    if (!term || e.instanceId !== props.instanceId) return;
    if (e.stream === "stderr") {
      term.write("\x1b[31m" + e.chunk + "\x1b[0m");
    } else {
      term.write(e.chunk);
    }
  });

  unlistenStatus.value = await onRuntimeStatus((e) => {
    if (!term || e.instanceId !== props.instanceId) return;
    const banner = statusBanner(e.status, e.code, e.message);
    if (banner) term.writeln(banner);
  });

  resizeObserver = new ResizeObserver(() => fit());
  resizeObserver.observe(host.value);
});

watch(
  () => props.instanceId,
  () => {
    term?.clear();
  },
);

onBeforeUnmount(() => {
  unlistenLog.value?.();
  unlistenLog.value = null;
  unlistenStatus.value?.();
  unlistenStatus.value = null;
  resizeObserver?.disconnect();
  resizeObserver = null;
  term?.dispose();
  term = null;
  fitAddon = null;
});

defineExpose({ clear });
</script>

<template>
  <div ref="host" class="h-full w-full" />
</template>
