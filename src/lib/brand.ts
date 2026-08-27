// Maps a model/provider identity onto an official brand mark from the
// `simple-icons` library (real, maintained brand SVGs — never hand-drawn).
// Instances whose model belongs to a known provider render that provider's
// logo in its official color; anything else falls back to the deterministic
// gradient-mesh avatar. simple-icons exposes { path, hex, title } per brand.
import {
  siDeepseek,
  siClaude,
  siAnthropic,
  siGooglegemini,
  siQwen,
  siMistralai,
  siMoonshotai,
  siOllama,
  siOpenrouter,
} from "simple-icons";

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
const MODEL_RULES: { re: RegExp; brand: Brand }[] = [
  { re: /deepseek/i, brand: b(siDeepseek) },
  { re: /claude/i, brand: b(siClaude) },
  { re: /gemini|palm|bison/i, brand: b(siGooglegemini) },
  { re: /qwen|tongyi/i, brand: b(siQwen) },
  { re: /mistral|mixtral|codestral/i, brand: b(siMistralai) },
  { re: /kimi|moonshot/i, brand: b(siMoonshotai) },
  { re: /llama|ollama/i, brand: b(siOllama) },
];

const PROVIDER_BRANDS: Record<string, Brand> = {
  deepseek: b(siDeepseek),
  "deepseek-official": b(siDeepseek),
  anthropic: b(siAnthropic),
  claude: b(siClaude),
  google: b(siGooglegemini),
  ollama: b(siOllama),
  openrouter: b(siOpenrouter),
  mistral: b(siMistralai),
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
