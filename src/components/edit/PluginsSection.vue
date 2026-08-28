<script setup lang="ts">
// Owned by Stream B (编辑页承接插件).
//
// Shared contract for all three extension sections, because
// EditInstanceDialog.vue does the one `api.readInstanceExtensions` call and
// passes the result down:
//   :instance-id  — "" while creating an unsaved instance ⇒ show `ext.saveFirst`
//   :extensions   — the whole InstanceExtensions, or null while loading/failed
//   :loading      — a read is in flight
//   @changed      — this section wrote something; parent re-reads
//   @browse       — open the market dialog for this section's kind
//
// This is a read-only list plus a removal, because plugins are not this
// instance's to own: for dsh they are the pnpm dependencies of
// `~/.dsh/profiles/<p>/package.json`, so every instance on that profile sees the
// same set. `plugin_scope` is therefore framing, not decoration — it decides
// whether there is a list at all, and it names the profile the list came from so
// the shared consequence is on screen before anyone removes anything.
import { computed, ref } from "vue";
import { Store, RefreshCw, Trash2 } from "lucide-vue-next";
import ExtSection from "@/components/edit/ExtSection.vue";
import Button from "@/components/ui/Button.vue";
import { api } from "@/lib/api";
import { useI18n } from "@/lib/i18n";
import type { ExtensionKind, InstanceExtensions } from "@/types";

const { t } = useI18n();
const props = defineProps<{
  instanceId: string;
  extensions: InstanceExtensions | null;
  loading: boolean;
}>();
const emit = defineEmits<{ changed: []; browse: [kind: ExtensionKind] }>();

// `plugin_scope` is `"dsh-profile:<name>"` or `"unsupported"`. Parsing the prefix
// rather than re-deriving the profile from the form: the backend read one
// specific profile's package.json, and this must name that one, not whatever the
// Runtime dropdown currently shows.
const PROFILE_PREFIX = "dsh-profile:";
const scope = computed(() => props.extensions?.plugin_scope ?? "");
const profile = computed(() =>
  scope.value.startsWith(PROFILE_PREFIX) ? scope.value.slice(PROFILE_PREFIX.length) : ""
);
const supported = computed(() => profile.value !== "");
const plugins = computed(() => props.extensions?.plugins ?? []);

const failed = computed(() => !props.loading && props.extensions === null);
const error = ref("");
// Two-step removal in the row itself. The consequence — this uninstalls the
// package from the profile, for every instance on it — is already stated by the
// notice above the list, and there is no `ext.plugins.confirmRemove` string to
// put in a modal, so the button pair is the question.
const pending = ref("");
const busy = ref("");

async function remove(pkg: string): Promise<void> {
  busy.value = pkg;
  error.value = "";
  try {
    // `pnpm-profile` uninstall is `dsh plugin --profile <p> remove <pkg>` — the
    // engine's own command, which is why it is also right for a package the user
    // installed by hand: it edits the profile's dependencies either way.
    await api.marketUninstall(props.instanceId, pkg, {
      method: "pnpm-profile",
      package: pkg,
      repo: "",
      command: "",
      env: [],
      mcp: null,
    });
    emit("changed");
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    busy.value = "";
    pending.value = "";
  }
}
</script>

<template>
  <ExtSection
    :title="t('ext.plugins.title')"
    :desc="t('ext.plugins.desc')"
    :loading="loading"
    :failed="failed"
    :empty="supported && plugins.length === 0"
    :empty-label="t('ext.plugins.empty')"
    :error="error"
    @retry="emit('changed')"
  >
    <template #actions>
      <!-- Nothing to browse for an engine with no readable plugin set: an install
           affordance that cannot lead anywhere is worse than its absence. -->
      <template v-if="supported">
        <Button variant="outline" size="sm" @click="emit('browse', 'plugin')">
          <Store class="h-3.5 w-3.5" />
          {{ t("ext.browse") }}
        </Button>
        <Button variant="ghost" size="sm" :title="t('ext.refresh')" @click="emit('changed')">
          <RefreshCw class="h-3.5 w-3.5" />
        </Button>
      </template>
    </template>

    <template #notice>
      <p v-if="supported" class="mt-3 text-[13px] text-muted-foreground">
        {{ t("ext.plugins.scopeShared") }}
        <span class="font-mono text-foreground/90">{{ profile }}</span>
      </p>
      <p v-else class="mt-3 text-[13px] text-muted-foreground">
        {{ t("ext.plugins.scopeUnsupported") }}
      </p>
    </template>

    <!-- Guarded, not just left to `v-for`: an unsupported scope has no list and
         must not leave an empty framed box behind to suggest one. -->
    <ul
      v-if="plugins.length"
      class="mt-3 divide-y divide-border rounded border border-border"
    >
      <li
        v-for="pkg in plugins"
        :key="pkg"
        class="flex items-center gap-3 px-3 py-2 text-[14px]"
      >
        <span class="min-w-0 flex-1 truncate font-mono">{{ pkg }}</span>
        <template v-if="pending === pkg">
          <Button
            variant="destructive"
            size="sm"
            :disabled="busy === pkg"
            @click="remove(pkg)"
          >
            {{ t("common.confirm") }}
          </Button>
          <Button variant="ghost" size="sm" @click="pending = ''">
            {{ t("common.cancel") }}
          </Button>
        </template>
        <Button
          v-else
          variant="ghost"
          size="sm"
          :title="t('common.remove')"
          :disabled="busy !== ''"
          @click="pending = pkg"
        >
          <Trash2 class="h-3.5 w-3.5" />
        </Button>
      </li>
    </ul>
  </ExtSection>
</template>
