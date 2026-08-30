// Maps a model/provider identity onto an official brand mark from the
// `simple-icons` library (real, maintained brand SVGs — never hand-drawn).
// Instances whose model belongs to a known provider render that provider's
// logo in its official color; anything else falls back to the deterministic
// gradient-mesh avatar. simple-icons exposes { path, hex, title } per brand.
import {
  siDeepseek,
  siClaude,
  siClaudecode,
  siAnthropic,
  siGooglegemini,
  siQwen,
  siMistralai,
  siMoonshotai,
  siOllama,
  siOpenrouter,
  siOpencode,
  siPerplexity,
  siHuggingface,
  siMeta,
  siMetaai,
  siMinimax,
  siReplicate,
  siAlibabacloud,
  siBaidu,
  siBytedance,
} from "simple-icons";

// OpenAI was removed from simple-icons >=15 for trademark reasons.
// Vendored from simple-icons@13 (hex 412991) so `gpt-*`/`o3` etc still render.
const siOpenai = {
  title: "OpenAI",
  hex: "412991",
  path: "M22.2819 9.8211a5.9847 5.9847 0 0 0-.5157-4.9108 6.0462 6.0462 0 0 0-6.5098-2.9A6.0651 6.0651 0 0 0 4.9807 4.1818a5.9847 5.9847 0 0 0-3.9977 2.9 6.0462 6.0462 0 0 0 .7427 7.0966 5.98 5.98 0 0 0 .511 4.9107 6.051 6.051 0 0 0 6.5146 2.9001A5.9847 5.9847 0 0 0 13.2599 24a6.0557 6.0557 0 0 0 5.7718-4.2058 5.9894 5.9894 0 0 0 3.9977-2.9001 6.0557 6.0557 0 0 0-.7475-7.0729zm-9.022 12.6081a4.4755 4.4755 0 0 1-2.8764-1.0408l.1419-.0804 4.7783-2.7582a.7948.7948 0 0 0 .3927-.6813v-6.7369l2.02 1.1686a.071.071 0 0 1 .038.052v5.5826a4.504 4.504 0 0 1-4.4945 4.4944zm-9.6607-4.1254a4.4708 4.4708 0 0 1-.5346-3.0137l.142.0852 4.783 2.7582a.7712.7712 0 0 0 .7806 0l5.8428-3.3685v2.3324a.0804.0804 0 0 1-.0332.0615L9.74 19.9502a4.4992 4.4992 0 0 1-6.1408-1.6464zM2.3408 7.8956a4.485 4.485 0 0 1 2.3655-1.9728V11.6a.7664.7664 0 0 0 .3879.6765l5.8144 3.3543-2.0201 1.1685a.0757.0757 0 0 1-.071 0l-4.8303-2.7865A4.504 4.504 0 0 1 2.3408 7.872zm16.5963 3.8558L13.1038 8.364 15.1192 7.2a.0757.0757 0 0 1 .071 0l4.8303 2.7913a4.4944 4.4944 0 0 1-.6765 8.1042v-5.6772a.79.79 0 0 0-.407-.667zm2.0107-3.0231l-.142-.0852-4.7735-2.7818a.7759.7759 0 0 0-.7854 0L9.409 9.2297V6.8974a.0662.0662 0 0 1 .0284-.0615l4.8303-2.7866a4.4992 4.4992 0 0 1 6.6802 4.66zM8.3065 12.863l-2.02-1.1638a.0804.0804 0 0 1-.038-.0567V6.0742a4.4992 4.4992 0 0 1 7.3757-3.4537l-.142.0805L8.704 5.459a.7948.7948 0 0 0-.3927.6813zm1.0976-2.3654l2.602-1.4998 2.6069 1.4998v2.9994l-2.5974 1.4997-2.6067-1.4997Z",
};

export interface Brand {
  /** 24x24 SVG path data. */
  path: string;
  /** official brand hex, WITHOUT the leading '#'. */
  hex: string;
  title: string;
}

function b(icon: { path: string; hex: string; title: string }): Brand {
  return { path: icon.path, hex: icon.hex, title: icon.title };
}

// Ordered rules: first regex to match the model id wins.
// Covers every provider that has a real icon in simple-icons@16.28 plus
// vendored OpenAI. Free-form `provider`/`model` strings still work —
// unmatched values fall back to the Lucide `bot` avatar (see Avatar.vue).
const MODEL_RULES: { re: RegExp; brand: Brand }[] = [
  { re: /deepseek/i, brand: b(siDeepseek) },
  { re: /claude/i, brand: b(siClaude) },
  { re: /gpt|openai|^o[13](-|\b)|codex/i, brand: b(siOpenai) },
  { re: /gemini|palm|bison/i, brand: b(siGooglegemini) },
  { re: /qwen|tongyi|alibaba/i, brand: b(siQwen) },
  { re: /mistral|mixtral|codestral/i, brand: b(siMistralai) },
  { re: /kimi|moonshot/i, brand: b(siMoonshotai) },
  // Perplexity sonar family: sonar, sonar-pro, sonar-reasoning
  { re: /perplexity|sonar/i, brand: b(siPerplexity) },
  { re: /minimax/i, brand: b(siMinimax) },
  { re: /doubao|bytedance/i, brand: b(siBytedance) },
  { re: /ernie|baidu/i, brand: b(siBaidu) },
  { re: /ollama/i, brand: b(siOllama) },
  { re: /llama/i, brand: b(siMetaai) },
  { re: /huggingface/i, brand: b(siHuggingface) },
  { re: /replicate/i, brand: b(siReplicate) },
  { re: /opencode/i, brand: b(siOpencode) },
];

