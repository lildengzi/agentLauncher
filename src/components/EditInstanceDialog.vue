<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { Save, SlidersHorizontal, BrainCircuit, ListChecks, Wrench, Puzzle, GraduationCap, Plug, ScrollText, KeyRound } from "lucide-vue-next";
import Dialog from "@/components/ui/Dialog.vue";
import DialogPanel from "@/components/ui/DialogPanel.vue";
import Button from "@/components/ui/Button.vue";
import Input from "@/components/ui/Input.vue";
import Textarea from "@/components/ui/Textarea.vue";
import Label from "@/components/ui/Label.vue";
import Select, { type SelectOption } from "@/components/ui/Select.vue";
import GroupBox from "@/components/ui/GroupBox.vue";
import Avatar from "@/components/ui/Avatar.vue";
import IconPickerDialog from "@/components/IconPickerDialog.vue";
import PluginsSection from "@/components/edit/PluginsSection.vue";
import SkillsSection from "@/components/edit/SkillsSection.vue";
import McpSection from "@/components/edit/McpSection.vue";
import AgentsSection from "@/components/edit/AgentsSection.vue";
import MarketDialog from "@/components/market/MarketDialog.vue";
import { api } from "@/lib/api";
import { FALLBACK_ENGINES } from "@/lib/engineList";
import { useI18n } from "@/lib/i18n";
import { config } from "@/lib/launcherConfig";
import type {
  DshProfile,
  EngineInfo,
  ExtensionKind,
  Instance,
  InstanceExtensions,
  InstanceKeyView,
  NewInstance,
  ProviderView,
} from "@/types";

const { t } = useI18n();

const props = defineProps<{
  /** An instance that already exists on disk — never null.
   *
   *  This dialog used to double as 新建实例 with `instance = null`, and four of its
   *  eight pages (扩展插件 / 技能 / MCP / 人设与契约) could then only say「先保存实例」:
   *  they read and write files under `instances/<id>/`, which an unsaved instance has
   *  not got. Half the nav was dead on arrival. Creating moved to
   *  `NewInstanceDialog.vue`, and the non-null type is what keeps it there. */
  instance: Instance;
  /** Render as the whole content of its own OS window instead of as a modal.
   *  Only the container changes: the panel fills the window, the title lives in
   *  the OS titlebar (so no header row and no redundant X), and `open = false`
   *  means "this window is done" rather than "hide the overlay". */
  inline?: boolean;
}>();
const open = defineModel<boolean>("open", { default: false });
const emit = defineEmits<{ saved: [instance: Instance] }>();

type Section = "general" | "model" | "runtime" | "extensions" | "skills" | "mcp" | "agents" | "task";
const section = ref<Section>("general");

// Known engines, used as a fallback when live detection fails so the picker is
// always usable. Shared with NewInstanceDialog so the two cannot drift.
const engines = ref<EngineInfo[]>(FALLBACK_ENGINES);

// Real dsh profiles discovered under ~/.dsh/profiles (fallback to the built-ins).
// `web` comes from the backend, which reads the profile's bundled packages — the
// name alone does not decide it.
const profiles = ref<DshProfile[]>([
  { name: "headless", web: false },
  { name: "web", web: true },
]);
/** The provider list, for the key-binding picker. Read-only here — this dialog
 *  binds a *reference*; the rows themselves are edited in Settings. Keys arrive
 *  as fingerprints, so nothing secret enters this component either. */
const providers = ref<ProviderView[]>([]);
function loadProviders() {
  api
    .getProviders()
    .then((found) => (providers.value = found))
    .catch(() => (providers.value = []));
}
/** dsh 的 provider **route** 列表（dsh 自己的命名空间，不是上面那份服务商 id）。
 *  原生适配器永远有 deepseek-official，其余来自 $DSH_HOME/settings.yaml，所以这份
 *  列表是从盘上读的、会变的东西，不能写死。 */
const dshRoutes = ref<string[]>([]);
function loadDshRoutes() {
  api
    .listDshModelRoutes()
    .then((found) => (dshRoutes.value = found))
    .catch(() => (dshRoutes.value = []));
}
watch(
  open,
  async (v) => {
    if (!v) return;
    // Probe installed engines fresh each time the dialog opens (never cached).
    api
      .detectEngines()
      .then((found) => {
        if (found.length) engines.value = found;
      })
      .catch(() => {
        /* keep fallback */
      });
    loadProviders();
    loadDshRoutes();
    try {
      const found = await api.listDshProfiles();
      if (found.length) profiles.value = found;
    } catch {
      /* keep fallback */
    }
  },
  // Immediate because in a standalone editor window `open` starts true and never
  // changes, so a change-only watcher would never probe at all. The `!v` guard
  // above makes the extra call a no-op for the modal case.
  { immediate: true }
);
// 管理密钥…把用户送去另一个窗口改 providers.json；回到这个窗口时那份列表已经旧了，
// 而它旧了正是「加了密钥却还是没有可用密钥」的样子。所以重新获得焦点就重读一次——
// 和主窗口用 focus 重读实例列表同一个理由，都是别的窗口在背后改了盘上的东西。
// dsh 的路由表同理：它由 dsh 网页版的 Models 页写，用户很可能是刚去那边加了一条
// 才切回来。
function reloadDiskLists() {
  loadProviders();
  loadDshRoutes();
}
onMounted(() => window.addEventListener("focus", reloadDiskLists));
onBeforeUnmount(() => window.removeEventListener("focus", reloadDiskLists));

