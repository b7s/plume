mod ai;
mod capture;
mod config;
mod engine;
mod llama;
mod spellcheck;
mod state;

use serde::Serialize;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, RunEvent, WindowEvent,
};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_clipboard_manager::ClipboardExt;
use uiautomation::{
    UIAutomation, UIElement,
    patterns::{UITextPattern, UIValuePattern},
    types::Handle as UIAHandle,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_KEYBOARD, INPUT_0, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    KEYEVENTF_UNICODE, VIRTUAL_KEY, VK_BACK, VK_CONTROL, VK_END,
};

#[derive(Default, Serialize, Clone)]
pub struct SuggestionPayload {
    pub corrections: Vec<String>,
    pub translation: Option<String>,
    pub word: String,
    pub full_text: String,
    #[serde(default)]
    pub ai_words: Vec<String>,
    #[serde(default)]
    pub window_title: String,
}

#[derive(Default, Serialize, Clone)]
pub struct PlaceholderPayload {
    pub corrections: Vec<String>,
    pub word: String,
    pub full_text: String,
    #[serde(default)]
    pub window_title: String,
}

#[derive(Default, Serialize, Clone)]
pub struct AiSuggestionPayload {
    pub words: Vec<String>,
}

fn replace_last_word(text: &str, replacement: &str) -> String {
    if let Some(last_space) = text.rfind(|c: char| c.is_whitespace()) {
        let mut result = text[..=last_space].to_string();
        result.push_str(replacement);
        result
    } else {
        replacement.to_string()
    }
}

fn type_via_sendinput(text: &str, backspace: usize) -> bool {
    let mut inputs = Vec::with_capacity(backspace * 2 + text.chars().count() * 2);

    for _ in 0..backspace {
        let ki = KEYBDINPUT { wVk: VIRTUAL_KEY(VK_BACK.0), ..Default::default() };
        inputs.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 { ki },
        });
        let ki = KEYBDINPUT { wVk: VIRTUAL_KEY(VK_BACK.0), dwFlags: KEYBD_EVENT_FLAGS(KEYEVENTF_KEYUP.0), ..Default::default() };
        inputs.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 { ki },
        });
    }

    for ch in text.chars() {
        let ki = KEYBDINPUT { wScan: ch as u16, dwFlags: KEYBD_EVENT_FLAGS(KEYEVENTF_UNICODE.0), ..Default::default() };
        inputs.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 { ki },
        });
        let ki = KEYBDINPUT { wScan: ch as u16, dwFlags: KEYBD_EVENT_FLAGS(KEYEVENTF_UNICODE.0 | KEYEVENTF_KEYUP.0), ..Default::default() };
        inputs.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 { ki },
        });
    }

    if inputs.is_empty() {
        return false;
    }

    unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>().try_into().unwrap()) > 0 }
}

/// Sends Ctrl+End to move the caret to the end of the document.
/// Used after ValuePattern.SetValue which resets caret to position 0.
fn send_ctrl_end() -> bool {
    let make = |vk, flags| {
        let ki = KEYBDINPUT { wVk: VIRTUAL_KEY(vk), dwFlags: KEYBD_EVENT_FLAGS(flags), ..Default::default() };
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 { ki },
        }
    };

    let inputs = [
        make(VK_CONTROL.0, 0),
        make(VK_END.0, 0),
        make(VK_END.0, KEYEVENTF_KEYUP.0),
        make(VK_CONTROL.0, KEYEVENTF_KEYUP.0),
    ];

    unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>().try_into().unwrap()) > 0 }
}

/// Last HWND of the user's text control (stored as isize because
/// uiautomation::Handle wraps *mut c_void and is !Send).
static LAST_FOCUSED_HWND: Mutex<Option<isize>> = Mutex::new(None);