const PROVIDER_BRANDS: Record<string, Brand> = {
  // DeepSeek
  deepseek: b(siDeepseek),
  "deepseek-official": b(siDeepseek),
  // Anthropic / Claude
  anthropic: b(siAnthropic),
  claude: b(siClaude),
  "claude-code": b(siClaudecode),
  // OpenAI family (vendored)
  openai: b(siOpenai),
  "openai-compatible": b(siOpenai),
  codex: b(siOpenai),
  // Google
  google: b(siGooglegemini),
  gemini: b(siGooglegemini),
  // Alibaba / Qwen
  qwen: b(siQwen),
  tongyi: b(siQwen),
  alibaba: b(siAlibabacloud),
  "alibaba-cloud": b(siAlibabacloud),
  alibabacloud: b(siAlibabacloud),
  // Others with icons
  mistral: b(siMistralai),
  moonshot: b(siMoonshotai),
  kimi: b(siMoonshotai),
  perplexity: b(siPerplexity),
  minimax: b(siMinimax),
  doubao: b(siBytedance),
  bytedance: b(siBytedance),
  baidu: b(siBaidu),
  ernie: b(siBaidu),
  // Meta / Llama
  meta: b(siMeta),
  "meta-ai": b(siMetaai),
  metaai: b(siMetaai),
  llama: b(siMetaai),
  ollama: b(siOllama),
  // Infra / aggregators
  huggingface: b(siHuggingface),
  replicate: b(siReplicate),
  openrouter: b(siOpenrouter),
  opencode: b(siOpencode),
};

/** Brand for a model id (e.g. "deepseek-reasoner" → DeepSeek), or null. */
export function brandForModel(model: string | null | undefined): Brand | null {
  if (!model) return null;
  return MODEL_RULES.find((r) => r.re.test(model))?.brand ?? null;
}

/** Brand for an explicit provider slug, or null. */
export function brandForProvider(provider: string | null | undefined): Brand | null {
  if (!provider) return null;
  return PROVIDER_BRANDS[provider.toLowerCase()] ?? null;
}

// ---- the icon picker's catalogue ------------------------------------------
// `instance.icon` used to be one thing — a lucide glyph name, typed into a text box.
// It now also accepts `brand:<slug>`, which names one of the marks above. Old values
// keep working untouched: anything without the prefix is still a lucide name, and an
// unknown prefix falls through to the same Bot fallback as an unknown glyph, so a
// hand-edited config.json can never render as nothing.

/** Marks a *chosen* icon, as opposed to one merely inferred from the model. */
export const BRAND_PREFIX = "brand:";

/** "Hugging Face" → "hugging-face". Ids come from the title rather than the
 *  `siFoo` import name so that two aliases of one mark collapse to one entry. */
function titleId(title: string): string {
  return title
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

/** Every distinct mark, once each — the brand half of the icon picker's grid. */
export const BRAND_CHOICES: { id: string; brand: Brand }[] = (() => {
  const seen = new Set<string>();
  const out: { id: string; brand: Brand }[] = [];
  for (const brand of Object.values(PROVIDER_BRANDS)) {
    const id = BRAND_PREFIX + titleId(brand.title);
    if (seen.has(id)) continue;
    seen.add(id);
    out.push({ id, brand });
  }
  return out.sort((a, b) => a.brand.title.localeCompare(b.brand.title));
})();

const BRAND_BY_ICON = new Map(BRAND_CHOICES.map((c) => [c.id, c.brand]));

/** The icon id for a mark, so a *derived* brand can be written down as a chosen one. */
export function iconIdForBrand(brand: Brand): string {
  return BRAND_PREFIX + titleId(brand.title);
}

/** The mark an `instance.icon` names, or null when it names a lucide glyph. */
export function brandForIcon(icon: string | null | undefined): Brand | null {
  if (!icon || !icon.startsWith(BRAND_PREFIX)) return null;
  return BRAND_BY_ICON.get(icon) ?? null;
}

/** What each engine looks like when nothing more specific is known.
 *
 *  `pi` and `omp` get a neutral lucide glyph rather than a mark, because neither
 *  project has one in simple-icons and hand-drawing brand art is not an option (see
 *  the note at the top of this file) — a wrong logo is worse than a plain glyph. */
const ENGINE_ICONS: Record<string, string> = {
  // dsh *is* the DeepSeek Harness, hence DeepSeek's mark rather than a generic one.
  dsh: iconIdForBrand(b(siDeepseek)),
  claude: iconIdForBrand(b(siClaudecode)),
  codex: iconIdForBrand(siOpenai),
  opencode: iconIdForBrand(b(siOpencode)),
  pi: "pi",
  omp: "blocks",
};

/**
 * The icon a new instance should start from, given what it is actually made of.
 *
 * The old default was the literal string `"bot"` — a grey robot on every tile, which
 * told the user nothing and looked like a placeholder because it was one. An instance
 * that runs DeepSeek should arrive wearing DeepSeek's mark; the picker is then for
 * *changing* that, not for rescuing it.
 *
 * Order is most-specific-first: the model names the vendor, the provider names it less
 * precisely, and the engine is the last thing that can say anything at all.
 */
export function defaultIcon(opts: {
  engine?: string | null;
  provider?: string | null;
  model?: string | null;
}): string {
  const found = brandForModel(opts.model) ?? brandForProvider(opts.provider);
  if (found) return iconIdForBrand(found);
  return ENGINE_ICONS[(opts.engine ?? "").toLowerCase()] ?? "bot";
}
