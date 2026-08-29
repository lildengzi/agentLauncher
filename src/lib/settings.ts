// Global launcher model/API defaults — the prefill a new instance starts from.
// Mirrors config.defaults (persisted to ~/.agentlauncher/config.json via
// launcherConfig) and holds nothing secret: it never had a place to put a key, and
// now it does not even carry a draft. Values go to `~/.agentlauncher/providers.json`
// (via api.setProviderKey) or, for dsh, to `~/.dsh/.credentials.yaml` (via
// api.setCredential) — in both cases straight from the field to the backend.
import { reactive, watch } from "vue";
import { config } from "@/lib/launcherConfig";

export interface ModelConfig {
  provider: string;
  defaultModel: string;
}

// Empty = "let the chosen engine use its own default", mirroring the backend's
// AgentDefaults. A vendor default here would be wrong for five of the six
// engines — and even for dsh, whose provider string is `deepseek-official`.
function defaults(): ModelConfig {
  return {
    provider: "",
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

/** The provider list is no longer hardcoded here.
 *
 *  It lives in `~/.agentlauncher/providers.json` and is read with
 *  `api.getProviders()` — because it is now user-extensible, and because each row
 *  carries API keys, which must never sit in frontend module state. `ProviderView`
 *  in `@/types` is the shape; `providers::builtins()` in the backend seeds the four
 *  rows this constant used to hold. */
