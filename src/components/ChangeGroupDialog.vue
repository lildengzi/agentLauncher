<script setup lang="ts">
// 改变分组 — Prism's Change Group dialog: move the selected instance into a group
// that already exists, or name a new one on the spot.
//
// The invariant this dialog exists to protect: membership belongs to the instance
// itself (`instances/<id>/instance.json`'s `group`), so the single write here is
// `api.updateInstance`. `instgroups.json` is a presentation overlay — display
// order, collapse state, manual intra-group order — and is deliberately left
// untouched: a group nobody has seen before still needs no entry there, because
// `applyOverlay` appends unknown groups by name instead of hiding their members.
import { computed, ref, watch } from "vue";
import Dialog from "@/components/ui/Dialog.vue";
import Button from "@/components/ui/Button.vue";
import Input from "@/components/ui/Input.vue";
import Label from "@/components/ui/Label.vue";
import Select, { type SelectOption } from "@/components/ui/Select.vue";
import GroupBox from "@/components/ui/GroupBox.vue";
import { api } from "@/lib/api";
import { useI18n } from "@/lib/i18n";
import type { Instance } from "@/types";

const { t } = useI18n();

const props = defineProps<{ instance: Instance | null; groups: string[] }>();
const open = defineModel<boolean>("open", { default: false });
const emit = defineEmits<{ saved: [instance: Instance] }>();

// Where an instance lands when nothing was chosen. A literal name rather than an
// empty string, matching EditInstanceDialog's defaults(): an empty `group` would
// surface in the sidebar as a nameless section.
const UNGROUPED = "未分类";

const picked = ref("");
const typed = ref("");
const errorBanner = ref("");
const saving = ref(false);

watch(
  () => [open.value, props.instance] as const,
  ([isOpen]) => {
    if (!isOpen) return;
    errorBanner.value = "";
    picked.value = props.instance?.group ?? "";
    typed.value = "";
  },
  { immediate: true }
);

const options = computed<SelectOption[]>(() =>
  props.groups.map((g) => ({ value: g, label: g }))
);

// Only ever one of the two controls holds the answer: picking from the list drops
// a half-typed name and typing clears the selection, so the answer is always the
// one the user can see. Keeping both filled would need a precedence rule that is
// invisible on screen.
watch(picked, (v) => {
  if (v) typed.value = "";
});
watch(typed, (v) => {
  if (v) picked.value = "";
});

const target = computed(() => typed.value.trim() || picked.value || UNGROUPED);
const unchanged = computed(
  () => !!props.instance && target.value === props.instance.group
);

async function save(): Promise<void> {
  const instance = props.instance;
  if (!instance) return;
  // A confirm that changes nothing still closes, but writing instance.json and
  // making App.vue re-read the whole list to learn nothing is not worth it.
  if (unchanged.value) {
    open.value = false;
    return;
  }
  errorBanner.value = "";
  saving.value = true;
  try {
    const result = await api.updateInstance({ ...instance, group: target.value });
    emit("saved", result);
    open.value = false;
  } catch (err) {
    errorBanner.value = err instanceof Error ? err.message : String(err);
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <Dialog v-model:open="open" width="max-w-md" :title="t('group.title')">
    <div class="px-5 pt-4">
      <div
        v-if="errorBanner"
        class="mb-4 rounded border border-destructive/50 bg-destructive/10 px-3 py-2 text-[14px] text-destructive"
      >
        {{ errorBanner }}
      </div>
      <p class="mb-4 text-[13px] text-muted-foreground">{{ t("group.desc") }}</p>

      <GroupBox :title="t('group.field')">
        <div class="grid grid-cols-[120px_1fr] items-start gap-x-3 gap-y-3 [&>label]:pt-2">
          <Label for="grp-existing">{{ t("group.existing") }}</Label>
          <Select
            id="grp-existing"
            v-model="picked"
            :options="options"
            :disabled="!props.groups.length"
            :placeholder="t('group.none')"
          />

          <Label for="grp-new">{{ t("group.newLabel") }}</Label>
          <Input id="grp-new" v-model="typed" :placeholder="t('group.placeholder')" />
        </div>
        <p v-if="unchanged" class="mt-2.5 text-[13px] text-muted-foreground">
          {{ t("group.unchanged") }}
        </p>
      </GroupBox>
    </div>

    <template #footer>
      <Button variant="ghost" @click="open = false">{{ t("common.cancel") }}</Button>
      <div class="flex-1" />
      <Button variant="primary" :disabled="saving || !props.instance" @click="save">
        {{ t("common.confirm") }}
      </Button>
    </template>
  </Dialog>
</template>
