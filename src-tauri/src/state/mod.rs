use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

/// Application settings — merged from config.json (provider/model/endpoint/api_key)
/// and tauri-plugin-store (material/position/auto_show/target_language).
#[derive(Debug, Serialize, Deserialize)]
pub struct Settings {
    pub material: String,
    pub position: String,
    pub provider: String,
    pub model: String,
    pub endpoint: String,
    pub api_key: String,
    pub target_language: String,
    pub auto_show: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            material: "mica".into(),
            position: "bottom-center".into(),
            provider: "local".into(),
            model: "Qwen3.5-2B-Q4_K_M.gguf".into(),
            endpoint: "http://127.0.0.1:8080".into(),
            api_key: String::new(),
            target_language: String::new(),
            auto_show: true,
        }
    }
}

pub fn load(app: &AppHandle) -> Settings {
    let mut settings = Settings::default();

    if let Ok(store) = app.store("plume-settings.json") {
        if let Some(val) = store.get("settings") {
            if let Ok(s) = serde_json::from_value(val) {
                settings = s;
            }
        }
    }

    let config = crate::config::load();
    if !config.provider.is_empty() {
        settings.provider = config.provider;
    }
    if !config.model.is_empty() {
        settings.model = config.model;
    }
    if !config.endpoint.is_empty() {
        settings.endpoint = config.endpoint;
    }
    if !config.api_key.is_empty() {
        settings.api_key = config.api_key;
    }
    if config.translation.enabled {
        settings.target_language = config.translation.language;
    } else {
        settings.target_language = String::new();
    }

    settings
}
