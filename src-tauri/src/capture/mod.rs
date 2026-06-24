use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use uiautomation::{
    UIAutomation, UIElement,
    patterns::{UITextPattern, UIValuePattern},
};

use crate::engine;
use crate::{PlaceholderPayload, SuggestionPayload};

const DEBOUNCE_MS: u64 = 300;
const MIN_WORD_LEN: usize = 1;

/// Mutable state for the capture loop (shared between the thread and async tasks).
struct CaptureState {
    /// Last word that triggered a suggestion — used for de-duplication.
    last_word: String,
    /// Full text from last poll — used to detect actual typing vs selection.
    last_full_text: String,
    /// HWND of the last focused control — detects context switches.
    last_hwnd: isize,
    /// Timestamp of the last text change — used for debounce.
    last_change: Instant,
    /// Whether an AI request is currently in flight — prevents overlapping calls.
    busy: AtomicBool,
}

pub fn start(app: AppHandle) {
    let automation = match UIAutomation::new() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("[plume] UIA init failed: {e}");
            return;
        }
    };

    let state_capture = Arc::new(Mutex::new(CaptureState {
        last_word: String::new(),
        last_full_text: String::new(),
        last_hwnd: 0,
        last_change: Instant::now() - Duration::from_secs(60),
        busy: AtomicBool::new(false),
    }));

    loop {
        std::thread::sleep(Duration::from_millis(100));

        if let Ok(focused) = automation.get_focused_element() {
            // Save HWND so insert_via_uia can target the right control
            // even if Plume's WebView2 briefly claims UIA focus on click.
            let hwnd_int = focused
                .get_native_window_handle()
                .ok()
                .map(|h| -> isize { h.into() })
                .unwrap_or(0);

            if hwnd_int != 0 {
                if let Ok(mut guard) = crate::LAST_FOCUSED_HWND.lock() {
                    *guard = Some(hwnd_int);
                }
            }

            if let Some(text) = read_text(&focused) {
                let app = app.clone();
                let st = state_capture.clone();
                handle_text(app, st, text, hwnd_int);
            }
        }
    }
}

fn read_text(element: &UIElement) -> Option<String> {
    if let Ok(text_pattern) = element.get_pattern::<UITextPattern>() {
        if let Ok(ranges) = text_pattern.get_visible_ranges() {
            for range in ranges {
                if let Ok(text) = range.get_text(-1) {
                    if !text.is_empty() {
                        return Some(text);
                    }
                }
            }
        }
        if let Ok(selection) = text_pattern.get_selection() {
            for range in selection {
                if let Ok(text) = range.get_text(-1) {
                    if !text.is_empty() {
                        return Some(text);
                    }
                }
            }
        }
        return None;
    }

    if let Ok(value_pattern) = element.get_pattern::<UIValuePattern>() {
        if let Ok(text) = value_pattern.get_value() {
            if !text.is_empty() {
                return Some(text);
            }
        }
    }

    None
}

fn extract_word(text: &str) -> Option<String> {
    let word = text
        .split_whitespace()
        .last()?
        .trim_end_matches(|c: char| c.is_ascii_punctuation());

    if word.len() >= MIN_WORD_LEN {
        Some(word.to_lowercase())
    } else {
        None
    }
}

fn handle_text(app: AppHandle, st: Arc<Mutex<CaptureState>>, text: String, hwnd: isize) {
    let now = Instant::now();
    // For terminals, the full text is the entire screen buffer.
    // Use only the last line for typing detection — it's the input line
    // in terminals and the line being typed in regular editors.
    let compare_text = text
        .lines()
        .last()
        .unwrap_or(&text)
        .to_string();

    let should_fire = {
        let mut guard = match st.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        // Context switch: HWND changed → tab/app/link switch, not typing.
        if hwnd != guard.last_hwnd {
            guard.last_hwnd = hwnd;
            guard.last_full_text = compare_text.clone();
            guard.last_word = String::new();
            guard.last_change = now;
            return;
        }

        let old_len = guard.last_full_text.len();
        let is_prefix = compare_text.starts_with(guard.last_full_text.as_str());

        // Always update last_full_text so next poll compares against current text
        guard.last_full_text = compare_text.clone();

        // Text didn't grow → not typing (deletion, selection, cursor move)
        if compare_text.len() <= old_len {
            return;
        }

        // Text grew but old text isn't a prefix → content replaced (navigation, paste-over)
        // Still extract the word, just reset the dedup so we can suggest for the new context
        if !is_prefix {
            guard.last_word = String::new();
        }

        // Large jump → paste, not typing
        let added = compare_text.len() - old_len;
        if added > 5 {
            return;
        }

        let word = match extract_word(&compare_text) {
            Some(w) => w,
            None => return,
        };

        // Same word as last suggestion → skip
        if guard.last_word == word {
            return;
        }

        // Don't fire if a suggestion is already in flight — but still update word
        // so the next poll after busy clears can fire if the word changed again
        if guard.busy.load(Ordering::Relaxed) {
            return;
        }

        let elapsed = now.duration_since(guard.last_change);
        if elapsed < Duration::from_millis(DEBOUNCE_MS) {
            return;
        }

        guard.last_word = word.clone();
        guard.last_change = now;
        guard.busy.store(true, Ordering::Relaxed);
        (true, word, text.clone())
    };

    if let (true, word, full_text) = should_fire {
        let _ = app.emit(
            "plume:show",
            PlaceholderPayload {
                corrections: Vec::new(),
                word: word.clone(),
                full_text: full_text.clone(),
            },
        );

        let app2 = app.clone();
        let st2 = st.clone();
        tauri::async_runtime::spawn(async move {
            let result = run_suggestion(&app2, &word, &full_text).await;
            if let Ok(g) = st2.lock() {
                g.busy.store(false, Ordering::Relaxed);
            }
            if let Err(e) = result {
                eprintln!("[plume] suggestion failed: {e}");
                let _ = app2.emit("plume:hide", ());
            }
        });
    }
}

async fn run_suggestion(app: &AppHandle, word: &str, full_text: &str) -> Result<(), String> {
    let cfg = crate::config::load();
    let count = cfg.suggestion_count.max(1);
    let response = engine::get_suggestions(word, count).await?;

    let payload = SuggestionPayload {
        corrections: response.corrections,
        translation: None,
        word: word.to_string(),
        full_text: full_text.to_string(),
    };

    let _ = app.emit("plume:suggestions", payload);
    Ok(())
}
