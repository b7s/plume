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
}

fn default_window_x() -> f64 { -1.0 }
fn default_window_y() -> f64 { -1.0 }
fn default_window_width() -> f64 { 400.0 }
fn default_window_height() -> f64 { 150.0 }

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            x: -1.0,
            y: -1.0,
            width: 400.0,
            height: 150.0,
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
}

fn default_port() -> u16 {
    8080
}

fn default_idle_timeout() -> u64 {
    4
}

fn default_suggestion_count() -> usize {
    6
}

impl Default for Config {
    fn default() -> Self {
        Self {
            provider: "local".into(),
            model: "Qwen3-0.6B-Q4_K_M.gguf".into(),
            model_url: String::new(),
            endpoint: "http://127.0.0.1:8080".into(),
            api_key: String::new(),
            port: 8080,
            dictionary: DictionaryConfig::default(),
            translation: TranslationConfig::default(),
            window: WindowConfig::default(),
            idle_timeout: 4,
            suggestion_count: 6,
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
