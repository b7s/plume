use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Serialize, Deserialize)]
pub struct LlmResponse {
    #[serde(default)]
    pub corrections: Vec<String>,
    #[serde(default)]
    pub emoji: String,
    #[serde(default)]
    pub translation: Option<String>,
}

fn extract_content(data: &serde_json::Value) -> Result<String, String> {
    let content = data["choices"][0]["message"]["content"]
        .as_str()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            data["choices"][0]["message"]["reasoning_content"]
                .as_str()
        })
        .ok_or_else(|| "no content in response".to_string())?;

    // Strip <think>...</think> blocks
    let mut s = content.to_string();
    while let Some(start) = s.find("<think>") {
        if let Some(end) = s[start..].find("</think>") {
            s.replace_range(start..start + end + 8, "");
        } else {
            s.replace_range(start.., "");
            break;
        }
    }
    Ok(s.trim().to_string())
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete_raw(&self, system: &str, prompt: &str) -> Result<String, String>;

    async fn translate_text(&self, text: &str, language: &str) -> Result<String, String> {
        let system = format!(
            "You are a professional translator. \
             Your ONLY task: translate the user's text into {language}. \
             Rules:\n\
             - Output ONLY the {language} translation.\n\
             - Do NOT include thinking, reasoning, or explanations.\n\
             - Do NOT repeat the original text.\n\
             - Do NOT add quotes or markdown.\n\
             - Preserve the meaning and tone of the original text."
        );
        let prompt = format!("Translate this text to {language}:\n\n{text}");
        self.complete_raw(&system, &prompt).await
    }

    async fn process_text(&self, text: &str, action: &str) -> Result<String, String> {
        let (system, prompt) = match action {
            "summarize" => (
                "You are a text summarizer. Summarize the user's text concisely. \
                 Output ONLY the summary — no thinking, no explanation.".to_string(),
                format!("Summarize:\n\n{text}"),
            ),
            "shorter" => (
                "You are a text editor. Make the user's text shorter while preserving meaning. \
                 Output ONLY the revised text — no thinking, no explanation.".to_string(),
                format!("Make this shorter:\n\n{text}"),
            ),
            "longer" => (
                "You are a text editor. Expand the user's text with relevant detail. \
                 Output ONLY the revised text — no thinking, no explanation.".to_string(),
                format!("Make this longer:\n\n{text}"),
            ),
            tone if tone.starts_with("tone:") => {
                let tone_name = &tone[5..];
                (
                    format!(
                        "You are a text editor. Rewrite the user's text in a {tone_name} tone. \
                         Output ONLY the rewritten text — no thinking, no explanation."
                    ),
                    format!("Rewrite in a {tone_name} tone:\n\n{text}"),
                )
            }
            fmt if fmt.starts_with("format:") => {
                let fmt_name = &fmt[7..];
                (
                    format!(
                        "You are a text formatter. Reformat the user's text as {fmt_name}. \
                         Output ONLY the formatted text — no thinking, no explanation."
                    ),
                    format!("Format as {fmt_name}:\n\n{text}"),
                )
            }
            _ => return Err(format!("unknown action: {action}")),
        };
        self.complete_raw(&system, &prompt).await
    }
}

pub struct OllamaProvider {
    endpoint: String,
    model: String,
    client: reqwest::Client,
}

impl OllamaProvider {
    pub fn new(endpoint: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            model: model.into(),
            client: reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .unwrap_or_default(),
        }
    }
}

#[async_trait]
impl LlmProvider for OllamaProvider {
    async fn complete_raw(&self, system: &str, prompt: &str) -> Result<String, String> {
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": prompt}
            ],
            "stream": false,
            "max_tokens": 1024,
            "temperature": 0.1,
            "reasoning_format": "none",
        });

        let resp = self
            .client
            .post(format!("{}/v1/chat/completions", self.endpoint))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;

        let data: serde_json::Value =
            resp.json().await.map_err(|e| format!("parse failed: {e}"))?;

        extract_content(&data)
    }
}

pub struct OpenAiProvider {
    endpoint: String,
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl OpenAiProvider {
    pub fn new(
        endpoint: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            endpoint: endpoint.into(),
            api_key: api_key.into(),
            model: model.into(),
            client: reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .unwrap_or_default(),
        }
    }
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
    async fn complete_raw(&self, system: &str, prompt: &str) -> Result<String, String> {
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": prompt}
            ],
            "max_tokens": 1024,
            "temperature": 0.1,
        });

        let resp = self
            .client
            .post(self.endpoint.as_str())
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;

        let data: serde_json::Value =
            resp.json().await.map_err(|e| format!("parse failed: {e}"))?;

        extract_content(&data)
    }
}

pub struct LocalProvider {
    endpoint: String,
    model: String,
    client: reqwest::Client,
}

impl LocalProvider {
    pub fn new(endpoint: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            model: model.into(),
            client: reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .unwrap_or_default(),
        }
    }
}

#[async_trait]
impl LlmProvider for LocalProvider {
    async fn complete_raw(&self, system: &str, prompt: &str) -> Result<String, String> {
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": prompt}
            ],
            "stream": false,
            "max_tokens": 1024,
            "temperature": 0.1,
            "reasoning_format": "none",
            "chat_template_kwargs": {
                "enable_thinking": false
            },
        });

        let resp = self
            .client
            .post(format!("{}/v1/chat/completions", self.endpoint))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;

        let data: serde_json::Value =
            resp.json().await.map_err(|e| format!("parse failed: {e}"))?;

        extract_content(&data)
    }
}

/// Build a provider from saved settings. Returns Err if API key is missing for API mode.
pub fn build_provider(settings: &crate::state::Settings) -> Result<Box<dyn LlmProvider>, String> {
    match settings.provider.as_str() {
        "local" => Ok(Box::new(LocalProvider::new(
            &settings.endpoint,
            &settings.model,
        ))),
        "ollama" => Ok(Box::new(OllamaProvider::new(
            &settings.endpoint,
            &settings.model,
        ))),
        "openai" => {
            if settings.api_key.is_empty() {
                Err("API key not set for OpenAI provider".into())
            } else {
                let endpoint = if settings.endpoint.is_empty() {
                    "https://api.openai.com/v1/chat/completions".to_string()
                } else {
                    settings.endpoint.clone()
                };
                Ok(Box::new(OpenAiProvider::new(
                    endpoint,
                    &settings.api_key,
                    &settings.model,
                )))
            }
        }
        other => Err(format!("Unknown provider: {other}")),
    }
}
