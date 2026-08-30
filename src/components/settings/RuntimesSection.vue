<script setup lang="ts">
// Settings ▸ 工具 — the launcher's own agent-CLI installs.
//
// The Prism-manages-its-own-Java move: everything goes into
// `~/.agentlauncher/runtimes/`, never `npm i -g`. That one choice is what buys no
// administrator rights (the blocker on Windows), no edit to the user's PATH, no
// global pollution, and an uninstall that is one `rm -r` — and because the
// launcher composes the child PATH itself, a CLI installed here works without
// restarting the app.
//
// Three rules this pane keeps, all for the same reason — installing means running
// somebody else's code:
//
//  * Nothing installs on mount. Mounting only *probes* (a PATH lookup; it creates
//    no directory and executes no candidate). The button is the ask.
//  * The exact command is shown before it runs, and again in the log as the
//    backend echoes it.
//  * Every row keeps a copy-the-command escape hatch, so a row whose button
//    cannot work still leaves the user somewhere to go. `omp` has no button at
//    all: it is built from git source by its packagers, and a button that must
//    fail is worse than an honest link.
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { ExternalLink, RefreshCw, Terminal } from "lucide-vue-next";
import Button from "@/components/ui/Button.vue";
import GroupBox from "@/components/ui/GroupBox.vue";
import { api, onInstallDone, onInstallLog } from "@/lib/api";
import { useI18n } from "@/lib/i18n";
import type { EngineInfo } from "@/types";

const NODE_URL = "https://nodejs.org/en/download";

const { t } = useI18n();

const engines = ref<EngineInfo[]>([]);
const dir = ref("");
const npm = ref("");
const loading = ref(true);
/** The engine currently installing — one at a time, which the backend enforces
 *  too: two npm processes writing one `node_modules` is how you corrupt it. */
const busy = ref("");
const error = ref("");
const logLines = ref<string[]>([]);
const copied = ref("");

let unlistenLog: (() => void) | null = null;
let unlistenDone: (() => void) | null = null;
const logEl = ref<HTMLElement | null>(null);

const canInstall = computed(() => !!npm.value && !busy.value);

/** The command the user would run by hand — also exactly what the button runs, so
 *  the copy fallback is never a different install from the button's. */
function command(e: EngineInfo): string {
  if (e.install === "npm" && e.package) {
    return `npm install --prefix ${dir.value || "~/.agentlauncher/runtimes"} ${e.package}@latest`;
  }
  // Manual rows have no package to name; the docs URL is the whole answer.
  return e.docs;
}