fn get_target_element(automation: &UIAutomation) -> Option<UIElement> {
    // Prefer the last-known-HWND from the capture thread.
    // When the user clicks a Plume chip, get_focused_element() may return
    // Plume's WebView2 (even with WS_EX_NOACTIVATE), but the capture loop
    // ran milliseconds earlier with the correct focus.
    if let Ok(guard) = LAST_FOCUSED_HWND.lock() {
        if let Some(hwnd) = *guard {
            if let Ok(el) = automation.element_from_handle(UIAHandle::from(hwnd)) {
                return Some(el);
            }
        }
    }
    automation.get_focused_element().ok()
}

fn insert_via_uia(replacement: &str) -> bool {
    // CoInitializeEx is not called here because Tauri/WebView2 already
    // initializes COM on the main thread (as STA).  The uiautomation
    // crate's UIAutomation::new() calls CoInitializeEx(MTA) which
    // conflicts → RPC_E_CHANGED_MODE.  Use new_direct() instead, which
    // skips COM init and just creates the COM object.
    let automation = match UIAutomation::new_direct() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("[plume] UIA init failed: {e}");
            return false;
        }
    };

    let Some(focused) = get_target_element(&automation) else {
        eprintln!("[plume] no target element");
        return false;
    };

    // Try ValuePattern (replaces last word at position)
    if let Ok(vp) = focused.get_pattern::<UIValuePattern>() {
        if let Ok(current) = vp.get_value() {
            let new_text = replace_last_word(&current, replacement);
            if vp.set_value(&new_text).is_ok() {
                eprintln!("[plume] ValuePattern success");
                send_ctrl_end();
                return true;
            }
        }
    }

    // Try TextPattern + SendInput (types into the focused control)
    if let Ok(tp) = focused.get_pattern::<UITextPattern>() {
        if let Ok(doc_range) = tp.get_document_range() {
            if let Ok(text) = doc_range.get_text(-1) {
                let word_len = text
                    .split_whitespace()
                    .last()
                    .map(|w| w.len())
                    .unwrap_or(0);
                eprintln!("[plume] TextPattern SendInput (backspace={})", word_len);
                return type_via_sendinput(replacement, word_len);
            }
        }
    }

    eprintln!("[plume] falling back to SendInput");
    type_via_sendinput(replacement, 0)
}

#[tauri::command]
fn accept_text(text: String) -> bool {
    insert_via_uia(&format!("{text} "))
}

#[tauri::command]
fn copy_text(app: AppHandle, text: String) {
    let _ = app.clipboard().write_text(text);
}

#[tauri::command]
fn toggle_plume(app: AppHandle) {
    if let Some(window) = app.get_webview_window("plume") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}