interface FormState {
  name: string;
  icon: string;
  group: string;
  description: string;
  engine: string;
  profile: string;
  provider: string;
  model: string;
  env_policy: string;
  /** "" = follow the engine's own default | "interactive" | "task". */
  mode: string;
  custom_bin: string;
  default_task: string;
}

function defaults(): FormState {
  // Prefill a new instance from the launcher-wide defaults + last used group.
  return {
    name: "",
    icon: "bot",
    group: config.session.last_used_group || "未分类",
    description: "",
    engine: "dsh",
    profile: "headless",
    provider: config.defaults.provider || "",
    // Empty = the selected framework's own default (see "空值即省略" contract).
    // The launcher-wide default is owned by config.defaults, not hardcoded here.
    model: config.defaults.model || "",
    env_policy: "autodetect",
    // Empty on purpose: "ask the engine" is the honest default, and it is what
    // every instance.json written before this field existed already says.
    mode: "",
    custom_bin: "",
    default_task: "",
  };
}

const form = reactive<FormState>(defaults());
const nameError = ref("");
const errorBanner = ref("");
const saving = ref(false);
/** 选择图标对话框。图标存 `brand:<slug>` 或裸 lucide 名，见 lib/brand.ts。 */
const iconPicking = ref(false);

watch(
  () => [open.value, props.instance] as const,
  ([isOpen]) => {
    if (!isOpen) return;
    nameError.value = "";
    errorBanner.value = "";
    section.value = "general";
    form.name = props.instance.name;
    form.icon = props.instance.icon;
    form.group = props.instance.group;
    form.description = props.instance.description;
    form.engine = props.instance.runtime?.engine || "dsh";
    form.profile = props.instance.profile;
    form.provider = props.instance.provider || "";
    form.model = props.instance.model;
    form.env_policy = props.instance.runtime?.env_policy || "autodetect";
    form.mode = props.instance.runtime?.mode || "";
    form.custom_bin = props.instance.runtime?.custom_bin || "";
    form.default_task = props.instance.default_task;
  },
  { immediate: true }
);

async function save() {
  nameError.value = "";
  errorBanner.value = "";

  if (!form.name.trim()) {
    nameError.value = "名称不能为空";
    section.value = "general";
    return;
  }

  // 密钥先写。它是这一页唯一动实例目录的东西，而它会失败的两种方式（变量名不合法、
  // 值里有换行）都是能改的输入错误——这时 instance.json 还没被碰过。
  if (keyHome.value === "instance" && keyDraft.value.trim()) {
    if (!keyVar.value) {
      errorBanner.value = t("edit.model.keyVarUnknown");
      section.value = "model";
      return;
    }
    try {
      await api.setInstanceKey(props.instance.id, keyVar.value, keyDraft.value.trim());
      keyDraft.value = "";
      loadInstanceKey();
    } catch (err) {
      errorBanner.value = err instanceof Error ? err.message : String(err);
      section.value = "model";
      return;
    }
  }

  const payload: NewInstance = {
    name: form.name,
    icon: form.icon,
    group: form.group,
    description: form.description,
    profile: form.profile,
    provider: form.provider.trim(),
    model: form.model,
    // 「密钥存放」只有选「用启动器保存的」时才落成一个引用；另外两档是空——空就是
    // 「启动器不从库里注入」，实例 .env 与引擎自己的凭据由后端那三层决定。
    api_key_ref: keyHome.value === "shared" ? sharedRef.value.trim() : "",
    runtime: {
      engine: form.engine,
      env_policy: form.env_policy,
      mode: form.mode,
      custom_bin: form.custom_bin.trim(),
    },
    default_task: form.default_task,
  };

  saving.value = true;
  try {
    const result = await api.updateInstance({
      ...props.instance,
      ...payload,
      id: props.instance.id,
      created_at: props.instance.created_at,
    });
    emit("saved", result);
    open.value = false;
  } catch (err) {
    errorBanner.value = err instanceof Error ? err.message : String(err);
  } finally {
    saving.value = false;
  }
}

const navItems = [
  { key: "general" as const, icon: SlidersHorizontal, label: () => t("edit.nav.general") },
  { key: "model" as const, icon: BrainCircuit, label: () => t("edit.nav.model") },
  { key: "runtime" as const, icon: Wrench, label: () => t("edit.nav.runtime") },
  { key: "extensions" as const, icon: Puzzle, label: () => t("edit.nav.extensions") },
  { key: "skills" as const, icon: GraduationCap, label: () => t("edit.nav.skills") },
  { key: "mcp" as const, icon: Plug, label: () => t("edit.nav.mcp") },
  // After MCP and before 任务, following the spec's own row order
  // (扩展插件 → Skills → MCP → 备注/人设与契约).
  { key: "agents" as const, icon: ScrollText, label: () => t("edit.nav.agents") },
  { key: "task" as const, icon: ListChecks, label: () => t("edit.nav.task") },
];

