<script setup lang="ts">
// The shell the three extension sections share: GroupBox → description line →
// content. Factored out because the three are one Prism dialog page repeated,
// and three hand-rolled copies would drift apart in exactly the places that
// matter — where "still reading", "could not read" and "genuinely empty" are
// told apart. None of the three may show a fabricated row in place of an honest
// empty state, so that branch lives here once.
import GroupBox from "@/components/ui/GroupBox.vue";
import Button from "@/components/ui/Button.vue";
import { useI18n } from "@/lib/i18n";

const { t } = useI18n();

withDefaults(
  defineProps<{
    title: string;
    desc: string;
    /** a read is in flight */
    loading: boolean;
    /** the read finished without a result — a failure, not an empty list */
    failed: boolean;
    /** the read succeeded and there is nothing to list */
    empty: boolean;
    emptyLabel: string;
    /** a write failed; surfaced here rather than only in the console */
    error?: string;
  }>(),
  { error: "" }
);

defineEmits<{ retry: [] }>();
</script>

<template>
  <GroupBox :title="title">
    <div class="flex items-start justify-between gap-4">
      <p class="text-[13px] text-muted-foreground">{{ desc }}</p>
      <div class="flex shrink-0 items-center gap-2"><slot name="actions" /></div>
    </div>

    <p
      v-if="error"
      class="mt-3 rounded border border-destructive/50 bg-destructive/10 px-3 py-2 text-[13px] text-destructive"
    >
      {{ error }}
    </p>

    <p v-if="loading" class="mt-3 text-[13px] text-muted-foreground">
      {{ t("ext.loading") }}
    </p>
    <div v-else-if="failed" class="mt-3 flex items-center gap-3">
      <p class="text-[13px] text-destructive">{{ t("ext.loadError") }}</p>
      <Button variant="outline" size="sm" @click="$emit('retry')">
        {{ t("common.retry") }}
      </Button>
    </div>
    <template v-else>
      <!-- Scope / provenance copy, shown even when the list is empty: whose
           plugins these would be is as much of an answer as the list itself. -->
      <slot name="notice" />
      <p v-if="empty" class="mt-3 text-[13px] text-muted-foreground">{{ emptyLabel }}</p>
      <slot v-else />
      <slot name="footer" />
    </template>
  </GroupBox>
</template>
