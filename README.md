<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="./app-icon-dark.png">
    <source media="(prefers-color-scheme: light)" srcset="./app-icon-light.png">
    <img alt="Plume" src="./app-icon.png" width="128" height="128">
  </picture>

  <h1>Plume</h1>
</div>

**Floating typing assistant for Windows.** Plume sits as a transparent overlay above your active window, listens via UI Automation (no keyloggers), and serves up smart suggestions — Hunspell-powered spelling corrections, AI next-word predictions, translation, and text actions.

| | |
|---|---|
| **Capture** | UI Automation only — reads committed text from any text field. No `WH_KEYBOARD_LL`, no `GetAsyncKeyState`. |
| **Corrections** | Hunspell dictionaries with prefix matching — works fully offline. |
| **AI Suggestions** | Next-word prediction using a local LLM (llama.cpp) or cloud APIs (OpenAI, Ollama, custom). |
| **Translation** | Translate captured text into any language. Rephrase, simplify, change tone. |
| **Privacy** | Never records keystrokes. All processing is in-memory. Network egress only when you opt into a remote API. |

## How It Works

Plume uses **Windows UI Automation** (`UIAutomation` `TextPattern` / `ValuePattern`) to detect what you're typing. When you stop typing, it:

1. Reads the last word from the focused text field
2. Runs it through **Hunspell** for spelling corrections
3. Sends the surrounding context to your chosen **LLM** for next-word suggestions
4. Shows the results in a floating **transparent overlay** — click a chip to copy or insert

The overlay stays on top of all windows, auto-hides after a configurable idle timeout, and repositions with your active window.

## Quick Start

### Prerequisites

- Windows 10 or 11
- [WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) (usually pre-installed on Win11)
- For local AI: a GGUF model file

### Install

