// Global launcher settings persisted to localStorage (model/API defaults).
// The real dsh credential wiring lands on top of this; the GUI reads/writes
// these values and (later) syncs them into instance .env / dsh config.
import { reactive, watch } from "vue";

export interface ModelConfig {
  provider: string;
  apiKey: string;
  baseUrl: string;
  defaultModel: string;
}

const STORAGE_KEY = "dsh-launcher.modelConfig";

function load(): ModelConfig {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) return { ...defaults(), ...JSON.parse(raw) };
  } catch {
    /* ignore */
  }
  return defaults();
}

function defaults(): ModelConfig {
  return {
    provider: "deepseek",
    apiKey: "",
    baseUrl: "https://api.deepseek.com",
    defaultModel: "deepseek-reasoner",
  };
}

export const modelConfig = reactive<ModelConfig>(load());

export function saveModelConfig(): void {
  // Never persist the secret to localStorage — API keys live in dsh's
  // ~/.dsh/.credentials.yaml (written via api.setCredential). Only the
  // non-secret preferences are cached here.
  const safe = {
    provider: modelConfig.provider,
    baseUrl: modelConfig.baseUrl,
    defaultModel: modelConfig.defaultModel,
  };
  localStorage.setItem(STORAGE_KEY, JSON.stringify(safe));
}

// keep persisted copy fresh on any change
watch(modelConfig, () => saveModelConfig(), { deep: true });

/** Known providers for the settings dropdown. OpenAI-compatible base URLs.
 *  `apiKeyEnv` is the credential reference dsh resolves (written to
 *  ~/.dsh/.credentials.yaml). */
export const PROVIDERS: {
  id: string;
  label: string;
  baseUrl: string;
  apiKeyEnv: string;
  models: string[];
}[] = [
  { id: "deepseek", label: "DeepSeek", baseUrl: "https://api.deepseek.com", apiKeyEnv: "DEEPSEEK_API_KEY", models: ["deepseek-v4-flash", "deepseek-v4-pro", "deepseek-reasoner", "deepseek-chat"] },
  { id: "openai", label: "OpenAI", baseUrl: "https://api.openai.com/v1", apiKeyEnv: "OPENAI_API_KEY", models: ["gpt-4o", "gpt-4o-mini", "o3-mini"] },
  { id: "openai-compatible", label: "OpenAI 兼容 / 自定义", baseUrl: "", apiKeyEnv: "OPENAI_API_KEY", models: [] },
];
