use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DictionaryConfig {
    #[serde(default = "default_dict_language")]
    pub language: String,
    #[serde(default)]
    pub url: String,
}

fn default_dict_language() -> String {
    "en_US".into()
}

impl Default for DictionaryConfig {
    fn default() -> Self {
        Self {
            language: "en_US".into(),
            url: String::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TranslationConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_translation_language")]
    pub language: String,
}

fn default_translation_language() -> String {
    "portuguese".into()
}

impl Default for TranslationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            language: "portuguese".into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WindowConfig {
    #[serde(default = "default_window_x")]
    pub x: f64,
    #[serde(default = "default_window_y")]
    pub y: f64,
    #[serde(default = "default_window_width")]
    pub width: f64,
    #[serde(default = "default_window_height")]
    pub height: f64,
    #[serde(default = "default_window_min_width")]
    pub min_width: f64,
    #[serde(default = "default_window_min_height")]
    pub min_height: f64,
    #[serde(default = "default_window_max_width")]
    pub max_width: f64,
    #[serde(default = "default_window_max_height")]
    pub max_height: f64,
}

fn default_window_x() -> f64 { -1.0 }
fn default_window_y() -> f64 { -1.0 }
fn default_window_width() -> f64 { 400.0 }
fn default_window_height() -> f64 { 250.0 }
fn default_window_min_width() -> f64 { 400.0 }
fn default_window_min_height() -> f64 { 250.0 }
fn default_window_max_width() -> f64 { 800.0 }
fn default_window_max_height() -> f64 { 800.0 }

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            x: -1.0,
            y: -1.0,
            width: 400.0,
            height: 250.0,
            min_width: 400.0,
            min_height: 250.0,
            max_width: 800.0,
            max_height: 800.0,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub model_url: String,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub headers: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub dictionary: DictionaryConfig,
    #[serde(default)]
    pub translation: TranslationConfig,
    #[serde(default)]
    pub window: WindowConfig,
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout: u64,
    #[serde(default = "default_suggestion_count")]
    pub suggestion_count: usize,
    #[serde(default = "default_ai_suggestion_count")]
    pub ai_suggestion_count: usize,
    #[serde(default = "default_ai_suggestion_delay")]
    pub ai_suggestion_delay: u64,
    // Capture
    #[serde(default = "default_poll_interval_ms")]
    pub poll_interval_ms: u64,
    #[serde(default = "default_read_throttle_ms")]
    pub read_throttle_ms: u64,
    #[serde(default = "default_debounce_ms")]
    pub debounce_ms: u64,
    #[serde(default = "default_min_word_len")]
    pub min_word_len: usize,
    #[serde(default = "default_max_added_chars")]
    pub max_added_chars: usize,
    // AI
    #[serde(default = "default_request_timeout_secs")]
    pub request_timeout_secs: u64,
    // UI
    #[serde(default = "default_resize_debounce_ms")]
    pub resize_debounce_ms: u64,
    #[serde(default = "default_hover_timeout_secs")]
    pub hover_timeout_secs: u64,
    #[serde(default = "default_hide_during_fullscreen")]
    pub hide_during_fullscreen: bool,
    #[serde(default = "default_auto_hide")]
    pub auto_hide: bool,
    #[serde(default = "default_window_opacity")]
    pub window_opacity: u8,
    #[serde(default = "default_compute_backend")]
    pub compute_backend: String,
}

fn default_port() -> u16 { 8080 }

fn default_idle_timeout() -> u64 { 6 }

fn default_suggestion_count() -> usize { 6 }

fn default_ai_suggestion_count() -> usize { 3 }

fn default_ai_suggestion_delay() -> u64 { 800 }

fn default_poll_interval_ms() -> u64 { 100 }

fn default_read_throttle_ms() -> u64 { 150 }

fn default_debounce_ms() -> u64 { 300 }

fn default_min_word_len() -> usize { 1 }

fn default_max_added_chars() -> usize { 5 }

fn default_request_timeout_secs() -> u64 { 600 }

fn default_resize_debounce_ms() -> u64 { 250 }

fn default_hover_timeout_secs() -> u64 { 30 }

fn default_hide_during_fullscreen() -> bool { false }

fn default_auto_hide() -> bool { true }

fn default_window_opacity() -> u8 { 100 }

fn default_compute_backend() -> String { "cpu".into() }

impl Default for Config {
    fn default() -> Self {
        Self {
            provider: "local".into(),
            model: "Qwen3-0.6B-Q8_0.gguf".into(),
            model_url: String::new(),
            endpoint: "http://127.0.0.1:8080".into(),
            api_key: String::new(),
            headers: String::new(),
            port: 8080,
            dictionary: DictionaryConfig::default(),
            translation: TranslationConfig::default(),
            window: WindowConfig::default(),
            idle_timeout: 6,
            suggestion_count: 6,
            ai_suggestion_count: 3,
            ai_suggestion_delay: 800,
            poll_interval_ms: 100,
            read_throttle_ms: 150,
            debounce_ms: 300,
            min_word_len: 1,
            max_added_chars: 5,
            request_timeout_secs: 600,
            resize_debounce_ms: 250,
            hover_timeout_secs: 30,
            hide_during_fullscreen: false,
            auto_hide: true,
            window_opacity: 100,
            compute_backend: "cpu".into(),
        }
    }
}

fn config_path() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p = dir.join("config.json");
            if p.exists() {
                return p;
            }
        }
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        PathBuf::from(appdata).join("plume").join("config.json")
    } else {
        PathBuf::from("config.json")
    }
}

