use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use uiautomation::{
    UIAutomation, UIElement,
    patterns::{UITextPattern, UIValuePattern},
};

use crate::engine;
use crate::config;
use crate::{PlaceholderPayload, SuggestionPayload};

/// Get the title of the top-level window that owns the given HWND.
#[cfg(target_os = "windows")]
fn get_window_title(hwnd: isize) -> String {
    use windows::Win32::UI::WindowsAndMessaging::{GetAncestor, GetWindowTextW, GA_ROOT};
    use windows::Win32::Foundation::HWND;
    let hwnd = HWND(hwnd as *mut _);
    // Walk up to the top-level window — the focused element is usually a child control
    let root = unsafe { GetAncestor(hwnd, GA_ROOT) };
    let root = if root.0.is_null() { hwnd } else { root };
    let mut buf = [0u16; 512];
    let len = unsafe { GetWindowTextW(root, &mut buf) };
    if len > 0 {
        String::from_utf16_lossy(&buf[..len as usize])
    } else {
        String::new()
    }
}

#[cfg(not(target_os = "windows"))]
fn get_window_title(_hwnd: isize) -> String {
    String::new()
}

/// Mutable state for the capture loop (shared between the thread and async tasks).
struct CaptureState {
    /// Last word that triggered a suggestion — used for de-duplication.
    last_word: String,
    /// Full text from last poll — used to detect actual typing vs selection.
    last_full_text: String,
    /// HWND of the last focused control — detects context switches.
    last_hwnd: isize,
    /// Timestamp of the last text *change* — used for debounce (wait-after-pause).
    last_change: Instant,
    /// Timestamp of the last suggestion *fire* — used for throttle (rate-limit mid-typing).
    last_fire: Instant,
    /// Whether an AI request is currently in flight — prevents overlapping calls.
    busy: AtomicBool,
    /// True after a paste / large jump — suppress suggestions until real typing resumes.
    suppress: bool,
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
        last_fire: Instant::now() - Duration::from_secs(60),
        busy: AtomicBool::new(false),
        suppress: false,
    }));

    let plume_pid = std::process::id();

    loop {
        let cfg = config::load();
        std::thread::sleep(Duration::from_millis(cfg.poll_interval_ms));

        // Auto-hide when a fullscreen game or screen-sharing session is active
        if cfg.hide_during_fullscreen && is_fullscreen_active() {
            if let Some(window) = app.get_webview_window("plume") {
                if window.is_visible().unwrap_or(false) {
                    let _ = window.hide();
                }
            }
            continue;
        }

        if let Ok(focused) = automation.get_focused_element() {
            // Skip Plume's own window — typing in the textarea should not
            // trigger the capture loop (feedback loop).
            if let Ok(pid) = focused.get_process_id() {
                if pid == plume_pid {
                    continue;
                }
            }

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
                let title = get_window_title(hwnd_int);
                handle_text(app, st, text, hwnd_int, title, &cfg);
            }
        }
    }
}

fn read_text(element: &UIElement) -> Option<String> {
    // Try TextPattern first — used by rich text editors (Word, Notepad, browsers).
    // Only read visible ranges, NOT selection (selection = highlighted text, not typing).
    if let Ok(text_pattern) = element.get_pattern::<UITextPattern>() {
        if let Ok(ranges) = text_pattern.get_visible_ranges() {
            for range in ranges {
                if let Ok(text) = range.get_text(-1) {
                    if !text.is_empty() {
                        return Some(sanitize_text(&text));
                    }
                }
            }
        }
        return None;
    }

    // ValuePattern — used by simple input fields (text boxes, address bars).
    if let Ok(value_pattern) = element.get_pattern::<UIValuePattern>() {
        if let Ok(text) = value_pattern.get_value() {
            if !text.is_empty() {
                return Some(sanitize_text(&text));
            }
        }
    }

    None
}

