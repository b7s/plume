import "./index.css";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalSize, LogicalPosition } from "@tauri-apps/api/window";

interface SuggestionPayload {
  corrections: string[];
  translation?: string;
  word: string;
  full_text: string;
}

interface PlaceholderPayload {
  corrections: string[];
  word: string;
  full_text: string;
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

function render() {
  root.innerHTML = `
    <div class="card hidden" id="overlay">
      <div class="word-row" id="word-row">
        <span id="word-label">WORD</span>
        <span id="word-display"></span>
        <button id="settings-btn" class="settings-btn" title="Settings">
          <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" width="14" height="14"><path d="M8 10.5a2.5 2.5 0 1 0 0-5 2.5 2.5 0 0 0 0 5z"/><path d="M13.5 8a5.5 5.5 0 0 0-.1-1l1.4-1.1-1.5-2.6-1.7.7a5.3 5.3 0 0 0-1.7-1l-.3-1.8h-3l-.3 1.8a5.3 5.3 0 0 0-1.7 1l-1.7-.7L1.4 5.9 2.8 7a5.5 5.5 0 0 0 0 2l-1.4 1.1 1.5 2.6 1.7-.7a5.3 5.3 0 0 0 1.7 1l.3 1.8h3l.3-1.8a5.3 5.3 0 0 0 1.7-1l1.7.7 1.5-2.6-1.4-1.1a5.5 5.5 0 0 0 .1-1z"/></svg>
        </button>
      </div>
      <div class="chips-row" id="chips"></div>
      <div class="tr-section hidden" id="tr-section">
        <div class="divider" id="divider"></div>
        <textarea id="tr-text" class="tr-text" placeholder="Type or auto-captured text will appear here…" rows="2"></textarea>
        <div class="tr-toolbar" id="tr-toolbar">
          <select id="tr-action" class="tr-action" title="AI action">
            <option value="">Action…</option>
            <option value="summarize">Summarize</option>
            <option value="shorter">Make shorter</option>
            <option value="longer">Make longer</option>
            <optgroup label="Change tone">
              <option value="tone:formal">Formal</option>
              <option value="tone:casual">Casual</option>
              <option value="tone:inspirational">Inspirational</option>
              <option value="tone:humor">Humor</option>
              <option value="tone:sarcastic">Sarcastic</option>
            </optgroup>
            <optgroup label="Format">
              <option value="format:paragraph">Paragraph</option>
              <option value="format:list">List</option>
              <option value="format:business">Business</option>
              <option value="format:academic">Academic</option>
              <option value="format:marketing">Marketing</option>
              <option value="format:poetry">Poetry</option>
            </optgroup>
          </select>
          <button id="tr-action-btn" class="tr-btn" title="Execute action">
            <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" width="14" height="14"><path d="M8 2v8M5 7l3 3 3-3M3 14h10"/></svg>
          </button>
          <select id="tr-lang" class="tr-lang" title="Translate to"></select>
          <button id="tr-btn" class="tr-btn" title="Translate">
            <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" width="14" height="14"><path d="M2 4h7M5 2v2M7.5 4S7 7 4 9.5M4.5 7c-.5 1.5-1.8 2.7-3 3.2M14 13c0-2-1.5-4.5-4-4.5S6 11 6 13s1.5 4.5 4 4.5 4-2.5 4-4.5z"/></svg>
          </button>
        </div>
        <div class="tr-result-wrap hidden" id="tr-result-wrap">
          <button id="tr-copy" class="tr-copy" title="Copy translation">
            <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round" width="12" height="12"><rect x="2.5" y="3" width="11" height="11" rx="1.5"/><path d="M5.5 3v-1a1 1 0 0 1 1-1h3a1 1 0 0 1 1 1v1"/></svg>
          </button>
          <div class="tr-result" id="tr-result"></div>
        </div>
      </div>
    </div>
  `;
}

render();

const overlay = document.getElementById("overlay")!;
const wordDisplay = document.getElementById("word-display")!;
const chipsContainer = document.getElementById("chips")!;
const trSection = document.getElementById("tr-section")!;
const trLang = document.getElementById("tr-lang") as HTMLSelectElement;
const trBtn = document.getElementById("tr-btn") as HTMLButtonElement;
const trText = document.getElementById("tr-text") as HTMLTextAreaElement;
const trResult = document.getElementById("tr-result")!;
const trResultWrap = document.getElementById("tr-result-wrap")!;
const trCopy = document.getElementById("tr-copy") as HTMLButtonElement;
const settingsBtn = document.getElementById("settings-btn") as HTMLButtonElement;
const trAction = document.getElementById("tr-action") as HTMLSelectElement;
const trActionBtn = document.getElementById("tr-action-btn") as HTMLButtonElement;

// Populate language select
for (const [code, name] of TRANSLATION_LANGS) {
  const opt = document.createElement("option");
  opt.value = code;
  opt.textContent = name;
  trLang.appendChild(opt);
}

const FADE_MS = 300;
let IDLE_MS = 4_000;

const win = getCurrentWindow();

let idleTimer: ReturnType<typeof setTimeout> | null = null;
let isVisible = false;
let isHovering = false;
let userEditedText = false;
let baseHeight = 150;
let baseWidth = 400;
void baseWidth;

document.body.addEventListener("mouseenter", () => {
  isHovering = true;
  if (isVisible) {
    scheduleHide();
  }
});
document.body.addEventListener("mouseleave", () => {
  isHovering = false;
  if (isVisible) {
    scheduleHide();
  }
});

// Track when user manually edits the textarea
let resizeTimer: ReturnType<typeof setTimeout> | null = null;
let lastTargetHeight = -1;
trText.addEventListener("input", () => {
  userEditedText = true;
  autoResize();
});

function autoResize() {
  if (resizeTimer) {
    clearTimeout(resizeTimer);
  }
  resizeTimer = setTimeout(() => {
    const contentHeight = overlay.scrollHeight;
    const targetHeight = Math.max(baseHeight, contentHeight);
    if (targetHeight === lastTargetHeight) {
      return;
    }
    win.outerSize().then((size) => {
      if (Math.abs(targetHeight - size.height) <= 10) {
        return;
      }
      lastTargetHeight = targetHeight;
      win.outerPosition().then((pos) => {
        const delta = targetHeight - size.height;
        win.setSize(new LogicalSize(size.width, targetHeight));
        if (delta !== 0) {
          win.setPosition(new LogicalPosition(pos.x, pos.y - delta));
        }
      });
    }).catch(() => {});
  }, 150);
}

function showOverlay() {
  if (idleTimer) {
    clearTimeout(idleTimer);
  }
  overlay.classList.remove("fading");
  if (!isVisible) {
    isVisible = true;
    win.show().catch(() => {});
  }
  overlay.classList.remove("hidden");
  scheduleHide();
}

function scheduleHide() {
  if (idleTimer) {
    clearTimeout(idleTimer);
  }
  const timeout = isHovering ? 30_000 : IDLE_MS;
  idleTimer = setTimeout(() => {
    overlay.classList.add("fading");
    setTimeout(() => {
      overlay.classList.add("hidden");
      overlay.classList.remove("fading");
      isVisible = false;
      win.hide().catch(() => {});
    }, FADE_MS);
  }, timeout);
}

function hideOverlay() {
  if (idleTimer) {
    clearTimeout(idleTimer);
    idleTimer = null;
  }
  overlay.classList.add("hidden");
  overlay.classList.remove("fading");
  isVisible = false;
  win.hide().catch(() => {});
}

function showWord(word: string) {
  wordDisplay.textContent = word;
}

function showLoading(word: string) {
  wordDisplay.style.opacity = "1";
  showWord(word);
  chipsContainer.innerHTML = "";
  const count = 6;
  for (let i = 0; i < count; i++) {
    const chip = document.createElement("button");
    chip.className = "chip skeleton";
    chip.disabled = true;
    chip.style.animationDelay = `${i * 40}ms`;
    chipsContainer.appendChild(chip);
  }
  showOverlay();
}

function makeChip(word: string, index: number): HTMLElement {
  const wrap = document.createElement("span");
  wrap.className = "chip-wrap animate-slide-up";
  wrap.style.animationDelay = `${index * 40}ms`;

  const btn = document.createElement("button");
  btn.className = "chip-word";
  btn.textContent = word;
  btn.onclick = () => {
    invoke("accept_text", { text: word }).catch(console.error);
    hideOverlay();
  };
  wrap.appendChild(btn);

  const sep = document.createElement("span");
  sep.className = "chip-sep";
  wrap.appendChild(sep);

  const copyBtn = document.createElement("button");
  copyBtn.className = "chip-copy";
  copyBtn.title = "Copy to clipboard";
  copyBtn.innerHTML = `<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round" width="12" height="12"><rect x="2.5" y="3" width="11" height="11" rx="1.5"/><path d="M5.5 3v-1a1 1 0 0 1 1-1h3a1 1 0 0 1 1 1v1"/></svg>`;
  const okSvg = `<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" width="12" height="12"><path d="M3 8l3 3 7-7"/></svg>`;
  copyBtn.onclick = (e) => {
    e.stopPropagation();
    invoke("copy_text", { text: word }).catch(console.error);
    copyBtn.innerHTML = okSvg;
    copyBtn.classList.add("copied");
    setTimeout(() => {
      copyBtn.innerHTML = `<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round" width="12" height="12"><rect x="2.5" y="3" width="11" height="11" rx="1.5"/><path d="M5.5 3v-1a1 1 0 0 1 1-1h3a1 1 0 0 1 1 1v1"/></svg>`;
      copyBtn.classList.remove("copied");
    }, 1200);
  };
  wrap.appendChild(copyBtn);

  return wrap;
}

function showSuggestions(data: SuggestionPayload) {
  showWord(data.word);
  chipsContainer.innerHTML = "";
  data.corrections.forEach((word, i) => {
    if (word) {
      chipsContainer.appendChild(makeChip(word, i));
    }
  });

  if (data.full_text && !userEditedText) {
    trText.value = data.full_text;
    autoResize();
  }

  showOverlay();
}

function showResult(text: string) {
  trResult.textContent = text;
  trResultWrap.classList.remove("hidden");
  trCopy.style.display = text && !text.startsWith("Error") ? "" : "none";
  autoResize();
}

async function runLLM(
  loadingText: string,
  fn: () => Promise<string>,
) {
  trBtn.classList.add("loading");
  trResult.textContent = loadingText;
  trResultWrap.classList.remove("hidden");
  trCopy.style.display = "none";
  autoResize();

  try {
    const result = await fn();
    showResult(result);
  } catch (e) {
    showResult(`Error: ${e}`);
  } finally {
    trBtn.classList.remove("loading");
  }
  autoResize();
  showOverlay();
}

// Translation
trBtn.onclick = async () => {
  const text = trText.value.trim();
  if (!text) return;
  await runLLM("Translating…", () =>
    invoke<string>("translate_text", { text, language: trLang.value })
  );
};

// AI action
trActionBtn.onclick = async () => {
  const text = trText.value.trim();
  if (!text) return;
  const action = trAction.value;
  if (!action) return;
  await runLLM("Processing…", () =>
    invoke<string>("process_text", { text, action })
  );
};

// Copy translation result
trCopy.onclick = (e) => {
  e.stopPropagation();
  const text = trResult.textContent;
  if (!text) return;
  invoke("copy_text", { text }).catch(console.error);
  const okSvg = `<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" width="12" height="12"><path d="M3 8l3 3 7-7"/></svg>`;
  trCopy.innerHTML = okSvg;
  trCopy.classList.add("copied");
  setTimeout(() => {
    trCopy.innerHTML = `<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round" width="12" height="12"><rect x="2.5" y="3" width="11" height="11" rx="1.5"/><path d="M5.5 3v-1a1 1 0 0 1 1-1h3a1 1 0 0 1 1 1v1"/></svg>`;
    trCopy.classList.remove("copied");
  }, 1200);
};

// Settings — opens independent window
settingsBtn.onclick = (e) => {
  e.stopPropagation();
  invoke("open_settings").catch(console.error);
};

// Save window geometry on move/resize
win.onMoved(({ payload: pos }) => {
  win.outerSize().then((size) => {
    invoke("save_window_position", {
      x: pos.x,
      y: pos.y,
      width: size.width,
      height: size.height,
    }).catch(() => {});
  });
});
win.onResized(({ payload: size }) => {
  win.outerPosition().then((pos) => {
    invoke("save_window_position", {
      x: pos.x,
      y: pos.y,
      width: size.width,
      height: size.height,
    }).catch(() => {});
  });
});

listen<PlaceholderPayload>("plume:show", (evt) => {
  showLoading(evt.payload.word);
});

listen<void>("plume:hide", () => {
  hideOverlay();
});

listen<SuggestionPayload>("plume:suggestions", (evt) => {
  showSuggestions(evt.payload);
});

invoke<[number, boolean, string]>("on_overlay_ready").then(async ([idle, trEnabled, trLangStr]) => {
  if (typeof idle === "number" && idle > 0) {
    IDLE_MS = idle * 1000;
  }

  const cfg = await invoke<{ window: { width: number; height: number } }>("get_config");
  baseWidth = cfg.window.width || 400;
  baseHeight = cfg.window.height || 150;

  if (trEnabled) {
    trSection.classList.remove("hidden");
    if (trLangStr) {
      const match = TRANSLATION_LANGS.find(([c]) => c === trLangStr);
      if (match) {
        trLang.value = trLangStr;
      }
    }
  } else {
    trSection.classList.add("hidden");
  }
}).catch(() => {});
