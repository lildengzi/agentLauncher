<script setup lang="ts">
// 选择图标 — Prism 的图标选择器，一格一图，图下写名字。
//
// 为什么值得单开一个对话框：图标以前是一个文本框，要用户自己背 lucide 的名字（"bot" /
// "flask-conical"），猜错就静默变成那只灰机器人。能看见的东西不该靠背名字来选。
//
// 网格的两半，都来自 `lib/brand.ts` 和 `lib/glyphs.ts`：
//   上半 品牌 — simple-icons 的官方标记，按自己的品牌色画（DeepSeek / Claude / OpenAI…）
//   下半 图形 — 一份挑过的 lucide 字形，不是全部一千多个：那不叫选择器，叫草堆
//
// 存的是什么：品牌存 `brand:<slug>`，字形存裸名字。老实例存的一直是裸名字，所以不需要
// 迁移；`brand:` 认不出来时和认不出的字形走同一条兜底路（Bot），配置手改坏了也画得出来。
//
// 这里没有「添加图标 / 移除图标 / 打开文件夹」——Prism 有，因为它能从磁盘读 png。后端还
// 没有那条命令，摆一个点不进去的按钮正是这一轮要改掉的毛病。
import { computed, ref, watch } from "vue";
import Dialog from "@/components/ui/Dialog.vue";
import Button from "@/components/ui/Button.vue";
import Input from "@/components/ui/Input.vue";
import AppIcon from "@/components/ui/AppIcon.vue";
import { BRAND_CHOICES } from "@/lib/brand";
import type { Brand } from "@/lib/brand";
import { GLYPH_CHOICES } from "@/lib/glyphs";
import { useI18n } from "@/lib/i18n";

const { t } = useI18n();
const props = defineProps<{
  /** 打开时高亮哪一格。可以是 `brand:<slug>`，也可以是裸 lucide 名。 */
  current: string;
}>();
const open = defineModel<boolean>("open", { default: false });
const emit = defineEmits<{ picked: [icon: string] }>();

interface Choice {
  id: string;
  /** 格子下面那行字。品牌用官方名字，字形用它自己的 lucide 名——名字本身就是它的身份。 */
  label: string;
  /** 有标记就按品牌色画矢量，没有就当成 lucide 字形。 */
  brand: Brand | null;
  /** 搜索用的小写串。 */
  hay: string;
}

const brands = computed<Choice[]>(() =>
  BRAND_CHOICES.map((c) => ({
    id: c.id,
    label: c.brand.title,
    brand: c.brand,
    hay: `${c.brand.title} ${c.id}`.toLowerCase(),
  }))
);
const glyphs = computed<Choice[]>(() =>
  GLYPH_CHOICES.map((g) => ({ id: g, label: g, brand: null, hay: g }))
);

const search = ref("");
/** 选中格。确定之前不往外发一个字：取消要真的能取消。 */
const sel = ref("");

const q = computed(() => search.value.trim().toLowerCase());
function hit(list: Choice[]): Choice[] {
  return q.value ? list.filter((c) => c.hay.includes(q.value)) : list;
}
const shownBrands = computed(() => hit(brands.value));
const shownGlyphs = computed(() => hit(glyphs.value));
const empty = computed(() => !shownBrands.value.length && !shownGlyphs.value.length);

// 每次打开都从当前值开始，搜索框清空：上一次的搜索词跟着对话框走会挡住整个网格。
watch(
  open,
  (isOpen) => {
    if (!isOpen) return;
    search.value = "";
    sel.value = props.current;
  },
  { immediate: true }
);

function pick(id: string, confirmIt = false): void {
  sel.value = id;
  if (confirmIt) confirm();
}

function confirm(): void {
  if (!sel.value) return;
  emit("picked", sel.value);
  open.value = false;
}
</script>

<template>
  <Dialog v-model:open="open" width="max-w-2xl" class="h-[72vh]" :title="t('icon.title')">
    <div class="flex h-full min-h-0 flex-col">
      <div class="shrink-0 border-b border-border bg-toolbar px-3 py-2">
        <Input v-model="search" class="h-7" :placeholder="t('icon.search')" />
      </div>

      <div class="min-h-0 flex-1 overflow-y-auto px-3 py-2">
        <template v-for="section in [
          { key: 'brand', title: t('icon.brands'), items: shownBrands },
          { key: 'glyph', title: t('icon.glyphs'), items: shownGlyphs },
        ]" :key="section.key">
          <template v-if="section.items.length">
            <p class="mb-1.5 mt-1 text-[12px] text-muted-foreground">{{ section.title }}</p>
            <div class="mb-2 grid grid-cols-[repeat(auto-fill,minmax(84px,1fr))] gap-1">
              <!-- 一格＝一次单选，双击直接确定：网格里最常见的两种手势都要认。 -->
              <button
                v-for="c in section.items"
                :key="c.id"
                type="button"
                class="flex flex-col items-center gap-1 rounded-sm border px-1 py-2"
                :class="
                  c.id === sel
                    ? 'border-border-strong bg-selection text-selection-foreground'
                    : 'border-transparent hover:bg-accent'
                "
                :title="c.label"
                @click="pick(c.id)"
                @dblclick="pick(c.id, true)"
              >
                <!-- 品牌按自己的颜色画，字形随文字色：一个是标识，一个是图形。 -->
                <svg
                  v-if="c.brand"
                  width="32"
                  height="32"
                  viewBox="0 0 24 24"
                  role="img"
                  :aria-label="c.label"
                >
                  <path :d="c.brand.path" :fill="`#${c.brand.hex}`" />
                </svg>
                <AppIcon v-else :name="c.id" class="h-8 w-8" />
                <span class="w-full truncate text-center text-[11px] leading-tight">
                  {{ c.label }}
                </span>
              </button>
            </div>
          </template>
        </template>
        <p v-if="empty" class="py-8 text-center text-[13px] text-muted-foreground">
          {{ t("icon.noMatch") }}
        </p>
      </div>
    </div>

    <template #footer>
      <p class="min-w-0 flex-1 truncate font-mono text-[12px] text-muted-foreground">
        {{ sel }}
      </p>
      <Button variant="ghost" @click="open = false">{{ t("common.cancel") }}</Button>
      <Button variant="primary" :disabled="!sel" @click="confirm">
        {{ t("common.confirm") }}
      </Button>
    </template>
  </Dialog>
</template>
