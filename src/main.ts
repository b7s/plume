import "./index.css";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalSize, LogicalPosition } from "@tauri-apps/api/window";

interface SuggestionPayload {
  corrections: string[];
  translation?: string;
  word: string;
  full_text: string;
  ai_words?: string[];
  window_title?: string;
}

interface PlaceholderPayload {
  corrections: string[];
  word: string;
  full_text: string;
  window_title?: string;
}

const TRANSLATION_LANGS: [string, string][] = [
  ["portuguese", "Português"],
  ["english", "English"],
  ["spanish", "Español"],
  ["french", "Français"],
  ["german (Deutsch)", "Deutsch"],
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
    <div class="card" id="overlay">
      <div class="word-row" id="word-row">
        <span id="word-display"></span>
        <button id="minimize-btn" class="settings-btn" title="Hide window">
          <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" width="14" height="14"><path d="M3 8h10"/></svg>
        </button>
        <button id="settings-btn" class="settings-btn" title="Settings">
          <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" width="14" height="14"><path d="M8 10.5a2.5 2.5 0 1 0 0-5 2.5 2.5 0 0 0 0 5z"/><path d="M13.5 8a5.5 5.5 0 0 0-.1-1l1.4-1.1-1.5-2.6-1.7.7a5.3 5.3 0 0 0-1.7-1l-.3-1.8h-3l-.3 1.8a5.3 5.3 0 0 0-1.7 1l-1.7-.7L1.4 5.9 2.8 7a5.5 5.5 0 0 0 0 2l-1.4 1.1 1.5 2.6 1.7-.7a5.3 5.3 0 0 0 1.7 1l.3 1.8h3l.3-1.8a5.3 5.3 0 0 0 1.7-1l1.7.7 1.5-2.6-1.4-1.1a5.5 5.5 0 0 0 .1-1z"/></svg>
        </button>
      </div>
      <div class="chips-row" id="chips"></div>
      <span id="ai-loading" class="ai-loading hidden">✨</span>
      <div class="tr-section hidden" id="tr-section">
        <textarea id="tr-text" class="tr-text" placeholder="Type or auto-captured text will appear here…" rows="2"></textarea>
        <div class="tr-result-wrap hidden" id="tr-result-wrap">
        <button id="tr-copy" class="tr-copy" title="Copy translation">
          <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round" width="12" height="12"><rect x="2.5" y="3" width="11" height="11" rx="1.5"/><path d="M5.5 3v-1a1 1 0 0 1 1-1h3a1 1 0 0 1 1 1v1"/></svg>
        </button>
        <button id="tr-insert" class="tr-copy" title="Insert text">
          <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" width="12" height="12"><path d="M2 8h9M8 5l3 3-3 3"/><path d="M14 2v12"/></svg>
        </button>
        <div class="tr-result" id="tr-result"></div>
      </div>
    </div>
    <div class="tr-toolbar" id="tr-toolbar">
        <div class="tr-col">
          <select id="tr-action" class="tr-action" title="AI action">
            <option value="">Action…</option>
            <option value="summarize">Summarize</option>
            <option value="shorter">Make shorter</option>
            <option value="longer">Make longer</option>
            <option value="grammar">Correct grammar</option>
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
            <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" width="14" height="14"><path d="M2 8h10M9 5l3 3-3 3"/></svg>
          </button>
        </div>
        <div class="tr-col tr-col-right">
          <button id="tr-explain-btn" class="tr-btn" title="Explain this text">
            <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" width="14" height="14"><path d="M8 6v3M8 12h0"/><circle cx="8" cy="8" r="6"/><path d="M5 5s.5-2 3-2 3 2 2 3.5C9.5 8 8 8 8 9.5"/></svg>
          </button>
          <select id="tr-lang" class="tr-lang" title="Translate to"></select>
          <button id="tr-btn" class="tr-btn" title="Translate">
            <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" width="14" height="14"><path d="M2 4h7M5 2v2M7.5 4S7 7 4 9.5M4.5 7c-.5 1.5-1.8 2.7-3 3.2M14 13c0-2-1.5-4.5-4-4.5S6 11 6 13s1.5 4.5 4 4.5 4-2.5 4-4.5z"/></svg>
          </button>
        </div>
    </div>
  `;
}

render();

// Disable the browser/webview right-click context menu app-wide.
document.addEventListener("contextmenu", (e) => e.preventDefault());

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
const trInsert = document.getElementById("tr-insert") as HTMLButtonElement;
const settingsBtn = document.getElementById("settings-btn") as HTMLButtonElement;
const minimizeBtn = document.getElementById("minimize-btn") as HTMLButtonElement;
const trAction = document.getElementById("tr-action") as HTMLSelectElement;
const trActionBtn = document.getElementById("tr-action-btn") as HTMLButtonElement;
const trExplainBtn = document.getElementById("tr-explain-btn") as HTMLButtonElement;
const aiLoading = document.getElementById("ai-loading")!;

// Populate language select
for (const [code, name] of TRANSLATION_LANGS) {
  const opt = document.createElement("option");
  opt.value = code;
  opt.textContent = name;
  trLang.appendChild(opt);
}

// Suppress text capture while any form field in the Plume UI is
// interacted with. WebView2 native dropdown popups (e.g. <select>) can
// bypass PID-based self-exclusion and get captured as if the user typed
// the option text. Same for inputs/textarea — we don't want those triggers.
let suppressTimer: ReturnType<typeof setTimeout> | null = null;
let suppressCapture = false;

function setSuppress(ms: number) {
  suppressCapture = true;
  if (suppressTimer !== null) clearTimeout(suppressTimer);
  suppressTimer = setTimeout(() => { suppressCapture = false; }, ms);
}

document.addEventListener("focusin", (e) => {
  const target = e.target as HTMLElement;
  if (target.matches("input, select, textarea")) {
    const delay = target.matches("select") ? 2000 : 500;
    setSuppress(delay);
  }
});

// Once a select option is chosen the dropdown closes — resume sooner.
document.addEventListener("change", (e) => {
  const target = e.target as HTMLElement;
  if (target.matches("select")) {
    setSuppress(300);
  }
});

// Cross-window: the settings window emits this when its form elements
// receive focus, so the overlay suppresses capture too.
listen("plume:ui-interaction", () => {
  setSuppress(2000);
});

// Settings window signals a select option was picked — resume sooner.
listen("plume:ui-change-done", () => {
  setSuppress(300);
});

let IDLE_MS = 6_000;

const win = getCurrentWindow();

let idleTimer: ReturnType<typeof setTimeout> | null = null;
let isVisible = false;
let userEditedText = false;
let initialized = false;

// Config-driven values (loaded from get_config on startup)
let MIN_HEIGHT = 250;
let MAX_HEIGHT = 800;
let MIN_WIDTH = 400;
let MAX_WIDTH = 800;
let RESIZE_DEBOUNCE_MS = 250;

// AI next-word suggestions
let aiSuggestionTimer: ReturnType<typeof setTimeout> | null = null;
let aiSuggestionDelay = 800;

let windowOpacity = 100;
let autoHide = true;

function applyOpacity() {
  const val = windowOpacity / 100;
  const isDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  const r = isDark ? 25 : 245;
  const g = isDark ? 25 : 245;
  const b = isDark ? 25 : 245;
  document.body.style.background = `rgba(${r}, ${g}, ${b}, ${val})`;
  invoke("set_window_opacity", { opacity: windowOpacity }).catch(() => {});
}

// Track when user manually edits the textarea
let resizeTimer: ReturnType<typeof setTimeout> | null = null;
let lastTargetHeight = -1;
trText.addEventListener("input", () => {
  userEditedText = true;
  autoResize();
});

// The overlay window is WS_EX_NOACTIVATE so it never steals focus while you
// type in another app — but that also blocks keystrokes from reaching this
// textarea. When the textarea is interacted with we make the window
// focusable/active so you can type freely; when focus leaves it we restore
// the no-activate behavior so chip clicks keep working as before.
let overlayActivatable = false;
function setOverlayActivatable(v: boolean) {
  if (overlayActivatable === v) return;
  overlayActivatable = v;
  invoke("set_window_activatable", { activatable: v }).catch(() => {});
}
trText.addEventListener("pointerdown", () => setOverlayActivatable(true));
trText.addEventListener("focus", () => setOverlayActivatable(true));
trText.addEventListener("blur", () => setOverlayActivatable(false));

// When the window loses focus, restore WS_EX_NOACTIVATE so the
// textarea no longer captures keyboard input.
win.listen("tauri://blur", () => {
  setOverlayActivatable(false);
});
window.addEventListener("blur", () => {
  setOverlayActivatable(false);
});

function autoResize() {
  if (resizeTimer) {
    clearTimeout(resizeTimer);
  }
  resizeTimer = setTimeout(() => {
    const contentHeight = overlay.scrollHeight;
    const targetHeight = Math.min(Math.max(MIN_HEIGHT, contentHeight), MAX_HEIGHT);
    if (targetHeight === lastTargetHeight) {
      return;
    }
    win.scaleFactor().then((scale) => {
      win.outerSize().then((size) => {
        const logicalH = size.height / scale;
        if (Math.abs(targetHeight - logicalH) <= 10) {
          return;
        }
        lastTargetHeight = targetHeight;
        const logicalW = Math.min(Math.max(size.width / scale, MIN_WIDTH), MAX_WIDTH);
        win.outerPosition().then((pos) => {
          const logicalX = pos.x / scale;
          const logicalY = pos.y / scale;
          const delta = targetHeight - logicalH;
          win.setSize(new LogicalSize(logicalW, targetHeight));
          if (delta !== 0) {
            win.setPosition(new LogicalPosition(logicalX, logicalY - delta));
          }
        });
      });
    }).catch(() => {});
  }, RESIZE_DEBOUNCE_MS);
}

function showOverlay() {
  if (idleTimer) {
    clearTimeout(idleTimer);
    idleTimer = null;
  }
  if (document.activeElement !== trText) {
    setOverlayActivatable(false);
  }
  if (!isVisible) {
    isVisible = true;
    win.show().catch(() => {});
  }
  applyOpacity();
  scheduleHide();
}

function scheduleHide() {
  if (idleTimer) {
    clearTimeout(idleTimer);
  }
  if (!autoHide) return;
  idleTimer = setTimeout(() => {
    isVisible = false;
    setOverlayActivatable(false);
    win.hide().catch(() => {});
  }, IDLE_MS);
}

function hideOverlay() {
  if (idleTimer) {
    clearTimeout(idleTimer);
    idleTimer = null;
  }
  isVisible = false;
  setOverlayActivatable(false);
  win.hide().catch(() => {});
}

function showWord(title: string) {
  wordDisplay.textContent = title;
}

function showLoading(title: string) {
  wordDisplay.style.opacity = "1";
  showWord(title);
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

function makeChip(word: string, index: number, isAi = false): HTMLElement {
  const wrap = document.createElement("span");
  wrap.className = "chip-wrap animate-slide-up" + (isAi ? " chip-ai" : "");
  wrap.style.animationDelay = `${index * 40}ms`;

  const btn = document.createElement("button");
  btn.className = "chip-word";
  btn.textContent = word;
  if (isAi) {
    btn.title = "AI suggestion";
  }
  btn.onclick = () => {
    invoke("accept_text", { text: word }).catch(console.error);
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
  showWord(data.window_title || data.word);
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

  // Schedule AI next-word suggestions after typing stops
  scheduleAiSuggestions(data.full_text);

  showOverlay();
}

function scheduleAiSuggestions(text: string) {
  if (aiSuggestionTimer) {
    clearTimeout(aiSuggestionTimer);
  }
  if (!text || text.trim().length < 2) {
    aiLoading.classList.add("hidden");
    return;
  }
  aiLoading.classList.remove("hidden");
  aiSuggestionTimer = setTimeout(() => {
    invoke<string[]>("suggest_next_words", { text })
      .then((words) => {
        aiLoading.classList.add("hidden");
        if (words && words.length > 0) {
          appendAiChips(words);
        }
      })
      .catch((e) => {
        aiLoading.classList.add("hidden");
        console.error("suggest_next_words failed:", e);
      });
  }, aiSuggestionDelay);
}

function appendAiChips(words: string[]) {
  // Remove existing AI chips
  chipsContainer.querySelectorAll(".chip-ai").forEach((el) => el.remove());

  const existingCount = chipsContainer.children.length;
  words.forEach((word, i) => {
    if (word) {
      chipsContainer.appendChild(makeChip(word, existingCount + i, true));
    }
  });
  autoResize();
}

function showResult(text: string) {
  trResult.textContent = text;
  trResultWrap.classList.remove("hidden");
  const hasText = text && !text.startsWith("Error");
  trCopy.style.display = hasText ? "" : "none";
  trInsert.style.display = hasText ? "" : "none";
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
  trInsert.style.display = "none";
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

// Explain
trExplainBtn.onclick = async () => {
  const text = trText.value.trim();
  if (!text) return;
  await runLLM("Explaining…", () =>
    invoke<string>("explain_text", { text })
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

// Insert translation result into the focused app
trInsert.onclick = (e) => {
  e.stopPropagation();
  const text = trResult.textContent;
  if (!text) return;
  invoke("accept_text", { text }).catch(console.error);
  hideOverlay();
};

// Settings — opens independent window
settingsBtn.onclick = (e) => {
  e.stopPropagation();
  invoke("open_settings").catch((err) => console.error("open_settings failed:", err));
};

// Minimize — hides the overlay window
minimizeBtn.onclick = (e) => {
  e.stopPropagation();
  hideOverlay();
};

// Save window geometry on move/resize
// Tauri event payloads are in physical pixels — convert to logical for storage
win.onMoved(({ payload: pos }) => {
  if (!initialized) return;
  win.outerSize().then((size) => {
    win.scaleFactor().then((scale) => {
      invoke("save_window_position", {
        x: pos.x / scale,
        y: pos.y / scale,
        width: size.width / scale,
        height: size.height / scale,
      }).catch(() => {});
    }).catch(() => {});
  });
});
win.onResized(({ payload: size }) => {
  if (!initialized) return;
  win.outerPosition().then((pos) => {
    win.scaleFactor().then((scale) => {
      invoke("save_window_position", {
        x: pos.x / scale,
        y: pos.y / scale,
        width: size.width / scale,
        height: size.height / scale,
      }).catch(() => {});
    }).catch(() => {});
  });
});

listen<PlaceholderPayload>("plume:show", (evt) => {
  if (suppressCapture) return;
  showLoading(evt.payload.window_title || evt.payload.word);
});

listen<void>("plume:hide", () => {
  if (suppressCapture) return;
  hideOverlay();
});

listen<SuggestionPayload>("plume:suggestions", (evt) => {
  if (suppressCapture) return;
  showSuggestions(evt.payload);
});

listen<number>("plume:config-updated", async () => {
  const cfg = await invoke<{ window_opacity: number; auto_hide: boolean }>("get_config");
  if (typeof cfg.window_opacity === "number" && cfg.window_opacity >= 0 && cfg.window_opacity <= 100) {
    windowOpacity = cfg.window_opacity;
  }
  if (typeof cfg.auto_hide === "boolean") {
    autoHide = cfg.auto_hide;
  }
  if (!autoHide && idleTimer) {
    clearTimeout(idleTimer);
    idleTimer = null;
  }
  applyOpacity();
});

invoke<[number, boolean, string]>("on_overlay_ready").then(async ([idle, trEnabled, trLangStr]) => {
  if (typeof idle === "number" && idle > 0) {
    IDLE_MS = idle * 1000;
  }

  const cfg = await invoke<{
    window: { min_height: number; max_height: number; min_width: number; max_width: number };
    ai_suggestion_delay: number;
    resize_debounce_ms: number;
    window_opacity: number;
    hide_during_fullscreen: boolean;
    auto_hide: boolean;
  }>("get_config");
  MIN_HEIGHT = cfg.window.min_height || 250;
  MAX_HEIGHT = cfg.window.max_height || 800;
  MIN_WIDTH = cfg.window.min_width || 400;
  MAX_WIDTH = cfg.window.max_width || 800;
  if (cfg.ai_suggestion_delay) {
    aiSuggestionDelay = cfg.ai_suggestion_delay;
  }
  if (cfg.resize_debounce_ms) {
    RESIZE_DEBOUNCE_MS = cfg.resize_debounce_ms;
  }
  if (typeof cfg.window_opacity === "number" && cfg.window_opacity >= 0 && cfg.window_opacity <= 100) {
    windowOpacity = cfg.window_opacity;
    applyOpacity();
  }
  if (typeof cfg.auto_hide === "boolean") {
    autoHide = cfg.auto_hide;
  }

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

  initialized = true;
}).catch(() => {});