// ---- extensions (plugins / skills / MCP) ---------------------------------
// One read for all three sections: they are three views of one instance's
// extension state, and three independent fetches would let them disagree.
const instanceId = computed(() => props.instance.id);
const extensions = ref<InstanceExtensions | null>(null);
const extLoading = ref(false);
const marketOpen = ref(false);
const marketKind = ref<ExtensionKind>("plugin");

async function loadExtensions(): Promise<void> {
  extLoading.value = true;
  try {
    // Ask about the form as it stands, not as it was saved: the engine and
    // profile pickers decide which plugin set is in scope, and a user who has
    // just switched them is looking at the new one.
    extensions.value = await api.readInstanceExtensions(
      instanceId.value,
      form.engine,
      form.profile
    );
  } catch (e) {
    extensions.value = null;
    console.error("read instance extensions failed", e);
  } finally {
    extLoading.value = false;
  }
}
watch(
  () => [open.value, instanceId.value] as const,
  ([isOpen]) => {
    if (isOpen) void loadExtensions();
  },
  { immediate: true }
);
// Both pickers change which plugin set is in scope, so re-read on the spot rather
// than showing the previous selection's list under the new name.
watch(
  () => [form.engine, form.profile] as const,
  () => {
    if (open.value) void loadExtensions();
  }
);

function browseMarket(kind: ExtensionKind): void {
  marketKind.value = kind;
  marketOpen.value = true;
}

// The dsh `profile` concept only applies to the dsh engine; other engines hide it.
const isDsh = computed(() => form.engine === "dsh");
// Live install status of the currently selected engine, for a hint line.
const selectedEngine = computed(
  () => engines.value.find((e) => e.id === form.engine) ?? null
);
// Whether the selected engine actually receives a provider on the command line.
// claude does not (ANTHROPIC_* env only), so the field is hidden rather than
// silently dropped. Unknown engines default to showing it.
const takesProvider = computed(() => selectedEngine.value?.takes_provider ?? true);

// Whether this instance serves a browser UI: only when the engine has a web mode
// *and* the chosen profile bundles it (the same judgement the backend's `is_serve`
// makes). That combination overrides the run mode below, because there is nothing
// else such a launch could mean.
const runsWeb = computed(() => {
  if (!selectedEngine.value?.web) return false;
  return profiles.value.find((p) => p.name === form.profile)?.web ?? false;
});

// Labels sit on the first line of their cell: rows whose field carries a hint are
// taller than the label, and centering made the label drift down beside the hint.
const rowGrid =
  "grid grid-cols-[120px_1fr] items-start gap-x-3 gap-y-3 [&>label]:pt-2";

// Dropdown option lists. Engine rows carry an install-status hint so a missing
// CLI is visible in the closed trigger too, not just in the open list.
const engineOptions = computed<SelectOption[]>(() =>
  engines.value.map((e) => ({
    value: e.id,
    label: e.display,
    hint: e.installed ? undefined : t("edit.runtime.engineMissing"),
    warn: !e.installed,
  }))
);
const profileOptions = computed<SelectOption[]>(() =>
  profiles.value.map((p) => ({
    value: p.name,
    label: p.name,
    hint: p.web ? t("edit.runtime.shape.web") : t("edit.runtime.shape.headless"),
  }))
);
const envPolicyOptions = computed<SelectOption[]>(() => [
  { value: "autodetect", label: t("edit.runtime.autodetect") },
  { value: "isolated", label: t("edit.runtime.isolated") },
]);
// dsh 的 服务商 是一份可枚举的东西，所以这一栏是下拉而不是文本框：以前它是文本框，
// 而这栏里写下的启动器服务商 id（deepseek / free-api …）正是「模型不可用」的来源——
// dsh 要的是它自己注册过的 route。
//
// 仍然要能显示一个不在列表里的旧值：老实例盘上就写着这种值，而下拉遇到匹配不上的
// v-model 会显示空白，看起来像「没填」而不是「填错了」。所以补一行，并说清它的下场。
const dshRouteOptions = computed<SelectOption[]>(() => {
  const out: SelectOption[] = [
    { value: "", label: t("edit.model.dshRouteAuto") },
    ...dshRoutes.value.map((r) => ({ value: r, label: r })),
  ];
  const cur = form.provider.trim();
  if (cur && !dshRoutes.value.includes(cur)) {
    // `deepseek` 是后端唯一认的别名（启动器的 DeepSeek 行 = dsh 的原生路由），
    // 所以它不是错，只是名字属于另一套。
    const alias = cur === "deepseek";
    out.push({
      value: cur,
      label: cur,
      hint: alias ? t("edit.model.dshRouteAlias") : t("edit.model.dshRouteUnknown"),
      warn: !alias,
    });
  }
  return out;
});
// 「跟随默认」这一行自己说出它这次会落到哪儿——否则用户要读文档才知道自己选了什么。
// 结论与后端 `RunMode::resolve` 同源：web 档一律是 web 服务，dsh 的其余档按档跑一次性
// 任务，其他五个 CLI 开会话。
const modeOptions = computed<SelectOption[]>(() => [
  {
    value: "",
    label: t("edit.runtime.mode.auto"),
    hint: runsWeb.value
      ? t("edit.runtime.shape.web")
      : isDsh.value
        ? t("edit.runtime.mode.task")
        : t("edit.runtime.mode.interactive"),
  },
  { value: "interactive", label: t("edit.runtime.mode.interactive") },
  { value: "task", label: t("edit.runtime.mode.task") },
]);