/// Toggle whether the overlay can take keyboard focus.
///
/// The overlay is normally `WS_EX_NOACTIVATE` so that popping it up while you
/// type in another app never steals focus. The trade-off is that a
/// no-activate window can't receive keystrokes, which makes the in-overlay
/// textarea untypeable. When the user clicks that textarea we call this with
/// `activatable = true` to drop the style and bring the window to the
/// foreground so typing works; we restore the style when focus leaves it.
#[tauri::command]
fn set_window_activatable(app: AppHandle, activatable: bool) {
    #[cfg(target_os = "windows")]
    {
        if let Some(window) = app.get_webview_window("plume") {
            if let Ok(hwnd) = window.hwnd() {
                use windows::Win32::UI::WindowsAndMessaging::{
                    GetWindowLongW, SetForegroundWindow, SetWindowLongW, SetWindowPos,
                    GWL_EXSTYLE, SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
                    WS_EX_NOACTIVATE,
                };
                unsafe {
                    let ex = GetWindowLongW(hwnd, GWL_EXSTYLE);
                    let new_ex = if activatable {
                        ex & !(WS_EX_NOACTIVATE.0 as i32)
                    } else {
                        ex | (WS_EX_NOACTIVATE.0 as i32)
                    };
                    SetWindowLongW(hwnd, GWL_EXSTYLE, new_ex);
                    // Flush the new extended style without moving/resizing.
                    let _ = SetWindowPos(
                        hwnd,
                        None,
                        0,
                        0,
                        0,
                        0,
                        SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
                    );
                    if activatable {
                        let _ = SetForegroundWindow(hwnd);
                    }
                }
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (app, activatable);
    }
}

/// Set the window opacity (0–100).  WebView2 does not support
/// SetLayeredWindowAttributes, so the actual opacity is applied via CSS
/// on the front-end. This command exists so the front-end can notify
/// the backend of the current value, but it is currently a no-op.
#[tauri::command]
fn set_window_opacity(_app: AppHandle, _opacity: u8) {}

#[tauri::command]
fn on_overlay_ready(app: AppHandle) -> (u64, bool, String) {
    let cfg = config::load();
    let idle_timeout = cfg.idle_timeout;
    let tr_enabled = cfg.translation.enabled;
    let tr_lang = cfg.translation.language.clone();

    if let Some(window) = app.get_webview_window("plume") {
        let wpos = &cfg.window;
        let w = wpos.width.clamp(wpos.min_width, wpos.max_width);
        let h = wpos.min_height;

        if wpos.x >= 0.0 && wpos.y >= 0.0 && wpos.x < 10_000.0 && wpos.y < 10_000.0 {
            let _ = window.set_position(LogicalPosition::new(wpos.x, wpos.y));
        } else {
            // Default: bottom-right corner
            if let Ok(Some(monitor)) = app.primary_monitor() {
                let scale = monitor.scale_factor();
                let screen_w = monitor.size().width as f64 / scale;
                let screen_h = monitor.size().height as f64 / scale;
                let x = screen_w - w - 20.0;
                let y = screen_h - h - 60.0;
                let _ = window.set_position(LogicalPosition::new(x, y));
            }
        }
        let _ = window.set_size(LogicalSize::new(w, h));
    }

    (idle_timeout, tr_enabled, tr_lang)
}

#[tauri::command]
fn save_window_position(_app: AppHandle, x: f64, y: f64, width: f64, height: f64) {
    config::save_window(x, y, width, height);
}

#[tauri::command]
fn get_config() -> config::Config {
    config::load()
}

#[tauri::command]
fn list_languages() -> Vec<(String, String)> {
    spellcheck::available_languages()
        .into_iter()
        .map(|(code, name)| (code.to_string(), name.to_string()))
        .collect()
}

#[tauri::command]
fn get_current_language() -> String {
    config::load().dictionary.language
}

#[tauri::command]
async fn set_dictionary_language(language: String) -> Result<bool, String> {
    // Download if not present
    let aff = spellcheck::aff_path(&language);
    let dic = spellcheck::dic_path(&language);
    if !aff.exists() || !dic.exists() {
        spellcheck::download_dictionary(&language, "").await?;
    }
    // Init the new dictionary
    spellcheck::init(&language)?;
    // Save to config
    config::set_dictionary_language(&language);
    eprintln!("[plume] Dictionary language set to {language}");
    Ok(true)
}

#[tauri::command]
async fn translate_text(app: AppHandle, text: String, language: String) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err("empty text".into());
    }
    eprintln!("[plume] translate: language={language} | text={text}");
    let settings = state::load(&app);
    let provider = ai::build_provider(&settings)?;
    let result = provider.translate_text(&text, &language).await?;
    eprintln!("[plume] translate result: {result}");
    Ok(result)
}

#[tauri::command]
async fn process_text(app: AppHandle, text: String, action: String) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err("empty text".into());
    }
    eprintln!("[plume] process: action={action} | text={text}");
    let settings = state::load(&app);
    let provider = ai::build_provider(&settings)?;
    let result = provider.process_text(&text, &action).await?;
    eprintln!("[plume] process result: {result}");
    Ok(result)
}

#[tauri::command]
async fn explain_text(app: AppHandle, text: String) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err("empty text".into());
    }
    eprintln!("[plume] explain: text={text}");
    let settings = state::load(&app);
    let provider = ai::build_provider(&settings)?;
    let result = provider.explain_text(&text).await?;
    eprintln!("[plume] explain result: {result}");
    Ok(result)
}

