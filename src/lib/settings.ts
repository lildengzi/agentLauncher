// Global launcher model/API defaults. The non-secret fields mirror
// config.defaults (persisted to ~/.agentlauncher/config.json via launcherConfig);
// `apiKey` is transient and never persisted here — it goes to dsh's
// ~/.dsh/.credentials.yaml (via api.setCredential).
import { reactive, watch } from "vue";
import { config } from "@/lib/launcherConfig";

export interface ModelConfig {
  provider: string;
  apiKey: string;
  defaultModel: string;
}

// Empty = "let the chosen engine use its own default", mirroring the backend's
// AgentDefaults. A vendor default here would be wrong for five of the six
// engines — and even for dsh, whose provider string is `deepseek-official`.
function defaults(): ModelConfig {
  return {
    provider: "",
    apiKey: "",
    defaultModel: "",
  };
}

export const modelConfig = reactive<ModelConfig>(defaults());

/** Push the editable non-secret fields into the launcher config (persisted). */
export function saveModelConfig(): void {
  config.defaults.provider = modelConfig.provider;
  config.defaults.model = modelConfig.defaultModel;
}

// Two-way binding with config.defaults. Equal-value writes are no-ops in Vue, so
// the pair converges without looping. This also picks up backend hydration.
watch(modelConfig, saveModelConfig, { deep: true });
watch(
  () => [config.defaults.provider, config.defaults.model] as const,
  ([provider, model]) => {
    modelConfig.provider = provider;
    modelConfig.defaultModel = model;
  }
);

/** Known providers for the settings dropdown. `apiKeyEnv` is the credential
 *  reference dsh resolves (written to ~/.dsh/.credentials.yaml). Base URLs are
 *  not listed: they never reached an engine from here — every engine reads its
 *  own base URL from the instance `.env`. */
export const PROVIDERS: {
  id: string;
  label: string;
  apiKeyEnv: string;
  models: string[];
}[] = [
  { id: "deepseek", label: "DeepSeek", apiKeyEnv: "DEEPSEEK_API_KEY", models: ["deepseek-v4-flash", "deepseek-v4-pro", "deepseek-reasoner", "deepseek-chat"] },
  { id: "openai", label: "OpenAI", apiKeyEnv: "OPENAI_API_KEY", models: ["gpt-4o", "gpt-4o-mini", "o3-mini"] },
  { id: "openai-compatible", label: "OpenAI 兼容 / 自定义", apiKeyEnv: "OPENAI_API_KEY", models: [] },
];
