// Global launcher model/API defaults. The non-secret fields mirror
// config.defaults (persisted to ~/.agentlauncher/config.json via launcherConfig);
// `apiKey` is transient and never persisted here — it goes to dsh's
// ~/.dsh/.credentials.yaml (via api.setCredential).
import { reactive, watch } from "vue";
import { config } from "@/lib/launcherConfig";

export interface ModelConfig {
  provider: string;
  apiKey: string;
  baseUrl: string;
  defaultModel: string;
}

function defaults(): ModelConfig {
  return {
    provider: "deepseek",
    apiKey: "",
    baseUrl: "https://api.deepseek.com",
    defaultModel: "deepseek-reasoner",
  };
}

export const modelConfig = reactive<ModelConfig>(defaults());

/** Push the editable non-secret fields into the launcher config (persisted). */
export function saveModelConfig(): void {
  config.defaults.provider = modelConfig.provider;
  config.defaults.base_url = modelConfig.baseUrl;
  config.defaults.model = modelConfig.defaultModel;
}

// Two-way binding with config.defaults. Equal-value writes are no-ops in Vue, so
// the pair converges without looping. This also picks up backend hydration.
watch(modelConfig, saveModelConfig, { deep: true });
watch(
  () => [config.defaults.provider, config.defaults.base_url, config.defaults.model] as const,
  ([provider, baseUrl, model]) => {
    modelConfig.provider = provider;
    modelConfig.baseUrl = baseUrl;
    modelConfig.defaultModel = model;
  }
);

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