async function refresh() {
  loading.value = true;
  error.value = "";
  try {
    // `api.detectEngines` rather than `probeEngines`: that helper falls back to a
    // table where every engine claims `installed: true`, which is the right hedge
    // for a picker (a failed probe must not lock you out of an engine you have)
    // and exactly the wrong one here — this pane exists to say what is on disk, so
    // a failed probe has to read as a failure, not as six green rows.
    const [list, status] = await Promise.all([
      api.detectEngines(),
      api.runtimesStatus(),
    ]);
    engines.value = list;
    dir.value = status.dir;
    npm.value = status.npm;
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

onMounted(async () => {
  // Listen before the first install can start, so no line is missed.
  unlistenLog = await onInstallLog((e) => {
    logLines.value = [...logLines.value, e.chunk.replace(/\n$/, "")];
    // Follow the tail the way a console does; the pane is short on purpose.
    requestAnimationFrame(() => {
      if (logEl.value) logEl.value.scrollTop = logEl.value.scrollHeight;
    });
  });
  unlistenDone = await onInstallDone(async (e) => {
    busy.value = "";
    logLines.value = [...logLines.value, e.message];
    if (!e.ok) error.value = e.message;
    // Re-probe rather than trusting the event: the row must say what is on disk.
    await refresh();
  });
  await refresh();
});
onBeforeUnmount(() => {
  unlistenLog?.();
  unlistenDone?.();
});

async function install(e: EngineInfo) {
  if (!canInstall.value || e.install !== "npm") return;
  error.value = "";
  busy.value = e.id;
  try {
    await api.installEngine(e.id);
  } catch (err) {
    // A rejected call means it never started, so no `install-done` is coming.
    busy.value = "";
    error.value = String(err);
  }
}

async function copy(e: EngineInfo) {
  try {
    await navigator.clipboard.writeText(command(e));
    copied.value = e.id;
    window.setTimeout(() => (copied.value = ""), 1600);
  } catch {
    /* a denied clipboard is not worth an error banner; the text is on screen */
  }
}

function openDocs(url: string) {
  // Never window.open: URLs go out through the backend opener, same as the market.
  api.openUrl(url).catch(console.error);
}
</script>

<template>
  <GroupBox :title="t('runtimes.title')">
    <p class="text-[13px] text-muted-foreground">{{ t('runtimes.desc') }}</p>

    <div class="mt-2.5 grid grid-cols-[92px_1fr] gap-x-3 gap-y-1 text-[13px]">
      <span class="text-foreground/85">{{ t('runtimes.dir') }}</span>
      <span class="min-w-0 break-all font-mono text-[12px] text-muted-foreground">{{ dir }}</span>
    </div>

    <!-- Prerequisite. npm missing is not an error state of this pane — it is the
         one thing the user has to do first, so it reads as an instruction. -->
    <div
      v-if="!loading && !npm"
      class="mt-3 flex flex-wrap items-center gap-2 rounded border border-border bg-toolbar px-3 py-2"
    >
      <span class="text-[13px] text-foreground/85">{{ t('runtimes.npmMissing') }}</span>
      <Button size="sm" variant="outline" @click="openDocs(NODE_URL)">
        <ExternalLink class="h-3.5 w-3.5" />
        {{ t('runtimes.getNode') }}
      </Button>
    </div>
    <p v-else class="mt-2 text-[12px] text-muted-foreground">{{ t('runtimes.prereq') }}</p>

    <p v-if="loading" class="mt-3 text-[13px] text-muted-foreground">{{ t('common.loading') }}</p>

    <div v-else class="mt-3 flex flex-col gap-2">
      <div
        v-for="e in engines"
        :key="e.id"
        class="rounded border border-border px-3 py-2.5"
      >
        <div class="flex flex-wrap items-center gap-x-2 gap-y-1.5">
          <span
            class="inline-flex h-[7px] w-[7px] shrink-0 rounded-full"
            :class="e.installed ? 'bg-selection' : 'bg-muted-foreground/45'"
          />
          <span class="font-mono text-[13px] text-foreground/90">{{ e.id }}</span>
          <span class="min-w-0 truncate text-[13px] text-muted-foreground">{{ e.display }}</span>
          <span
            v-if="e.managed"
            class="shrink-0 rounded-sm border border-border px-1.5 py-0.5 text-[11px] text-muted-foreground"
          >
            {{ t('runtimes.managed') }}
          </span>
          <span
            v-else-if="e.install === 'manual'"
            class="shrink-0 rounded-sm border border-border px-1.5 py-0.5 text-[11px] text-muted-foreground"
          >
            {{ t('runtimes.manual') }}
          </span>

          <div class="flex-1" />

          <Button size="sm" variant="ghost" :title="t('runtimes.docs')" @click="openDocs(e.docs)">
            <ExternalLink class="h-3.5 w-3.5" />
          </Button>
          <Button size="sm" variant="outline" @click="copy(e)">
            <Terminal class="h-3.5 w-3.5" />
            {{ copied === e.id ? t('runtimes.copied') : t('runtimes.copyCommand') }}
          </Button>
          <Button
            v-if="e.install === 'npm'"
            size="sm"
            :variant="e.installed ? 'outline' : 'primary'"
            :disabled="!canInstall"
            @click="install(e)"
          >
            {{
              busy === e.id
                ? t('runtimes.installing')
                : e.installed
                  ? t('runtimes.update')
                  : t('runtimes.install')
            }}
          </Button>
        </div>

        <div class="mt-1.5 flex flex-wrap items-baseline gap-x-2 text-[12px] text-muted-foreground">
          <span :class="e.installed ? '' : 'text-foreground/70'">
            {{ e.installed ? t('runtimes.installed') : t('runtimes.missing') }}
          </span>
          <span v-if="e.path" class="min-w-0 break-all font-mono">{{ e.path }}</span>
        </div>
        <p v-if="e.install === 'manual'" class="mt-1 text-[12px] text-muted-foreground">
          {{ t('runtimes.manualHint') }}
        </p>
        <!-- Shown, not hidden behind the copy button: the user is entitled to see
             what would be fetched before agreeing to fetch it. -->
        <p class="mt-1 min-w-0 break-all font-mono text-[12px] text-muted-foreground/80">
          {{ command(e) }}
        </p>
      </div>

      <p class="text-[12px] text-muted-foreground">{{ t('runtimes.latestHint') }}</p>
      <p class="text-[12px] text-muted-foreground">{{ t('runtimes.runsRemoteCode') }}</p>

      <div class="mt-1 flex items-center gap-3">
        <Button variant="outline" size="sm" :disabled="!!busy" @click="refresh">
          <RefreshCw class="h-3.5 w-3.5" :class="loading ? 'animate-spin' : ''" />
          {{ t('runtimes.refresh') }}
        </Button>
      </div>

      <p v-if="error" class="break-all text-[12px] text-destructive">{{ error }}</p>

      <!-- Read-only, like the run console: this pane shows npm's own output and
           takes no input. -->
      <div class="mt-1">
        <p class="mb-1 text-[13px] text-foreground/85">{{ t('runtimes.log') }}</p>
        <pre
          ref="logEl"
          class="max-h-40 overflow-y-auto whitespace-pre-wrap break-all rounded border border-border bg-background px-2.5 py-2 font-mono text-[12px] leading-relaxed text-foreground/80"
        >{{ logLines.length ? logLines.join("\n") : t('runtimes.logEmpty') }}</pre>
      </div>
    </div>
  </GroupBox>
</template>
