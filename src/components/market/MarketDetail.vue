<script setup lang="ts">
// The right-hand detail pane of the market dialog.
//
// Two rules shape this file, both about the same thing — the payload is written by
// whoever runs the feed, not by us:
//
//  * Nothing is rendered as markup. The README arrives as Markdown and is shown as
//    pre-wrapped **text**; `v-html` here would hand a third-party feed script
//    execution inside a desktop app that can read the filesystem. Losing the
//    heading formatting is the cheaper trade.
//  * `homepage` / `repo` are not `<a href>`. The parent has already dropped
//    anything that is not http(s) and opens the survivors through `api.openUrl`,
//    so the webview never navigates itself somewhere a feed chose.
import { computed } from "vue";
import AppIcon from "@/components/ui/AppIcon.vue";
import { useI18n } from "@/lib/i18n";
import type { InstallSpec, MarketItem } from "@/types";

const { t } = useI18n();

const props = defineProps<{
  item: MarketItem;
  /** install spec of the version picked in the footer, null when there is none. */
  spec: InstallSpec | null;
  /** the spec's method is one the launcher cannot run ⇒ copy-a-command instead. */
  manual: boolean;
  readme: string;
  readmeLoading: boolean;
  readmeError: string;
  /** already filtered to http(s) by the parent. */
  links: { label: string; url: string }[];
}>();
defineEmits<{ open: [url: string] }>();

/** A feed's `updated_at` is meant to be RFC3339; when it is not, show it verbatim
 *  rather than the word "Invalid Date". */
function day(iso: string): string {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : d.toLocaleDateString();
}

const meta = computed(() =>
  [
    { label: t("market.author"), value: props.item.author },
    { label: t("market.license"), value: props.item.license },
    {
      label: t("market.downloads"),
      value: props.item.downloads ? props.item.downloads.toLocaleString() : "",
    },
    {
      label: t("market.updated"),
      value: props.item.updated_at ? day(props.item.updated_at) : "",
    },
  ].filter((row) => row.value.trim() !== "")
);

const tags = computed(() => props.item.tags.filter((tag) => tag.trim() !== ""));

// Variable *names* only. The launcher never reads, stores or displays a value for
// one of these — that belongs in the instance's own `.env`.
const envNames = computed(() => (props.spec?.env ?? []).filter((n) => n.trim() !== ""));
</script>

<template>
  <div class="flex flex-col gap-3 p-4">
    <div class="flex items-start gap-3">
      <AppIcon :name="item.icon || 'package'" class="mt-0.5 h-7 w-7 shrink-0" />
      <div class="min-w-0">
        <h3 class="break-words text-[15px] font-semibold text-foreground">{{ item.name }}</h3>
        <p v-if="item.author" class="truncate text-[13px] text-muted-foreground">
          {{ item.author }}
        </p>
      </div>
    </div>

    <p v-if="item.description" class="break-words text-[14px] leading-relaxed text-foreground/90">
      {{ item.description }}
    </p>

    <div v-if="meta.length" class="grid grid-cols-[72px_1fr] gap-x-3 gap-y-1 text-[13px]">
      <template v-for="row in meta" :key="row.label">
        <span class="text-muted-foreground">{{ row.label }}</span>
        <span class="min-w-0 break-words text-foreground/90">{{ row.value }}</span>
      </template>
    </div>

    <div v-if="tags.length" class="flex flex-wrap items-center gap-1 text-[12px]">
      <span class="text-muted-foreground">{{ t("market.tags") }}</span>
      <span
        v-for="tag in tags"
        :key="tag"
        class="max-w-[9rem] truncate rounded bg-accent/60 px-1.5 py-0.5 text-foreground/80"
      >
        {{ tag }}
      </span>
    </div>

    <div v-if="links.length" class="flex flex-col items-start gap-1 text-[13px]">
      <button
        v-for="link in links"
        :key="link.url"
        type="button"
        class="text-link hover:underline"
        @click="$emit('open', link.url)"
      >
        {{ link.label }}
      </button>
    </div>

    <!-- What installing this actually needs from the user. Shown before the README
         because it is the part that decides whether the footer button works. -->
    <div
      v-if="manual || envNames.length"
      class="flex flex-col gap-1.5 border-t border-border pt-3 text-[13px]"
    >
      <template v-if="manual">
        <p class="text-muted-foreground">{{ t("market.manualHint") }}</p>
        <p
          v-if="spec && spec.command"
          class="whitespace-pre-wrap break-all rounded border border-border bg-muted px-2 py-1.5 font-mono text-[12px] text-foreground/90"
        >
          {{ spec.command }}
        </p>
      </template>
      <template v-if="envNames.length">
        <p class="text-foreground/90">{{ t("market.needsEnv") }}</p>
        <p class="break-all font-mono text-[12px] text-foreground/80">
          {{ envNames.join(", ") }}
        </p>
        <p class="text-muted-foreground">{{ t("market.needsEnvHint") }}</p>
      </template>
    </div>

    <div class="border-t border-border pt-3">
      <p v-if="readmeLoading" class="text-[13px] text-muted-foreground">
        {{ t("market.loading") }}
      </p>
      <template v-else-if="readmeError">
        <p class="text-[13px] text-destructive">{{ t("market.loadError") }}</p>
        <p class="mt-1 break-words text-[12px] text-muted-foreground">{{ readmeError }}</p>
      </template>
      <p
        v-else-if="readme.trim()"
        class="whitespace-pre-wrap break-words text-[13px] leading-relaxed text-foreground/85"
      >
        {{ readme }}
      </p>
      <p v-else class="text-[13px] text-muted-foreground">{{ t("market.noReadme") }}</p>
    </div>
  </div>
</template>
