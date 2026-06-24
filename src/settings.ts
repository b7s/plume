import "./settings.css";
import { invoke } from "@tauri-apps/api/core";

interface Config {
  provider: string;
  model: string;
  model_url: string;
  endpoint: string;
  api_key: string;
  headers: string;
  port: number;
  dictionary: { language: string; url: string };
  translation: { enabled: boolean; language: string };
  window: { x: number; y: number; width: number; height: number };
  idle_timeout: number;
  suggestion_count: number;
  ai_suggestion_count: number;
  ai_suggestion_delay: number;
}

const MODEL_PRESETS_LOCAL: [string, string][] = [
  ["Qwen3-0.6B-Q4_K_M.gguf", "Qwen3-0.6B (Q4_K_M, 397MB)"],
  ["Qwen3-0.6B-Q6_K.gguf", "Qwen3-0.6B (Q6_K, 495MB)"],
  ["Qwen3-0.6B-Q8_0.gguf", "Qwen3-0.6B (Q8_0, 639MB) - Best quality"],
  ["Qwen3-1.7B-Q4_K_M.gguf", "Qwen3-1.7B (Q4_K_M, ~1.1GB)"],
  ["Llama-3.2-1B-Instruct-Q3_K_S.gguf", "Llama 3.2-1B (Q3_K_S, 642MB)"],
  ["TinyLlama-1.1B-Chat-v1.0-Q4_K_S.gguf", "TinyLlama-1.1B (Q4_K_S, 640MB)"],
];

const MODEL_PRESETS_OLLAMA: [string, string][] = [
  ["qwen3:0.6b", "Qwen3-0.6B"],
  ["qwen3:1.7b", "Qwen3-1.7B"],
  ["llama3.2:1b", "Llama 3.2-1B"],
  ["tinyllama:1.1b", "TinyLlama-1.1B"],
  ["smollm2:1.7b", "SmolLM2-1.7B"],
  ["phi3:mini", "Phi-3 Mini (3.8B)"],
];

const MODEL_PRESETS_OPENAI: [string, string][] = [
  ["gpt-5.4-nano", "GPT-5.4 Nano — $0.20/$1.25"],
  ["gpt-5.4-mini", "GPT-5.4 Mini — $0.75/$4.50"],
  ["gpt-4.1-mini", "GPT-4.1 Mini — $0.40/$1.60"],
  ["gpt-4o-mini", "GPT-4o Mini — $0.15/$0.60"],
  ["gpt-5.5", "GPT-5.5 — $5/$30"],
  ["gpt-5.5-pro", "GPT-5.5 Pro — $30/$180"],
];

const TRANSLATION_LANGS: [string, string][] = [
  ["portuguese", "Português"],
  ["english", "English"],
  ["spanish", "Español"],
  ["french", "Français"],
  ["german", "Deutsch"],
  ["italian", "Italiano"],
  ["russian", "Русский"],
  ["japanese", "日本語"],
  ["chinese", "中文"],
  ["korean", "한국어"],
  ["arabic", "العربية"],
  ["dutch", "Nederlands"],
];

const root = document.getElementById("root")!;