#[tauri::command]
async fn suggest_next_words(app: AppHandle, text: String) -> Result<Vec<String>, String> {
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let cfg = config::load();
    let count = cfg.ai_suggestion_count.max(1);
    eprintln!("[plume] ai_suggest: count={count} | text={text}");
    let settings = state::load(&app);
    let provider = ai::build_provider(&settings)?;
    // Language is derived from the user's text inside the provider — we
    // deliberately do NOT pass the dictionary/settings language here.
    let words = provider.suggest_next_words(&text, count).await?;
    eprintln!("[plume] ai_suggest result: {words:?}");
    Ok(words)
}

#[tauri::command]
async fn check_model(model_name: String) -> bool {
    let llama = crate::llama::LlamaSetup::new();
    llama.model_exists(&model_name)
}

#[tauri::command]
async fn download_model(app: AppHandle, model_name: String, model_url: String) -> Result<String, String> {
    let llama = crate::llama::LlamaSetup::new();

    if llama.model_exists(&model_name) {
        let path = llama.model_path(&model_name);
        return Ok(path.to_string_lossy().to_string());
    }

    eprintln!("[plume] Downloading model: {model_name}");
    app.emit("plume:model-download-start", &model_name).ok();
    let result = llama.download_model(&model_name, &model_url).await;
    match result {
        Ok(path) => {
            app.emit("plume:model-download-done", &model_name).ok();
            Ok(path.to_string_lossy().to_string())
        }
        Err(e) => {
            app.emit("plume:model-download-error", &e).ok();
            Err(e)
        }
    }
}

#[tauri::command]
fn save_config(app: AppHandle, config: config::Config) -> bool {
    config::save_config(&config);
    app.emit("plume:config-updated", config.window_opacity).ok();
    if let Some(main_win) = app.get_webview_window("main") {
        let js = format!("windowOpacity = {}; applyOpacity();", config.window_opacity);
        let _ = main_win.eval(&js);
    }
    true
}

/// Restart the local llama-server so it picks up a newly-saved model/port.
/// Kills the current server (and any orphaned llama-server.exe holding the
/// port), then loads the model from the current config. No-op for non-local
/// providers.
#[tauri::command]
async fn restart_llama(app: AppHandle) -> Result<String, String> {
    let cfg = config::load();
    if cfg.provider != "local" {
        return Ok("skipped: not local provider".into());
    }
    let model_name = cfg.model.clone();
    let model_url = cfg.model_url.clone();
    let port = cfg.port;

    eprintln!("[plume] Restarting llama-server with {model_name} on port {port}");
    llama::kill_all();
    let _ = app.emit("plume:model-reload", ());

    let (server_path, model_path) =
        llama::LlamaSetup::ensure_ready(&model_name, &model_url, port, true, true)
            .await
            .map_err(|e| format!("model setup failed: {e}"))?;

    let child = llama::LlamaSetup::start_server(port, &model_path, &server_path)
        .map_err(|e| format!("failed to start server: {e}"))?;
    if let Ok(mut proc) = llama::LLAMA_PROCESS.lock() {
        *proc = Some(child);
    }

    llama::LlamaSetup::wait_ready(port, Duration::from_secs(180))
        .await
        .map_err(|e| format!("server didn't become ready: {e}"))?;
    eprintln!("[plume] llama-server ready with {model_name}");
    Ok(model_name)
}

#[tauri::command]
fn open_settings(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("settings") {
        window.show().map_err(|e| format!("show failed: {e}"))?;
        window.set_focus().map_err(|e| format!("focus failed: {e}"))?;
        let _ = window.eval("modalSave.disabled = false; modalSave.textContent = 'Save'; loadConfig();");
        Ok(())
    } else {
        Err("settings window not found".into())
    }
}

#[tauri::command]
fn close_settings(app: AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.hide();
    }
}

fn cleanup_llama() {
    llama::kill_all();
}

pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let _ = app.get_webview_window("plume").map(|w| w.set_focus());
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![]),
        ))
        .invoke_handler(tauri::generate_handler![
            accept_text,
            copy_text,
            toggle_plume,
            set_window_activatable,
            set_window_opacity,
            on_overlay_ready,
            save_window_position,
            get_config,
            save_config,
            open_settings,
            close_settings,
            list_languages,
            get_current_language,
            set_dictionary_language,
            translate_text,
            process_text,
            explain_text,
            suggest_next_words,
            check_model,
            download_model,
            restart_llama,
        ])
        .setup(|app| {
            #[cfg(target_os = "windows")]
            force_window_corners_round(app);

            // WS_EX_NOACTIVATE — clicking Plume never steals focus from the user's app.
            #[cfg(target_os = "windows")]
            set_no_activate(app);

            let quit =
                tauri::menu::MenuItem::with_id(app, "quit", "Quit Plume", true, None::<&str>)?;
            let settings_item =
                tauri::menu::MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
            let toggle =
                tauri::menu::MenuItem::with_id(app, "toggle", "Show/Hide", true, None::<&str>)?;
            let menu = tauri::menu::Menu::with_items(app, &[&toggle, &settings_item, &quit])?;

            tauri::tray::TrayIconBuilder::with_id("plume-tray")
                .icon(load_tray_icon(is_dark_mode()))
                .tooltip("Plume")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => {
                        cleanup_llama();
                        app.exit(0);
                    }
                    "toggle" => {
                        if let Some(window) = app.get_webview_window("plume") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                    "settings" => {
                        if let Some(window) = app.get_webview_window("settings") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click { button, button_state, .. } = &event {
                        if button == &tauri::tray::MouseButton::Left
                            && button_state == &tauri::tray::MouseButtonState::Up
                        {
                            let app = tray.app_handle();
                            if let Some(window) = app.get_webview_window("plume") {
                                if window.is_visible().unwrap_or(false) {
                                    let _ = window.hide();
                                } else {
                                    let _ = window.show();
                                    let _ = window.set_focus();
                                }
                            }
                        }
                    }
                })
                .build(app)?;

            // Watch for OS theme changes and update tray icon
            #[cfg(target_os = "windows")]
            watch_theme_changes(app.handle().clone());

            let cfg = config::load();

            // Initialize spellcheck dictionary
            {
                let lang = cfg.dictionary.language.clone();
                tauri::async_runtime::spawn(async move {
                    let aff = spellcheck::aff_path(&lang);
                    let dic = spellcheck::dic_path(&lang);
                    if !aff.exists() || !dic.exists() {
                        if let Err(e) = spellcheck::download_dictionary(&lang, "").await {
                            eprintln!("[plume] Dictionary download failed: {e}");
                            return;
                        }
                    }
                    if let Err(e) = spellcheck::init(&lang) {
                        eprintln!("[plume] Spellcheck init failed: {e}");
                    }
                });
            }

            // Start the local LLM whenever the user is on the "local" provider.
            // Both AI next-word suggestions and translation depend on it, so we
            // must not gate this on translation being enabled — otherwise AI
            // suggestions silently fail (connection refused) by default.
            // Hunspell spell-checking is independent and stays instant.
            if cfg.provider == "local" {
                let model_name = cfg.model.clone();
                let model_url = cfg.model_url.clone();
                let port = cfg.port;

                tauri::async_runtime::spawn(async move {
                    match llama::LlamaSetup::ensure_ready(&model_name, &model_url, port, true, true).await {
                        Ok((server_path, model_path)) => {
                            eprintln!("[plume] Starting llama-server...");
                            match llama::LlamaSetup::start_server(port, &model_path, &server_path)
                            {
                                Ok(child) => {
                                    if let Ok(mut proc) = llama::LLAMA_PROCESS.lock() {
                                        *proc = Some(child);
                                    }
                                    eprintln!("[plume] Waiting for server health...");
                                    match llama::LlamaSetup::wait_ready(port, Duration::from_secs(120)).await {
                                        Ok(_) => eprintln!("[plume] LLM ready on port {port}"),
                                        Err(e) => eprintln!("[plume] Health check failed: {e}"),
                                    }
                                }
                                Err(e) => eprintln!("[plume] Failed to start server: {e}"),
                            }
                        }
                        Err(e) => eprintln!("[plume] LLM setup failed: {e}"),
                    }
                });
            }

            let handle = app.handle().clone();
            std::thread::spawn(move || {
                capture::start(handle);
            });

            // Enable auto-start with Windows on first launch.
            let _ = app.autolaunch().enable();

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        });

    let app = builder
        .build(tauri::generate_context!())
        .expect("error while building Plume");

    app.run(|_app_handle, event| {
        if let RunEvent::Exit = event {
            cleanup_llama();
        }
    });
}