Download the latest `.exe` installer from [Releases](https://github.com/b7s/plume/releases).

### First Run

1. Launch Plume — it starts in the system tray
2. Click the tray icon to show/hide the overlay
3. Start typing in any app — suggestions appear automatically

## AI Providers

Plume supports four backends. Configure yours in **Settings** (gear icon on the overlay).

### Local (llama.cpp)

Uses a bundled `llama-server.exe` to run GGUF models locally. The model file is auto-downloaded from HuggingFace on first use.

| Preset | Size | Notes |
|---|---|---|
| **Qwen3-0.6B (Q8_0)** | 639 MB | Recommended — near-lossless, great for text continuation |
| Qwen2.5-0.5B (Q8_0) | 507 MB | Smallest, lowest RAM |
| Llama 3.2-1B (IQ3_M) | 627 MB | Multilingual |
| Gemma 3-1B (Q4_K_M) | 769 MB | Efficient on CPU |
| Qwen2.5-1.5B (Q4_K_M) | 940 MB | Higher quality |
| Qwen3-1.7B (Q4_K_M) | ~1.1 GB | Higher capability |
| Gemma 2-2B (IQ3_M) | 1.33 GB | Most capable |

Select **Custom...** to type any GGUF filename. Built-in presets download from verified HuggingFace URLs; custom filenames auto-resolve from unsloth.

### Ollama

Connect to a local or remote [Ollama](https://ollama.com) instance. Supports any model you've pulled.

| Preset | Notes |
|---|---|
| `qwen3:0.6b` | Lightweight, fast |
| `qwen3:1.7b` | More capable |
| `llama3.2:1b` | Multilingual |
| `tinyllama:1.1b` | Compact |
| `smollm2:1.7b` | Code-aware |
| `phi3:mini` | Microsoft's small model |

### OpenAI-compatible

Connect to OpenAI or any OpenAI-compatible API (together.ai, Groq, etc.). Set your API key and endpoint.

| Preset | Input/Output (per 1M tokens) |
|---|---|
| GPT-5.4 Nano | $0.20 / $1.25 |
| GPT-5.4 Mini | $0.75 / $4.50 |
| GPT-4.1 Mini | $0.40 / $1.60 |
| GPT-4o Mini | $0.15 / $0.60 |
| GPT-5.5 | $5.00 / $30.00 |
| GPT-5.5 Pro | $30.00 / $180.00 |

### Custom API

Bring your own endpoint with custom headers (JSON). Useful for self-hosted or proxy setups.

## Features

### Spelling Corrections

Hunspell parses `.dic`/`.aff` files directly — no external process. Corrections use prefix matching (binary search) for fast lookup. Click a correction to insert it. Click the copy icon to copy to clipboard.

### AI Next-Word Suggestions

After you stop typing, Plume sends the last ~10 words to the LLM and shows predicted next words as special chips with an "AI" indicator. The suggestion delay is configurable in Settings (default 800ms).

### Translation

Toggle translation in Settings. Select a target language, type or capture text, and click the translate button. Results can be copied or inserted directly.

### Text Actions

Select an action from the dropdown and apply it to the captured text:

- **Summarize**, **Make shorter**, **Make longer**
- **Correct grammar**, **Make sense**
- **Change tone**: Formal, Casual, Inspirational, Humor, Sarcastic
- **Format**: Paragraph, List, Business, Academic, Marketing, Poetry

### Window & Overlay

- Transparent, always-on-top, frameless window with rounded corners
- Mica material (Win11) with auto dark/light mode
- Draggable by any part of the overlay
- Configurable min/max size, idle timeout, hover timeout
- Position saved on move/resize, restored on restart
- System tray with left-click toggle and right-click menu (Show/Hide, Settings, Quit)

## Configuration

Plume stores settings in `%APPDATA%\plume\config.json`. You can edit this file directly — all fields use `#[serde(default)]`, so missing keys never break deserialization.

### Key settings

| Key | Default | Description |
|---|---|---|
| `provider` | `"local"` | `local`, `ollama`, `openai`, or `custom` |
| `model` | `"Qwen3-0.6B-Q8_0.gguf"` | Model name / GGUF filename |
| `endpoint` | `"http://127.0.0.1:8080"` | API endpoint |
| `port` | `8080` | Local llama-server port |
| `suggestion_count` | `6` | Number of spelling corrections |
| `ai_suggestion_count` | `3` | Number of AI next-word suggestions |
| `ai_suggestion_delay` | `800` | Delay before AI suggestions fire (ms) |
| `idle_timeout` | `6` | Seconds before overlay auto-hides |
| `dictionary.language` | `"en_US"` | Hunspell dictionary code |

## Keyboard Shortcuts

| Shortcut | Action |
|---|---|
| **Click chip** | Insert suggestion into active field |
| **Copy icon** | Copy suggestion to clipboard |
| **Gear icon** | Open Settings |
| **Tray left-click** | Toggle overlay visibility |
| **Tray right-click** | Menu: Settings, Quit |

## Building from Source

```
npm install
npm run tauri dev      # Development mode
npm run tauri build    # Production build
```

## Architecture

```
                        ┌─────────────┐
                        │   Overlay   │  WebView2 (transparent)
                        │   (TS/CSS)  │  floating window
                        └──────┬──────┘
                               │ Tauri IPC (events + commands)
                        ┌──────┴──────┐
                        │    Tauri    │  Rust backend
                        │   (lib.rs)  │
                        └──────┬──────┘
              ┌────────────────┼────────────────┐
        ┌─────┴─────┐  ┌───────┴─────┐  ┌───────┴─────┐
        │  Capture  │  │     AI /    │  │  Spellcheck │
        │   (UIA)   │  │    Engine   │  │  (Hunspell) │
        └───────────┘  └─────────────┘  └─────────────┘
                              │
                ┌─────────────┴───────────┐
                │  llama.cpp  │  API      │
                │  (local)    │ (cloud)   │
                └─────────────┴───────────┘
```

- **Capture** — `capture/mod.rs`: UIAutomation polling loop, reads TextPattern/ValuePattern, debounces
- **Engine** — `engine/mod.rs`: Word extraction, prompt assembly, result dispatch
- **AI** — `ai/mod.rs`: `LlmProvider` trait with local (llama.cpp) and remote (OpenAI/Ollama) implementations
- **Spellcheck** — `spellcheck/mod.rs`: Pure-Rust Hunspell .dic parser with binary search suggestions
- **Config** — `config/mod.rs`: JSON config with `#[serde(default)]` on every field, merge-on-save preserves unknown keys

## License

MIT