root.innerHTML = `
  <div class="settings">
    <div class="settings-header">
      <span class="settings-title">Settings</span>
    </div>
    <div class="settings-body" id="settings-body">
      <div class="cfg-section">LLM</div>
      <label class="cfg-field">
        <span>Provider</span>
        <select id="cfg-provider" class="cfg-input">
          <option value="local">Local (llama.cpp)</option>
          <option value="ollama">Ollama</option>
          <option value="openai">OpenAI-compatible</option>
          <option value="custom">Custom API</option>
        </select>
      </label>

      <label class="cfg-field" data-provider="local">
        <span>Model</span>
        <select id="cfg-model-local" class="cfg-input"></select>
        <input id="cfg-model-local-custom" class="cfg-input cfg-model-custom hidden" type="text" placeholder="Enter GGUF filename..." />
      </label>
      <label class="cfg-field" data-provider="local">
        <span>Model URL</span>
        <input id="cfg-model-url" class="cfg-input" type="text" placeholder="https://huggingface.co/..." />
      </label>
      <label class="cfg-field" data-provider="local">
        <span>Port</span>
        <input id="cfg-port" class="cfg-input" type="number" min="1" max="65535" placeholder="8080" />
      </label>
      <label class="cfg-field" data-provider="local">
        <span>Endpoint</span>
        <input id="cfg-endpoint-local" class="cfg-input" type="text" placeholder="http://127.0.0.1:8080" />
      </label>

      <label class="cfg-field" data-provider="ollama">
        <span>Model</span>
        <select id="cfg-model-ollama" class="cfg-input"></select>
        <input id="cfg-model-ollama-custom" class="cfg-input cfg-model-custom hidden" type="text" placeholder="Enter Ollama model name..." />
      </label>
      <label class="cfg-field" data-provider="ollama">
        <span>Endpoint</span>
        <input id="cfg-endpoint-ollama" class="cfg-input" type="text" placeholder="http://127.0.0.1:11434" />
      </label>

      <label class="cfg-field" data-provider="openai">
        <span>Model</span>
        <select id="cfg-model-openai" class="cfg-input"></select>
        <input id="cfg-model-openai-custom" class="cfg-input cfg-model-custom hidden" type="text" placeholder="Enter model name..." />
      </label>
      <label class="cfg-field" data-provider="openai">
        <span>API Key</span>
        <input id="cfg-api-key" class="cfg-input" type="password" placeholder="sk-..." />
      </label>
      <label class="cfg-field" data-provider="openai">
        <span>Endpoint (optional)</span>
        <input id="cfg-endpoint-openai" class="cfg-input" type="text" placeholder="https://api.openai.com/v1/chat/completions" />
      </label>

      <label class="cfg-field" data-provider="custom">
        <span>API URL (required)</span>
        <input id="cfg-endpoint-custom" class="cfg-input" type="text" placeholder="https://api.example.com/v1/chat/completions" />
      </label>
      <label class="cfg-field" data-provider="custom">
        <span>Model</span>
        <input id="cfg-model-custom-api" class="cfg-input" type="text" placeholder="model-name" />
      </label>
      <label class="cfg-field" data-provider="custom">
        <span>Headers (JSON, optional)</span>
        <input id="cfg-headers" class="cfg-input" type="text" placeholder='{"Authorization": "Bearer sk-..."}' />
      </label>

      <div class="cfg-section">Spellcheck</div>
      <label class="cfg-field">
        <span>Dictionary Language</span>
        <select id="cfg-dict-lang" class="cfg-input"></select>
      </label>
      <label class="cfg-field">
        <span>Suggestion Count</span>
        <input id="cfg-suggestion-count" class="cfg-input" type="number" min="1" max="20" placeholder="6" />
      </label>
      <label class="cfg-field">
        <span>AI Next-Word Count</span>
        <input id="cfg-ai-suggestion-count" class="cfg-input" type="number" min="0" max="10" placeholder="2" />
      </label>
      <label class="cfg-field">
        <span>AI Suggestion Delay (ms)</span>
        <input id="cfg-ai-suggestion-delay" class="cfg-input" type="number" min="200" max="5000" step="100" placeholder="800" />
      </label>
      <label class="cfg-field">
        <span>Idle Timeout (seconds)</span>
        <input id="cfg-idle-timeout" class="cfg-input" type="number" min="1" max="60" placeholder="6" />
      </label>
      <div class="cfg-section">Translation</div>
      <label class="cfg-field cfg-check">
        <input id="cfg-tr-enabled" type="checkbox" />
        <span>Enable Translation</span>
      </label>
      <label class="cfg-field">
        <span>Translation Language</span>
        <select id="cfg-tr-lang" class="cfg-input"></select>
      </label>
    </div>
    <div class="settings-footer">
      <button id="modal-cancel" class="modal-btn modal-btn-secondary">Cancel</button>
      <button id="modal-save" class="modal-btn modal-btn-primary">Save</button>
    </div>
  </div>
`;