/// Strip control characters and other non-typed artifacts from captured text.
fn sanitize_text(text: &str) -> String {
    text.chars()
        .filter(|c| {
            // Only strip ASCII control chars (0x00-0x1F except whitespace),
            // not Unicode control-class chars which include legitimate marks
            // used in Arabic, Indic, emoji sequences, etc.
            if c.is_ascii_control() {
                *c == '\n' || *c == '\r' || *c == '\t'
            } else {
                true
            }
        })
        .collect::<String>()
        .replace('\u{200b}', "")
        .replace('\u{200d}', "")
        .replace('\u{feff}', "")
}

fn extract_word(text: &str, min_len: usize) -> Option<String> {
    let word = text
        .split_whitespace()
        .last()?;

    if word.chars().count() >= min_len {
        Some(word.to_lowercase())
    } else {
        None
    }
}

fn handle_text(
    app: AppHandle,
    st: Arc<Mutex<CaptureState>>,
    text: String,
    hwnd: isize,
    window_title: String,
    cfg: &config::Config,
) {
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
            guard.last_fire = now;
            guard.suppress = false;
            return;
        }

        let prev = guard.last_full_text.clone();
        let changed = compare_text != prev;

        if changed {
            let is_prefix = compare_text.starts_with(prev.as_str());
            // Use char count, not byte count — accented chars are multi-byte
            // and a single keystroke (e.g. dead-key + letter) adds 2-3 bytes
            // but only 1 character.
            let added_chars = compare_text.chars().count().saturating_sub(prev.chars().count());
            guard.last_full_text = compare_text.clone();
            guard.last_change = now;

            if !is_prefix {
                guard.last_word = String::new();
                guard.suppress = added_chars > cfg.max_added_chars;
            } else if added_chars > cfg.max_added_chars {
                guard.suppress = true;
            } else {
                guard.suppress = false;
            }
        }

        if guard.suppress {
            return;
        }

        let word = match extract_word(&compare_text, cfg.min_word_len) {
            Some(w) => w,
            None => return,
        };

        if guard.last_word == word {
            return;
        }

        if guard.busy.load(Ordering::Relaxed) {
            return;
        }

        let since_change = now.duration_since(guard.last_change);
        let since_fire = now.duration_since(guard.last_fire);

        // THROTTLE during active typing: allow at most one fire per debounce_ms.
        // DEBOUNCE after pause: once typing goes quiet, fire the final word.
        //
        // "Text is changing" = last_change is recent → throttle.
        // "Text went quiet"  = last_change is stale   → debounce elapsed → fire.
        let text_changing = since_change < Duration::from_millis(cfg.debounce_ms);
        let too_soon = since_fire < Duration::from_millis(cfg.debounce_ms);

        if text_changing && too_soon {
            return;
        }

        guard.last_word = word.clone();
        guard.last_fire = now;
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
                window_title: window_title.clone(),
            },
        );

        let app2 = app.clone();
        let st2 = st.clone();
        let title2 = window_title.clone();
        tauri::async_runtime::spawn(async move {
            let result = run_suggestion(&app2, &word, &full_text, &title2).await;
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

async fn run_suggestion(app: &AppHandle, word: &str, full_text: &str, window_title: &str) -> Result<(), String> {
    let cfg = crate::config::load();
    let count = cfg.suggestion_count.max(1);
    let response = engine::get_suggestions(word, count).await?;

    let payload = SuggestionPayload {
        corrections: response.corrections,
        translation: None,
        word: word.to_string(),
        full_text: full_text.to_string(),
        ai_words: Vec::new(),
        window_title: window_title.to_string(),
    };

    let _ = app.emit("plume:suggestions", payload);
    Ok(())
}

/// Returns `true` when a fullscreen Direct3D app (game) or presentation mode
/// (screen sharing, slideshow) is active on Windows 10+.
///
/// Uses `SHQueryUserNotificationState` — the same API Windows uses for Game
/// Bar and toast suppression.
#[cfg(target_os = "windows")]
fn is_fullscreen_active() -> bool {
    use windows::Win32::UI::Shell::SHQueryUserNotificationState;

    unsafe {
        if let Ok(state) = SHQueryUserNotificationState() {
            // 2 = QUNS_RUNNING_D3D_FULL_SCREEN, 3 = QUNS_PRESENTATION_MODE
            state.0 == 2 || state.0 == 3
        } else {
            false
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn is_fullscreen_active() -> bool {
    false
}