// ---- 模型页：五格 --------------------------------------------------------
// 服务商 / 模型 / Base URL / 密钥存放 / 密钥。就这五格，别的都不在这一页问。
//
// Base URL 只读：它不是实例的字段，跟着所选服务商那一行走（providers.json）。同一个
// 地址两处都能改就是两份真相，所以这里显示它，不让编辑。

/** 表单里的 provider 对上的启动器服务商行；对不上就是 null——dsh 的路由名、或引擎自己
 *  命名空间里的名字（pi 的 google）都会落到这里。模型列表和 Base URL 都从它来。 */
const providerRow = computed<ProviderView | null>(
  () => providers.value.find((p) => p.id === form.provider.trim()) ?? null
);

/** 「自定义…」这一行的哨兵值。不能用空串——空串是「跟随默认」这个真实取值。
 *
 *  选了它，下拉**留在原地**、文本框长在它下面，这一栏才有回头路。把下拉整个换成文本框
 *  的话，「自定义…」就成了单向门：除了关掉窗口重开，回不到列表。 */
const CUSTOM = "__custom__";
const providerCustom = ref(false);
const modelCustom = ref(false);

const providerOptions = computed<SelectOption[]>(() => {
  const cur = form.provider.trim();
  const out: SelectOption[] = [{ value: "", label: t("edit.model.providerAuto") }];
  for (const p of providers.value) {
    if (!p.enabled && p.id !== cur) continue;
    out.push({ value: p.id, label: p.label || p.id });
  }
  // 盘上写着一个不在库里的名字（引擎自己的命名空间，或删掉的服务商行）：列出来，
  // 否则下拉匹配不上会显示空白，看着像「没填」。
  if (cur && !out.some((o) => o.value === cur)) out.push({ value: cur, label: cur });
  out.push({ value: CUSTOM, label: t("edit.model.custom") });
  return out;
});

function pickProvider(v: string): void {
  if (v === CUSTOM) {
    providerCustom.value = true;
    return;
  }
  providerCustom.value = false;
  form.provider = v;
  // 换了服务商，旧模型多半不属于新的这家——只在它确实不在新列表里时才替掉。
  const models = providers.value.find((p) => p.id === v)?.models ?? [];
  if (models.length && !models.includes(form.model.trim())) form.model = models[0];
}

/** 所选服务商报得出模型列表时这一栏是下拉；报不出（自己填的服务商、dsh 的路由）才是
 *  文本框。用户选了「自定义…」不改变这件事——下拉留着，文本框长在它下面。 */
const modelHasList = computed(() => (providerRow.value?.models.length ?? 0) > 0);
const modelOptions = computed<SelectOption[]>(() => {
  const models = providerRow.value?.models ?? [];
  const cur = form.model.trim();
  const out: SelectOption[] = models.map((m) => ({ value: m, label: m }));
  // 存着的模型不在这家的列表里 —— 正是「模型不可用」的样子，所以说出来而不是隐藏。
  if (cur && !models.includes(cur)) {
    out.unshift({ value: cur, label: cur, hint: t("edit.model.modelStale"), warn: true });
  }
  out.push({ value: CUSTOM, label: t("edit.model.custom") });
  return out;
});
function pickModel(v: string): void {
  if (v === CUSTOM) {
    modelCustom.value = true; // 现有文本留着，别把用户的字清了
    return;
  }
  modelCustom.value = false;
  form.model = v;
}

// ---- 密钥存放：三选一，对着后端 `executor::resolve_credentials` 的三层 ----------
//   system   → 不注入，引擎自己的凭据（只有 dsh 真有一个：~/.dsh/.credentials.yaml）
//   shared   → 启动器密钥库 providers.json（存的是引用，不是值）
//   instance → 这个实例自己的 .env，启动时层在最后，所以它压得住上面两层
// 这三个就是用户说的「直接用系统还是实例保管的」，加上启动器自己那份。
type KeyHome = "system" | "shared" | "instance";
const keyHome = ref<KeyHome>("system");
/** 用户动过这一格之后，异步读盘的结果不许再覆盖他的选择。 */
const keyHomeTouched = ref(false);
const keyHomeOptions = computed<SelectOption[]>(() => [
  { value: "system", label: t("edit.model.keyHome.system") },
  { value: "shared", label: t("edit.model.keyHome.shared") },
  { value: "instance", label: t("edit.model.keyHome.instance") },
]);