const cfgProvider = document.getElementById("cfg-provider") as HTMLSelectElement;
const cfgModelLocal = document.getElementById("cfg-model-local") as HTMLSelectElement;
const cfgModelLocalCustom = document.getElementById("cfg-model-local-custom") as HTMLInputElement;
const cfgModelOllama = document.getElementById("cfg-model-ollama") as HTMLSelectElement;
const cfgModelOllamaCustom = document.getElementById("cfg-model-ollama-custom") as HTMLInputElement;
const cfgModelOpenai = document.getElementById("cfg-model-openai") as HTMLSelectElement;
const cfgModelOpenaiCustom = document.getElementById("cfg-model-openai-custom") as HTMLInputElement;
const cfgModelCustomApi = document.getElementById("cfg-model-custom-api") as HTMLInputElement;
const cfgModelUrl = document.getElementById("cfg-model-url") as HTMLInputElement;
const cfgEndpointLocal = document.getElementById("cfg-endpoint-local") as HTMLInputElement;
const cfgEndpointOllama = document.getElementById("cfg-endpoint-ollama") as HTMLInputElement;
const cfgEndpointOpenai = document.getElementById("cfg-endpoint-openai") as HTMLInputElement;
const cfgEndpointCustom = document.getElementById("cfg-endpoint-custom") as HTMLInputElement;
const cfgApiKey = document.getElementById("cfg-api-key") as HTMLInputElement;
const cfgHeaders = document.getElementById("cfg-headers") as HTMLInputElement;
const cfgPort = document.getElementById("cfg-port") as HTMLInputElement;
const cfgDictLang = document.getElementById("cfg-dict-lang") as HTMLSelectElement;
const cfgSuggestionCount = document.getElementById("cfg-suggestion-count") as HTMLInputElement;
const cfgAiSuggestionCount = document.getElementById("cfg-ai-suggestion-count") as HTMLInputElement;
const cfgAiSuggestionDelay = document.getElementById("cfg-ai-suggestion-delay") as HTMLInputElement;
const cfgIdleTimeout = document.getElementById("cfg-idle-timeout") as HTMLInputElement;
const cfgTrEnabled = document.getElementById("cfg-tr-enabled") as HTMLInputElement;
const cfgTrLang = document.getElementById("cfg-tr-lang") as HTMLSelectElement;
const modalCancel = document.getElementById("modal-cancel") as HTMLButtonElement;
const modalSave = document.getElementById("modal-save") as HTMLButtonElement;

for (const [code, name] of TRANSLATION_LANGS) {
  const opt = document.createElement("option");
  opt.value = code;
  opt.textContent = name;
  cfgTrLang.appendChild(opt);
}

function showCustomInput(select: HTMLSelectElement, customInput: HTMLInputElement) {
  const isCustom = select.value === "__custom__";
  customInput.classList.toggle("hidden", !isCustom);
}

function setupSelect(select: HTMLSelectElement, customInput: HTMLInputElement, presets: [string, string][]) {
  select.innerHTML = "";
  for (const [value, label] of presets) {
    const opt = document.createElement("option");
    opt.value = value;
    opt.textContent = label;
    select.appendChild(opt);
  }
  const customOpt = document.createElement("option");
  customOpt.value = "__custom__";
  customOpt.textContent = "Custom...";
  select.appendChild(customOpt);

  select.addEventListener("change", () => showCustomInput(select, customInput));
}

setupSelect(cfgModelLocal, cfgModelLocalCustom, MODEL_PRESETS_LOCAL);
setupSelect(cfgModelOllama, cfgModelOllamaCustom, MODEL_PRESETS_OLLAMA);
setupSelect(cfgModelOpenai, cfgModelOpenaiCustom, MODEL_PRESETS_OPENAI);

function updateProviderFields() {
  const provider = cfgProvider.value;
  document.querySelectorAll<HTMLElement>(".cfg-field[data-provider]").forEach((el) => {
    el.style.display = el.dataset.provider === provider ? "" : "none";
  });
}

cfgProvider.addEventListener("change", updateProviderFields);

function getModelValue(provider: string): string {
  if (provider === "local") {
    return cfgModelLocal.value === "__custom__" ? cfgModelLocalCustom.value : cfgModelLocal.value;
  }
  if (provider === "ollama") {
    return cfgModelOllama.value === "__custom__" ? cfgModelOllamaCustom.value : cfgModelOllama.value;
  }
  if (provider === "openai") {
    return cfgModelOpenai.value === "__custom__" ? cfgModelOpenaiCustom.value : cfgModelOpenai.value;
  }
  return cfgModelCustomApi.value;
}

function getEndpointValue(provider: string): string {
  if (provider === "local") return cfgEndpointLocal.value;
  if (provider === "ollama") return cfgEndpointOllama.value;
  if (provider === "openai") return cfgEndpointOpenai.value;
  if (provider === "custom") return cfgEndpointCustom.value;
  return "";
}

function setModelValue(provider: string, model: string) {
  if (provider === "local") {
    populateSelectDirect(cfgModelLocal, MODEL_PRESETS_LOCAL, model);
    cfgModelLocalCustom.value = model;
    showCustomInput(cfgModelLocal, cfgModelLocalCustom);
  } else if (provider === "ollama") {
    populateSelectDirect(cfgModelOllama, MODEL_PRESETS_OLLAMA, model);
    cfgModelOllamaCustom.value = model;
    showCustomInput(cfgModelOllama, cfgModelOllamaCustom);
  } else if (provider === "openai") {
    populateSelectDirect(cfgModelOpenai, MODEL_PRESETS_OPENAI, model);
    cfgModelOpenaiCustom.value = model;
    showCustomInput(cfgModelOpenai, cfgModelOpenaiCustom);
  } else {
    cfgModelCustomApi.value = model;
  }
}

