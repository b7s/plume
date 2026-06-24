import "./settings.css";
import { invoke } from "@tauri-apps/api/core";

interface Config {
  provider: string;
  model: string;
  model_url: string;
  endpoint: string;
  api_key: string;
  port: number;
  dictionary: { language: string; url: string };
  translation: { enabled: boolean; language: string };
  window: { x: number; y: number; width: number; height: number };
  idle_timeout: number;
  suggestion_count: number;
}

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
        </select>
      </label>
      <label class="cfg-field">
        <span>Model</span>
        <input id="cfg-model" class="cfg-input" type="text" placeholder="Qwen3-0.6B-Q4_K_M.gguf" />
      </label>
      <label class="cfg-field">
        <span>Model URL</span>
        <input id="cfg-model-url" class="cfg-input" type="text" placeholder="https://huggingface.co/..." />
      </label>
      <label class="cfg-field">
        <span>Endpoint</span>
        <input id="cfg-endpoint" class="cfg-input" type="text" placeholder="http://127.0.0.1:8080" />
      </label>
      <label class="cfg-field">
        <span>API Key</span>
        <input id="cfg-api-key" class="cfg-input" type="password" placeholder="(OpenAI only)" />
      </label>
      <label class="cfg-field">
        <span>Port</span>
        <input id="cfg-port" class="cfg-input" type="number" min="1" max="65535" placeholder="8080" />
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
        <span>Idle Timeout (seconds)</span>
        <input id="cfg-idle-timeout" class="cfg-input" type="number" min="1" max="60" placeholder="4" />
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
const cfgModel = document.getElementById("cfg-model") as HTMLInputElement;
const cfgModelUrl = document.getElementById("cfg-model-url") as HTMLInputElement;
const cfgEndpoint = document.getElementById("cfg-endpoint") as HTMLInputElement;
const cfgApiKey = document.getElementById("cfg-api-key") as HTMLInputElement;
const cfgPort = document.getElementById("cfg-port") as HTMLInputElement;
const cfgDictLang = document.getElementById("cfg-dict-lang") as HTMLSelectElement;
const cfgSuggestionCount = document.getElementById("cfg-suggestion-count") as HTMLInputElement;
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

async function loadConfig() {
  const cfg = await invoke<Config>("get_config");
  cfgProvider.value = cfg.provider || "local";
  cfgModel.value = cfg.model || "";
  cfgModelUrl.value = cfg.model_url || "";
  cfgEndpoint.value = cfg.endpoint || "";
  cfgApiKey.value = cfg.api_key || "";
  cfgPort.value = String(cfg.port || 8080);
  cfgSuggestionCount.value = String(cfg.suggestion_count || 6);
  cfgIdleTimeout.value = String(cfg.idle_timeout || 4);
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
}

loadConfig().catch(console.error);

modalCancel.onclick = async () => {
  await invoke("close_settings");
};

modalSave.onclick = async () => {
  const existing = await invoke<Config>("get_config");
  const newCfg: Config = {
    ...existing,
    provider: cfgProvider.value,
    model: cfgModel.value,
    model_url: cfgModelUrl.value,
    endpoint: cfgEndpoint.value,
    api_key: cfgApiKey.value,
    port: parseInt(cfgPort.value) || 8080,
    dictionary: {
      language: cfgDictLang.value,
      url: existing.dictionary?.url || "",
    },
    translation: {
      enabled: cfgTrEnabled.checked,
      language: cfgTrLang.value,
    },
    idle_timeout: parseInt(cfgIdleTimeout.value) || 4,
    suggestion_count: parseInt(cfgSuggestionCount.value) || 6,
  };

  modalSave.textContent = "Saving…";
  modalSave.disabled = true;

  try {
    await invoke("save_config", { config: newCfg });

    if (cfgDictLang.value !== existing.dictionary?.language) {
      modalSave.textContent = "Downloading dictionary…";
      await invoke("set_dictionary_language", { language: cfgDictLang.value });
    }

    await invoke("close_settings");
  } catch (e) {
    console.error(e);
    modalSave.textContent = "Save";
    modalSave.disabled = false;
  }
};