/** 启动器密钥库里选中的那一行：`<服务商>`（在它启用的密钥间轮换），或盘上留下的
 *  `<服务商>/<别名>`（固定一把）。后者这一页不再新造，但也不偷偷改掉。 */
const sharedRef = ref("");
const sharedOptions = computed<SelectOption[]>(() => {
  const cur = sharedRef.value.trim();
  const out: SelectOption[] = [];
  for (const p of providers.value) {
    const isCur = cur === p.id || cur.startsWith(`${p.id}/`);
    if (!p.enabled && !isCur) continue;
    const usable = p.keys.filter((k) => k.enabled && k.has_value).length;
    out.push({
      value: p.id,
      label: p.label || p.id,
      hint: usable
        ? t("edit.model.keyUsable").replace("{n}", String(usable))
        : t("edit.model.keyNone"),
      warn: !usable,
    });
  }
  if (cur && !out.some((o) => o.value === cur)) {
    out.push({ value: cur, label: cur.replace("/", " / "), hint: t("edit.model.keyPinned") });
  }
  return out;
});

/** 库里一把密钥都没有。得自己喊出来：库是空的时候用户没有理由去点开下拉，于是这一格
 *  看起来只是没选，而不是没得选。 */
const keyStoreEmpty = computed(
  () => !providers.value.some((p) => p.keys.some((k) => k.has_value))
);

/** 这个实例 `.env` 里那把密钥的样子（只有变量名和指纹，值永远不过来）。 */
const instKey = ref<InstanceKeyView>({ var: "", fingerprint: "", has_value: false });
/** 「只给这个实例」那一格里刚输入的值；空＝不动盘上已有的那把。 */
const keyDraft = ref("");

function loadInstanceKey(): void {
  api
    .getInstanceKey(props.instance.id)
    .then((v) => {
      instKey.value = v;
      // 没绑库里的行、但实例自己有一把 → 这个实例本来就是「实例保管」。
      if (!keyHomeTouched.value && keyHome.value !== "shared" && v.has_value) {
        keyHome.value = "instance";
      }
    })
    .catch(() => (instKey.value = { var: "", fingerprint: "", has_value: false }));
}

/** 引擎读密钥的惯例变量名，所选服务商没声明自己的时候兜底。pi / omp 不在表里：它们
 *  按服务商各读各的变量，猜一个只会写错地方。 */
const ENGINE_KEY_VAR: Record<string, string> = {
  dsh: "DEEPSEEK_API_KEY",
  claude: "ANTHROPIC_API_KEY",
  codex: "OPENAI_API_KEY",
  opencode: "OPENAI_API_KEY",
};
/** 「只给这个实例」那把密钥落进哪个变量：盘上已有的那个优先（改的就是它），否则所选
 *  服务商声明的，最后才是引擎惯例。空＝这一格填不了，页面会说为什么。 */
const keyVar = computed(
  () => instKey.value.var || providerRow.value?.api_key_env || ENGINE_KEY_VAR[form.engine] || ""
);

function pickKeyHome(v: string): void {
  keyHomeTouched.value = true;
  keyHome.value = v as KeyHome;
  if (v === "shared" && !sharedRef.value.trim()) {
    const match = providers.value.find((p) => p.id === form.provider.trim());
    const anyWithKey = providers.value.find(
      (p) => p.enabled && p.keys.some((k) => k.enabled && k.has_value)
    );
    sharedRef.value = (match ?? anyWithKey)?.id ?? "";
  }
}

/** 明确地删掉实例自己那把密钥。换「密钥存放」不会顺手删——盘上的东西只在用户说删的
 *  时候才删。 */
async function clearInstanceKey(): Promise<void> {
  if (!instKey.value.var) return;
  try {
    await api.setInstanceKey(props.instance.id, instKey.value.var, "");
    keyDraft.value = "";
    loadInstanceKey();
  } catch (err) {
    errorBanner.value = err instanceof Error ? err.message : String(err);
  }
}

/** 去密钥库。编辑窗口不自带一份：providers.json 只有一个编辑者（主窗口的
 *  设置 → 模型与 API），两个窗口同时存同一个凭据文件就是后写覆盖前写。所以这里
 *  只是把主窗口顶到前面并翻到那一页，由后端决定窗口与焦点。 */
function manageKeys() {
  api.openSettings("model").catch((e) => console.error("open settings failed", e));
}