function populateSelectDirect(sel: HTMLSelectElement, presets: [string, string][], selected: string) {
  sel.value = presets.some(([v]) => v === selected) ? selected : "__custom__";
}

async function loadConfig() {
  const cfg = await invoke<Config>("get_config");
  cfgProvider.value = cfg.provider || "local";
  setModelValue(cfgProvider.value, cfg.model || "");
  cfgModelUrl.value = cfg.model_url || "";
  cfgEndpointLocal.value = cfg.endpoint || "http://127.0.0.1:8080";
  cfgEndpointOllama.value = cfg.endpoint || "http://127.0.0.1:11434";
  cfgEndpointOpenai.value = cfg.endpoint || "https://api.openai.com/v1/chat/completions";
  cfgEndpointCustom.value = cfg.endpoint || "";
  cfgApiKey.value = cfg.api_key || "";
  cfgHeaders.value = cfg.headers || "";
  cfgPort.value = String(cfg.port ?? 8080);
  cfgSuggestionCount.value = String(cfg.suggestion_count ?? 6);
  cfgAiSuggestionCount.value = String(cfg.ai_suggestion_count ?? 3);
  cfgAiSuggestionDelay.value = String(cfg.ai_suggestion_delay ?? 800);
  cfgIdleTimeout.value = String(cfg.idle_timeout ?? 6);
  cfgTrEnabled.checked = !!cfg.translation?.enabled;
  cfgTrLang.value = cfg.translation?.language || "portuguese";

  const langs = await invoke<[string, string][]>("list_languages");
  cfgDictLang.innerHTML = "";
  for (const [code, name] of langs) {
    const opt = document.createElement("option");
    opt.value = code;
    opt.textContent = name;
    cfgDictLang.appendChild(opt);
  }
  cfgDictLang.value = cfg.dictionary?.language || "en_US";

  updateProviderFields();
}

loadConfig().catch(console.error);

let isDownloading = false;

async function handleLocalModelChange() {
  if (isDownloading) return;
  const model = getModelValue("local");
  if (!model) return;
  const exists = await invoke<boolean>("check_model", { modelName: model });
  if (!exists) {
    isDownloading = true;
    modalSave.disabled = true;
    modalSave.textContent = "Downloading model… (this may take a while)";
    try {
      await invoke("download_model", { modelName: model, modelUrl: cfgModelUrl.value });
    } catch (e) {
      console.error(e);
    } finally {
      isDownloading = false;
      modalSave.disabled = false;
      modalSave.textContent = "Save";
    }
  }
}

cfgModelLocal.addEventListener("change", handleLocalModelChange);
cfgModelLocalCustom.addEventListener("blur", handleLocalModelChange);

modalCancel.onclick = async () => {
  await invoke("close_settings");
};

modalSave.onclick = async () => {
  const existing = await invoke<Config>("get_config");
  const provider = cfgProvider.value;
  const model = getModelValue(provider);
  const newCfg: Config = {
    ...existing,
    provider,
    model,
    model_url: cfgModelUrl.value,
    endpoint: getEndpointValue(provider),
    api_key: cfgApiKey.value,
    headers: cfgHeaders.value,
    port: parseInt(cfgPort.value) || 8080,
    dictionary: {
      language: cfgDictLang.value,
      url: existing.dictionary?.url || "",
    },
    translation: {
      enabled: cfgTrEnabled.checked,
      language: cfgTrLang.value,
    },
    idle_timeout: parseInt(cfgIdleTimeout.value) || 6,
    suggestion_count: parseInt(cfgSuggestionCount.value) || 6,
    ai_suggestion_count: parseInt(cfgAiSuggestionCount.value) || 3,
    ai_suggestion_delay: parseInt(cfgAiSuggestionDelay.value) || 800,
  };

  modalSave.disabled = true;

  try {
    if (provider === "local") {
      const exists = await invoke<boolean>("check_model", { modelName: model });
      if (!exists) {
        modalSave.textContent = "Downloading model… (this may take a while)";
        await invoke("download_model", { modelName: model, modelUrl: cfgModelUrl.value });
      }
    }

    modalSave.textContent = "Saving…";
    await invoke("save_config", { config: newCfg });

    if (cfgDictLang.value !== existing.dictionary?.language) {
      modalSave.textContent = "Downloading dictionary…";
      await invoke("set_dictionary_language", { language: cfgDictLang.value });
    }

    modalSave.textContent = "Done";
    await invoke("close_settings");
  } catch (e) {
    console.error(e);
    modalSave.textContent = "Save";
    modalSave.disabled = false;
  }
};
