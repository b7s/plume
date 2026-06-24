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
    AppHandle, LogicalPosition, LogicalSize, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder,
    WindowEvent,
};
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
}

#[derive(Default, Serialize, Clone)]
pub struct PlaceholderPayload {
    pub corrections: Vec<String>,
    pub word: String,
    pub full_text: String,
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
        let mut ki = KEYBDINPUT::default();
        ki.wVk = VIRTUAL_KEY(VK_BACK.0);
        inputs.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 { ki },
        });
        let mut ki = KEYBDINPUT::default();
        ki.wVk = VIRTUAL_KEY(VK_BACK.0);
        ki.dwFlags = KEYBD_EVENT_FLAGS(KEYEVENTF_KEYUP.0);
        inputs.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 { ki },
        });
    }

    for ch in text.chars() {
        let mut ki = KEYBDINPUT::default();
        ki.wScan = ch as u16;
        ki.dwFlags = KEYBD_EVENT_FLAGS(KEYEVENTF_UNICODE.0);
        inputs.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 { ki },
        });
        let mut ki = KEYBDINPUT::default();
        ki.wScan = ch as u16;
        ki.dwFlags = KEYBD_EVENT_FLAGS(KEYEVENTF_UNICODE.0 | KEYEVENTF_KEYUP.0);
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
        let mut ki = KEYBDINPUT::default();
        ki.wVk = VIRTUAL_KEY(vk);
        ki.dwFlags = KEYBD_EVENT_FLAGS(flags);
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
            // Re-apply Mica after show
            use tauri::window::{Effect, EffectsBuilder};
            let _ = window.set_effects(EffectsBuilder::new().effect(Effect::Mica).build());
        }
    }
}

#[tauri::command]
fn set_window_material(app: AppHandle, material: String) {
    use tauri::window::{Effect, EffectsBuilder};
    if let Some(window) = app.get_webview_window("plume") {
        let effect = if material == "acrylic" {
            Effect::Acrylic
        } else {
            Effect::Mica
        };
        let _ = window.set_effects(EffectsBuilder::new().effect(effect).build());
    }
}

#[tauri::command]
fn on_overlay_ready(app: AppHandle) -> (u64, bool, String) {
    let cfg = config::load();
    let idle_timeout = cfg.idle_timeout;
    let tr_enabled = cfg.translation.enabled;
    let tr_lang = cfg.translation.language.clone();

    if let Some(window) = app.get_webview_window("plume") {
        let wpos = &cfg.window;

        if wpos.x >= 0.0 && wpos.y >= 0.0 {
            let _ = window.set_position(LogicalPosition::new(wpos.x, wpos.y));
        } else {
            // Default: bottom-right corner
            if let Ok(Some(monitor)) = app.primary_monitor() {
                let scale = monitor.scale_factor();
                let screen_w = monitor.size().width as f64 / scale;
                let screen_h = monitor.size().height as f64 / scale;
                let x = screen_w - wpos.width - 20.0;
                let y = screen_h - wpos.height - 60.0;
                let _ = window.set_position(LogicalPosition::new(x, y));
            }
        }
        // Always set the exact configured size
        let _ = window.set_size(LogicalSize::new(wpos.width, wpos.height));
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
fn save_config(config: config::Config) -> bool {
    config::save_config(&config);
    true
}

#[tauri::command]
fn open_settings(app: AppHandle) {
    // If window already exists, just show it
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }

    // Create the settings window dynamically
    let builder = WebviewWindowBuilder::new(&app, "settings", WebviewUrl::App("settings.html".into()))
        .title("Plume Settings")
        .inner_size(380.0, 520.0)
        .min_inner_size(320.0, 400.0)
        .resizable(true)
        .minimizable(false)
        .maximizable(false)
        .center()
        .visible(false)
        .decorations(true)
        .shadow(true)
        .always_on_top(false)
        .skip_taskbar(false);

    #[cfg(target_os = "windows")]
    let builder = builder.effects(
        tauri::window::EffectsBuilder::new()
            .effect(tauri::window::Effect::Mica)
            .build(),
    );

    match builder.build() {
        Ok(window) => {
            let _ = window.show();
            let _ = window.set_focus();
        }
        Err(e) => {
            eprintln!("[plume] Failed to create settings window: {e}");
        }
    }
}

#[tauri::command]
fn close_settings(app: AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.close();
    }
}

fn cleanup_llama() {
    if let Ok(mut proc) = llama::LLAMA_PROCESS.lock() {
        if let Some(ref mut child) = *proc {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
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
            set_window_material,
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
        ])
        .setup(|app| {
            #[cfg(target_os = "windows")]
            force_window_corners_round(app);

            // Explicitly set Mica effect
            if let Some(window) = app.get_webview_window("plume") {
                use tauri::window::{Effect, EffectsBuilder};
                let _ = window.set_effects(EffectsBuilder::new().effect(Effect::Mica).build());
            }

            // WS_EX_NOACTIVATE — clicking Plume never steals focus from the user's app.
            #[cfg(target_os = "windows")]
            set_no_activate(app);

            let quit =
                tauri::menu::MenuItem::with_id(app, "quit", "Quit Plume", true, None::<&str>)?;
            let toggle =
                tauri::menu::MenuItem::with_id(app, "toggle", "Show/Hide", true, None::<&str>)?;
            let menu = tauri::menu::Menu::with_items(app, &[&toggle, &quit])?;

            tauri::tray::TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
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
                    _ => {}
                })
                .build(app)?;

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

            // Only start the local LLM when translation is enabled.
            // Hunspell spell-checking is instant and doesn't need the LLM.
            if cfg.provider == "local" && cfg.translation.enabled {
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
            } else if cfg.provider == "local" {
                eprintln!("[plume] Translation disabled — LLM not started");
            }

            let handle = app.handle().clone();
            std::thread::spawn(move || {
                capture::start(handle);
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            let label = window.label().to_string();
            if let WindowEvent::CloseRequested { api, .. } = event {
                // Settings window: allow close (it was created dynamically)
                if label == "settings" {
                    return;
                }
                // Main plume window: hide instead of close
                api.prevent_close();
                let _ = window.hide();
            }
            // Only save geometry for the main plume overlay
            if label == "plume" {
                if let WindowEvent::Moved(position) = event {
                    let Ok(size) = window.inner_size() else { return };
                    config::save_window(
                        position.x as f64,
                        position.y as f64,
                        size.width as f64,
                        size.height as f64,
                    );
                }
                if let WindowEvent::Resized(size) = event {
                    let Ok(position) = window.outer_position() else { return };
                    config::save_window(
                        position.x as f64,
                        position.y as f64,
                        size.width as f64,
                        size.height as f64,
                    );
                }
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