// 模型页那几格不是实例字段，是从盘上两处推出来的，所以它们有自己的一次重置：绑了库里
// 的行就是「用启动器保存的」，否则等 `.env` 读回来（`loadInstanceKey`）再决定。
// 单独一个 watcher 而不是并进上面那个：上面那个在这些 ref 声明之前就跑了。
watch(
  () => [open.value, props.instance] as const,
  ([isOpen]) => {
    if (!isOpen) return;
    keyHomeTouched.value = false;
    keyDraft.value = "";
    providerCustom.value = false;
    modelCustom.value = false;
    sharedRef.value = props.instance.api_key_ref || "";
    keyHome.value = sharedRef.value.trim() ? "shared" : "system";
    instKey.value = { var: "", fingerprint: "", has_value: false };
    loadInstanceKey();
  },
  { immediate: true }
);
</script>

<template>
  <component
    :is="props.inline ? DialogPanel : Dialog"
    v-model:open="open"
    :width="props.inline ? undefined : 'max-w-2xl'"
    :class="props.inline ? 'h-screen w-screen' : 'h-[72vh]'"
    :title="props.inline ? undefined : t('edit.title.edit')"
  >
    <div class="flex h-full min-h-0">
      <!-- Left nav -->
      <nav class="w-40 shrink-0 border-r border-border bg-toolbar py-2">
        <button
          v-for="item in navItems"
          :key="item.key"
          type="button"
          class="flex w-full items-center gap-2.5 px-3 py-2 text-left text-[14px] transition-colors"
          :class="
            section === item.key
              ? 'bg-selection text-selection-foreground'
              : 'text-foreground/85 hover:bg-accent'
          "
          @click="section = item.key"
        >
          <component :is="item.icon" class="h-4 w-4 shrink-0" :stroke-width="1.75" />
          <span>{{ item.label() }}</span>
        </button>
      </nav>

      <!-- Right content -->
      <div class="min-w-0 flex-1 overflow-y-auto px-5 py-4">
        <div
          v-if="errorBanner"
          class="mb-4 rounded border border-destructive/50 bg-destructive/10 px-3 py-2 text-[14px] text-destructive"
        >
          {{ errorBanner }}
        </div>

        <!-- General -->
        <GroupBox v-if="section === 'general'" :title="t('edit.nav.general')">
          <div :class="rowGrid">
            <Label for="inst-name">{{ t("edit.name") }}</Label>
            <div>
              <Input id="inst-name" v-model="form.name" />
              <p v-if="nameError" class="mt-1 text-[13px] text-destructive">
                {{ nameError }}
              </p>
            </div>

            <Label for="inst-icon">{{ t("edit.icon") }}</Label>
            <div>
              <!-- 点图标进选择器，和 Prism 一样。旁边的文本框留着：手填一个 lucide 名字
                   仍然合法，网格里没收录的字形只能这样进来。 -->
              <div class="flex items-center gap-2">
                <button
                  id="inst-icon"
                  type="button"
                  class="flex h-8 w-8 shrink-0 items-center justify-center rounded-sm border border-border bg-muted hover:border-border-strong hover:bg-accent"
                  :title="t('icon.change')"
                  @click="iconPicking = true"
                >
                  <Avatar :seed="props.instance.id" :icon="form.icon || 'bot'" :size="18" />
                </button>
                <Input v-model="form.icon" class="flex-1 font-mono" placeholder="bot" />
              </div>
              <p class="mt-1 text-[13px] text-muted-foreground">
                {{ t("edit.iconHint") }}
              </p>
            </div>

            <Label for="inst-group">{{ t("edit.group") }}</Label>
            <Input id="inst-group" v-model="form.group" />

            <Label for="inst-desc" class="self-start">{{ t("edit.description") }}</Label>
            <Textarea id="inst-desc" v-model="form.description" class="min-h-[64px]" />
          </div>
        </GroupBox>

        <!-- 模型 — 五格，不多不少：服务商 / 模型 / Base URL / 密钥存放 / 密钥。
             框架、运行方式那些宿主侧的旋钮在「运行时」页，不在这儿。 -->
        <GroupBox v-if="section === 'model'" :title="t('edit.nav.model')">
          <div :class="rowGrid">
            <!-- 1 服务商 -->
            <Label for="inst-provider">{{ t("edit.model.provider") }}</Label>
            <p v-if="!takesProvider" class="pt-2 text-[13px] text-muted-foreground">
              {{ t("edit.model.providerEnvOnly") }}
            </p>
            <div v-else>
              <!-- dsh 的服务商是它自己注册过的 route，可枚举，所以给它真实的那一份；
                   别的引擎给密钥库里的服务商行，外加一个「自定义…」的出口。 -->
              <Select
                v-if="isDsh"
                id="inst-provider"
                v-model="form.provider"
                :options="dshRouteOptions"
              />
              <template v-else>
                <Select
                  id="inst-provider"
                  :model-value="providerCustom ? CUSTOM : form.provider.trim()"
                  :options="providerOptions"
                  @update:model-value="pickProvider"
                />
                <!-- 「自定义…」不换掉下拉，只在它下面多一个框：换掉就回不去了。 -->
                <Input v-if="providerCustom" v-model="form.provider" class="mt-2 font-mono" />
              </template>
              <p v-if="isDsh" class="mt-1 text-[13px] text-muted-foreground">
                {{ t("edit.model.dshRouteHint") }}
              </p>
            </div>

            <!-- 2 模型 — 服务商报得出列表就是下拉，报不出才是文本框 -->
            <Label for="inst-model">{{ t("edit.model") }}</Label>
            <div>
              <template v-if="modelHasList">
                <Select
                  id="inst-model"
                  :model-value="modelCustom ? CUSTOM : form.model.trim()"
                  :options="modelOptions"
                  @update:model-value="pickModel"
                />
                <Input v-if="modelCustom" v-model="form.model" class="mt-2 font-mono" />
              </template>
              <Input v-else id="inst-model" v-model="form.model" class="font-mono" />
            </div>

            <!-- 3 Base URL — 只读：它跟着上面那个服务商走，不是实例自己的字段 -->
            <Label>{{ t("edit.model.baseUrl") }}</Label>
            <div>
              <p v-if="providerRow?.base_url" class="pt-2 font-mono text-[13px]">
                {{ providerRow.base_url }}
              </p>
              <p
                class="text-[13px] text-muted-foreground"
                :class="providerRow?.base_url ? 'mt-1' : 'pt-2'"
              >
                {{
                  providerRow?.base_url
                    ? t("edit.model.baseUrlFollow")
                    : t("edit.model.baseUrlDefault")
                }}
              </p>
            </div>

            <!-- 4 密钥存放 — 用户说的「直接用系统还是实例保管的」，加上启动器自己那份。
                 三档正对后端 `executor::resolve_credentials` 的三层。 -->
            <Label for="inst-keyhome">{{ t("edit.model.keyHome") }}</Label>
            <Select
              id="inst-keyhome"
              :model-value="keyHome"
              :options="keyHomeOptions"
              @update:model-value="pickKeyHome"
            />

            <!-- 5 密钥 — 这一格长什么样由上一格决定 -->
            <Label for="inst-key">{{ t("edit.model.key") }}</Label>
            <div>
              <!-- 用 Agent 自己的：没有东西可填，只说清密钥会从哪儿来 -->
              <p v-if="keyHome === 'system'" class="pt-2 text-[13px] text-muted-foreground">
                {{ isDsh ? t("edit.model.keySystemDsh") : t("edit.model.keySystemOther") }}
              </p>

              <!-- 用启动器保存的：选库里的一行。存的是引用，值不在这儿，也永远不到这儿；
                   「管理密钥…」是这个窗口通向密钥库的唯一一条路（见 manageKeys）。 -->
              <template v-else-if="keyHome === 'shared'">
                <div class="flex items-start gap-2">
                  <div class="min-w-0 flex-1">
                    <Select
                      id="inst-key"
                      :model-value="sharedRef"
                      :options="sharedOptions"
                      @update:model-value="(v: string) => (sharedRef = v)"
                    />
                  </div>
                  <Button variant="outline" @click="manageKeys">
                    <KeyRound class="h-4 w-4" :stroke-width="1.75" />
                    {{ t("edit.model.keyManage") }}
                  </Button>
                </div>
                <p v-if="keyStoreEmpty" class="mt-1 text-[13px] text-destructive">
                  {{ t("edit.model.keyStoreEmpty") }}
                </p>
              </template>

              <!-- 只给这个实例：写进 instances/<id>/.env（0600），启动时层在最后，所以
                   它压得住库里那份。输入框空着＝不动盘上已有的那把，删要点「清除」。 -->
              <template v-else>
                <div class="flex items-start gap-2">
                  <div class="min-w-0 flex-1">
                    <Input
                      id="inst-key"
                      v-model="keyDraft"
                      type="password"
                      autocomplete="off"
                      :placeholder="t('edit.model.keyPlaceholder')"
                    />
                  </div>
                  <Button v-if="instKey.has_value" variant="outline" @click="clearInstanceKey">
                    {{ t("edit.model.keyClear") }}
                  </Button>
                </div>
                <p v-if="keyVar" class="mt-1 text-[13px] text-muted-foreground">
                  <template v-if="instKey.has_value">
                    {{ t("edit.model.keySaved")
                    }}<span class="font-mono">{{ instKey.fingerprint }}</span> ·
                  </template>
                  {{ t("edit.model.keyWritesTo")
                  }}<span class="font-mono">{{ keyVar }}</span>
                </p>
                <p v-else class="mt-1 text-[13px] text-destructive">
                  {{ t("edit.model.keyVarUnknown") }}
                </p>
              </template>
            </div>
          </div>
        </GroupBox>

        <!-- Runtime / environment override -->
        <GroupBox v-if="section === 'runtime'" :title="t('edit.nav.runtime')">
          <div :class="rowGrid">
            <Label for="inst-engine">{{ t("edit.runtime.engine") }}</Label>
            <div>
              <Select id="inst-engine" v-model="form.engine" :options="engineOptions" />
              <p class="mt-1 text-[13px] text-muted-foreground">
                {{ t("edit.runtime.engineHint") }}
                <template v-if="selectedEngine && !selectedEngine.installed">
                  · <span class="text-destructive">{{ t("edit.runtime.engineMissing") }}</span>
                </template>
                <template v-else-if="selectedEngine && selectedEngine.path">
                  · <span class="font-mono">{{ selectedEngine.path }}</span>
                </template>
              </p>
            </div>

            <!-- How this run is hosted. A real choice for every engine, not a
                 read-only line: all six CLIs open a session by default and go
                 one-shot only under a flag, so the launcher must not decide for
                 them. A dsh web profile is the one case with no choice to make. -->
            <Label for="inst-mode">{{ t("edit.runtime.mode") }}</Label>
            <div>
              <Select id="inst-mode" v-model="form.mode" :options="modeOptions" />
              <p class="mt-1 text-[13px] text-muted-foreground">
                {{ runsWeb ? t("edit.runtime.modeServe") : t("edit.runtime.modeHint") }}
              </p>
            </div>

            <Label for="inst-envpolicy">{{ t("edit.runtime.envPolicy") }}</Label>
            <div>
              <Select
                id="inst-envpolicy"
                v-model="form.env_policy"
                :options="envPolicyOptions"
              />
              <p class="mt-1 text-[13px] text-muted-foreground">
                {{ t("edit.runtime.envPolicyHint") }}
              </p>
            </div>

            <Label for="inst-custombin" class="self-start">
              {{ t("edit.runtime.customBin") }}
            </Label>
            <div>
              <Input
                id="inst-custombin"
                v-model="form.custom_bin"
                :placeholder="`/usr/local/bin/${form.engine || 'dsh'}`"
              />
              <p class="mt-1 text-[13px] text-muted-foreground">
                {{ t("edit.runtime.customBinHint") }}
              </p>
            </div>
          </div>

          <!-- Knobs that exist only for the selected framework. Fenced off and
               named after it, so a vendor-specific field never reads as a
               launcher-wide one — dsh's `profile` is the only such knob today. -->
          <div v-if="isDsh" class="mt-4 border-t border-border pt-4">
            <p class="mb-3 text-[13px] text-muted-foreground">
              {{ t("edit.runtime.engineSpecific") }} ·
              <span class="font-mono">{{ selectedEngine?.display || form.engine }}</span>
            </p>
            <div :class="rowGrid">
              <Label for="inst-profile">{{ t("edit.profile") }}</Label>
              <div>
                <Select id="inst-profile" v-model="form.profile" :options="profileOptions" />
                <p class="mt-1 text-[13px] text-muted-foreground">
                  {{ t("edit.profileHint") }}
                </p>
              </div>
            </div>
          </div>
        </GroupBox>

        <!-- Extensions / Skills / MCP / AGENTS.md — the per-instance file pages.
             AGENTS.md loads itself (see AgentsSection.vue); the other three share
             one `extensions` read so they cannot disagree about the same state. -->
        <PluginsSection
          v-if="section === 'extensions'"
          :instance-id="instanceId"
          :extensions="extensions"
          :loading="extLoading"
          @changed="loadExtensions"
          @browse="browseMarket"
        />
        <SkillsSection
          v-else-if="section === 'skills'"
          :instance-id="instanceId"
          :extensions="extensions"
          :loading="extLoading"
          @changed="loadExtensions"
          @browse="browseMarket"
        />
        <McpSection
          v-else-if="section === 'mcp'"
          :instance-id="instanceId"
          :extensions="extensions"
          :loading="extLoading"
          @changed="loadExtensions"
          @browse="browseMarket"
        />
        <AgentsSection v-else-if="section === 'agents'" :instance-id="instanceId" />

        <!-- Task -->
        <GroupBox v-if="section === 'task'" :title="t('edit.nav.task')">
          <div class="grid gap-1.5">
            <Label for="inst-task">{{ t("edit.defaultTask") }}</Label>
            <Textarea id="inst-task" v-model="form.default_task" class="min-h-[140px]" />
            <p class="text-[13px] text-muted-foreground">
              {{ t("edit.defaultTaskHint") }}
            </p>
          </div>
        </GroupBox>
      </div>
    </div>

    <!-- The browse-and-install surface, opened from any of the three sections.
         It reports back with `installed` so the sections re-read from disk
         instead of guessing what the install did. -->
    <MarketDialog
      v-model:open="marketOpen"
      :kind="marketKind"
      :instance-id="instanceId"
      @installed="loadExtensions"
    />

    <template #footer>
      <Button variant="ghost" @click="open = false">{{ t("edit.cancel") }}</Button>
      <div class="flex-1" />
      <Button variant="primary" :disabled="saving" @click="save">
        <Save class="h-4 w-4" />
        {{ t("edit.save") }}
      </Button>
    </template>
  </component>

  <!-- 兄弟节点：Dialog 自己 Teleport 到 body，套进这个组件的内容里会被它的滚动容器裁掉。 -->
  <IconPickerDialog v-model:open="iconPicking" :current="form.icon || 'bot'" @picked="form.icon = $event" />
</template>