pub fn load() -> Config {
    let path = config_path();
    if path.exists() {
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or_default()
    } else {
        let cfg = Config::default();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&path, serde_json::to_string_pretty(&cfg).unwrap_or_default());
        cfg
    }
}

pub fn save_window(x: f64, y: f64, width: f64, height: f64) {
    let cfg = Config::default();
    // Clamp to sane bounds — Tauri can report wrapped u32 values (near u32::MAX)
    // when the window is minimized or in a transitional state, which overflows tao.
    let x = x.clamp(-10_000.0, 10_000.0);
    let y = y.clamp(-10_000.0, 10_000.0);
    let width = width.clamp(cfg.window.min_width, cfg.window.max_width);
    let height = height.clamp(cfg.window.min_height, cfg.window.max_height);

    let path = config_path();

    // Read existing file as raw JSON value to preserve unknown fields
    let mut root: serde_json::Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_else(|| serde_json::to_value(Config::default()).unwrap_or_default());

    root["window"] = serde_json::json!({
        "x": x,
        "y": y,
        "width": width,
        "height": height,
    });

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(content) = serde_json::to_string_pretty(&root) {
        let _ = std::fs::write(&path, content);
    }
}

pub fn set_dictionary_language(language: &str) {
    let path = config_path();

    let mut root: serde_json::Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_else(|| serde_json::to_value(Config::default()).unwrap_or_default());

    if root.get("dictionary").is_none() {
        root["dictionary"] = serde_json::json!({});
    }
    root["dictionary"]["language"] = serde_json::json!(language);
    root["dictionary"]["url"] = serde_json::json!("");

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(content) = serde_json::to_string_pretty(&root) {
        let _ = std::fs::write(&path, content);
    }
}

/// Save the full config, merging with existing file to preserve unknown fields.
pub fn save_config(new_cfg: &Config) {
    let path = config_path();

    let mut root: serde_json::Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_else(|| serde_json::to_value(Config::default()).unwrap_or_default());

    let new_val = serde_json::to_value(new_cfg).unwrap_or_default();
    if let (Some(old_obj), Some(new_obj)) = (root.as_object_mut(), new_val.as_object()) {
        for (key, val) in new_obj {
            old_obj.insert(key.clone(), val.clone());
        }
    }

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(content) = serde_json::to_string_pretty(&root) {
        let _ = std::fs::write(&path, content);
    }
}