#[cfg(target_os = "windows")]
fn set_no_activate(app: &tauri::App) {
    if let Some(window) = app.get_webview_window("plume") {
        if let Ok(hwnd) = window.hwnd() {
            use windows::Win32::UI::WindowsAndMessaging::{
                GetWindowLongW, SetWindowLongW, SetWindowPos, SWP_FRAMECHANGED,
                GWL_EXSTYLE, WS_EX_NOACTIVATE, WS_EX_TOPMOST,
            };
            unsafe {
                let ex = GetWindowLongW(hwnd, GWL_EXSTYLE);
                SetWindowLongW(
                    hwnd,
                    GWL_EXSTYLE,
                    ex | (WS_EX_NOACTIVATE.0 | WS_EX_TOPMOST.0) as i32,
                );
                let _ = SetWindowPos(hwnd, None, 0, 0, 0, 0, SWP_FRAMECHANGED);
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn force_window_corners_round(app: &tauri::App) {
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWM_WINDOW_CORNER_PREFERENCE,
    };

    if let Some(window) = app.get_webview_window("plume") {
        if let Ok(hwnd) = window.hwnd() {
            let pref = DWM_WINDOW_CORNER_PREFERENCE(2);
            unsafe {
                let _ = DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_WINDOW_CORNER_PREFERENCE,
                    &pref as *const _ as *const std::ffi::c_void,
                    std::mem::size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
                );
            }
        }
    }
}

// ========== Theme-aware tray icon ==========

const TRAY_LIGHT_ICON: &[u8] = include_bytes!("../icons/tray-light.png");
const TRAY_DARK_ICON: &[u8] = include_bytes!("../icons/tray-dark.png");

#[cfg(target_os = "windows")]
fn is_dark_mode() -> bool {
    use windows::core::w;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ,
        REG_VALUE_TYPE,
    };

    unsafe {
        let mut hkey = HKEY::default();
        let path = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
        if RegOpenKeyExW(HKEY_CURRENT_USER, path, None, KEY_READ, &mut hkey).is_err() {
            return false;
        }
        let name = w!("AppsUseLightTheme");
        let mut buf: [u8; 4] = [0; 4];
        let mut len: u32 = 4;
        let mut ty = REG_VALUE_TYPE(0);
        let result = RegQueryValueExW(hkey, name, None, Some(&mut ty), Some(buf.as_mut_ptr()), Some(&mut len));
        let _ = RegCloseKey(hkey);
        if result.is_err() {
            return false;
        }
        // AppsUseLightTheme: 0 = dark mode, 1 = light mode
        buf[0] == 0
    }
}

fn load_tray_icon(dark: bool) -> tauri::image::Image<'static> {
    let bytes = if dark { TRAY_DARK_ICON } else { TRAY_LIGHT_ICON };
    tauri::image::Image::from_bytes(bytes)
        .unwrap_or_else(|_| tauri::image::Image::from_bytes(TRAY_LIGHT_ICON).unwrap())
}

#[cfg(target_os = "windows")]
fn watch_theme_changes(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last_dark = is_dark_mode();
        loop {
            std::thread::sleep(Duration::from_secs(2));
            let dark = is_dark_mode();
            if dark != last_dark {
                last_dark = dark;
                let icon = load_tray_icon(dark);
                if let Some(tray) = app.tray_by_id("plume-tray") {
                    let _ = tray.set_icon(Some(icon));
                }
            }
        }
    });
}

#[cfg(not(target_os = "windows"))]
fn watch_theme_changes(_app: AppHandle) {}

#[cfg(not(target_os = "windows"))]
fn is_dark_mode() -> bool { false }
