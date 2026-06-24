use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::config;

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().to_string() + c.as_str()).unwrap_or_default()
}

fn lang_code_to_name(code: whatlang::Lang) -> &'static str {
    match code {
        whatlang::Lang::Eng => "English",
        whatlang::Lang::Por => "Portuguese",
        whatlang::Lang::Spa => "Spanish",
        whatlang::Lang::Fra => "French",
        whatlang::Lang::Deu => "German",
        whatlang::Lang::Ita => "Italian",
        whatlang::Lang::Rus => "Russian",
        whatlang::Lang::Jpn => "Japanese",
        whatlang::Lang::Cmn => "Chinese",
        whatlang::Lang::Kor => "Korean",
        whatlang::Lang::Ara => "Arabic",
        whatlang::Lang::Nld => "Dutch",
        _ => "Text",
    }
}

fn detect_source_language(text: &str) -> &'static str {
    whatlang::detect(text)
        .map(|info| lang_code_to_name(info.lang()))
        .unwrap_or("Text")
}

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
             - Preserve the meaning, tone, and intent of the original text.\n\
             - Use natural, fluent {language} — not a word-for-word translation.\n\
             - Keep names, numbers, and dates as-is."
        );
        let prompt = format!("Translate this text to {language}:\n\n{text}");
        self.complete_raw(&system, &prompt).await
    }

    async fn process_text(&self, text: &str, action: &str) -> Result<String, String> {
        let (system, prompt) = match action {
            "summarize" => (
                "You are an expert summarizer. \
                 Your task: write a concise summary of the user's text.\n\
                 Rules:\n\
                 - Capture the key points and main ideas only.\n\
                 - Keep it significantly shorter than the original.\n\
                 - Output ONLY the summary text.\n\
                 - Do NOT include thinking, reasoning, or explanations.\n\
                 - Do NOT add quotes, labels, or markdown.\n\
                 - Preserve factual accuracy — do not invent information."
                    .to_string(),
                format!("Summarize the key points of this text:\n\n{text}"),
            ),
            "shorter" => (
                "You are an expert text editor. \
                 Your task: make the user's text shorter while keeping all meaning.\n\
                 Rules:\n\
                 - Remove redundancy, wordiness, and unnecessary phrases.\n\
                 - Keep every key point and fact from the original.\n\
                 - Keep the same language as the original text.\n\
                 - Output ONLY the shortened text.\n\
                 - Do NOT include thinking, reasoning, or explanations.\n\
                 - Do NOT add quotes or markdown.\n\
                 - The result must read naturally, not like a list of cuts."
                    .to_string(),
                format!("Make this text shorter without losing any meaning:\n\n{text}"),
            ),
            "longer" => (
                "You are an expert text editor. \
                 Your task: expand the user's text with additional relevant detail.\n\
                 Rules:\n\
                 - Add meaningful context, examples, or elaboration that fits the topic.\n\
                 - Keep all original meaning — do not change the message.\n\
                 - Keep the same language as the original text.\n\
                 - Do NOT repeat the same idea with different words — add NEW information.\n\
                 - Output ONLY the expanded text.\n\
                 - Do NOT include thinking, reasoning, or explanations.\n\
                 - Do NOT add quotes or markdown.\n\
                 - The result must read naturally as one coherent piece."
                    .to_string(),
                format!("Expand this text with more detail, keeping the original meaning:\n\n{text}"),
            ),
            "grammar" => (
                "You are an expert proofreader. \
                 Your task: fix all grammar, spelling, and punctuation errors in the user's text.\n\
                 Rules:\n\
                 - Correct spelling, grammar, punctuation, and capitalization errors.\n\
                 - Keep the same meaning and language — do NOT rewrite or rephrase.\n\
                 - Do NOT change the tone, style, or word choices unless they are wrong.\n\
                 - Output ONLY the corrected text.\n\
                 - Do NOT include thinking, reasoning, or explanations.\n\
                 - Do NOT add quotes or markdown.\n\
                 - Do NOT explain what was changed — just output the clean result."
                    .to_string(),
                format!("Correct the grammar of this text:\n\n{text}"),
            ),
            tone if tone.starts_with("tone:") => {
                let tone_name = &tone[5..];
                (
                    format!(
                        "You are an expert writer. \
                         Your task: rewrite the user's text in a {tone_name} tone.\n\
                         Rules:\n\
                         - Keep the same meaning and all key information.\n\
                         - Change only the tone and word choice to be {tone_name}.\n\
                         - Keep the same language as the original text.\n\
                         - Output ONLY the rewritten text.\n\
                         - Do NOT include thinking, reasoning, or explanations.\n\
                         - Do NOT add quotes or markdown.\n\
                         - The result must read naturally."
                    ),
                    format!("Rewrite this text in a {tone_name} tone:\n\n{text}"),
                )
            }
            fmt if fmt.starts_with("format:") => {
                let fmt_name = &fmt[7..];
                (
                    format!(
                        "You are an expert formatter. \
                         Your task: reformat the user's text as {fmt_name}.\n\
                         Rules:\n\
                         - Keep the same meaning and all key information.\n\
                         - Restructure the text to match {fmt_name} style.\n\
                         - Keep the same language as the original text.\n\
                         - Output ONLY the formatted text.\n\
                         - Do NOT include thinking, reasoning, or explanations.\n\
                         - Do NOT add quotes.\n\
                         - Use markdown formatting only if the style requires it (e.g. lists)."
                    ),
                    format!("Reformat this text as {fmt_name}:\n\n{text}"),
                )
            }
            _ => return Err(format!("unknown action: {action}")),
        };
        self.complete_raw(&system, &prompt).await
    }

    async fn suggest_next_words(&self, text: &str, count: usize, lang: &str) -> Result<Vec<String>, String> {
        let lang_name = match lang {
            "pt_BR" | "pt_PT" => "Portuguese",
            "en_US" | "en_GB" => "English",
            "es_ES" => "Spanish",
            "fr_FR" => "French",
            "de_DE" => "German",
            "it_IT" => "Italian",
            "ja_JP" => "Japanese",
            "zh_CN" => "Chinese",
            "ko_KR" => "Korean",
            "ar_SA" => "Arabic",
            "nl_NL" => "Dutch",
            "ru_RU" => "Russian",
            _ => "the same language as the user's text",
        };

        // Use only the last few words as context — reduces the model's tendency
        // to echo the full text back as suggestions.
        let last_words: Vec<&str> = text.split_whitespace().rev().take(10).collect();
        let context = last_words.iter().rev().copied().collect::<Vec<_>>().join(" ");

        let system = format!(
            "You are a next-word predictor. \
             Your ONLY job: output {count} NEW words in {lang_name} that continue the sentence.\n\
             The user wrote: \"{context}\"\n\
             What comes NEXT? Answer with {count} different options.\n\
             RULES:\n\
             - Output ONLY the new words, one per line.\n\
             - NEVER repeat or copy words from the user's text.\n\
             - NEVER output the user's text back.\n\
             - One option CAN be a short phrase (max 5 words). The rest must be single words.\n\
             - Make each option different (different meaning or part of speech).\n\
             - Do NOT use articles, prepositions, conjunctions, pronouns, or auxiliary verbs.\n\
             - Do NOT use thinking, reasoning, explanations, lists, or markdown.\n\
             - Do NOT start lines with - or numbers.\n\
             Example input: \"The cat jumped on the\"\n\
             Example output (don't use as real output):\n\
             roof\n\
             fence\n\
             chased a mouse"
        );
        let prompt = format!("What {count} words come after: \"{context}\"?");
        let raw = self.complete_raw(&system, &prompt).await?;
        eprintln!("[plume] ai_suggest raw response: {raw:?}");

        // Collect all words from user's text for dedup
        let text_lower = text.to_lowercase();
        let text_words: std::collections::HashSet<&str> = text_lower.split_whitespace().collect();

        let words: Vec<String> = raw
            .lines()
            .flat_map(|l| l.split(','))
            .map(|l| {
                let mut l = l.trim();
                // Repeatedly strip leading list markers, digits, dots, and whitespace
                // Handles patterns like ". ", "1. ", "- ", ". . ", "1. - "
                loop {
                    let stripped = l
                        .trim_start_matches(|c: char| c == '-' || c == '*' || c == '.' || c == ' ');
                    let stripped = stripped.trim_start_matches(|c: char| c.is_ascii_digit());
                    let stripped = stripped.trim_start_matches(|c: char| c == '.' || c == ' ');
                    if stripped.len() == l.len() {
                        break;
                    }
                    l = stripped;
                }
                l.trim().trim_end_matches(|c: char| c.is_ascii_punctuation()).to_string()
            })
            .filter(|l| !l.is_empty())
            .map(|l| {
                // Strip echoed text: if the suggestion starts with any portion
                // of the user's text, remove that prefix and keep only the new part.
                let l_lower = l.to_lowercase();
                let text_lower = text.to_lowercase();
                if text_lower.contains(&l_lower) {
                    // The entire suggestion is a substring of the user's text → skip
                    return String::new();
                }
                l
            })
            .filter(|l| !l.is_empty())
            .filter(|l| {
                let l_lower = l.to_lowercase();
                let first = l_lower.split_whitespace().next().unwrap_or("");
                !text_words.contains(first)
            })
            .take(count)
            .collect();
        Ok(words)
    }
}

pub struct OllamaProvider {
    endpoint: String,
    model: String,
    client: reqwest::Client,
}

impl OllamaProvider {
    pub fn new(endpoint: impl Into<String>, model: impl Into<String>, timeout: Duration) -> Self {
        Self {
            endpoint: endpoint.into(),
            model: model.into(),
            client: reqwest::Client::builder()
                .timeout(timeout)
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
        timeout: Duration,
    ) -> Self {
        Self {
            endpoint: endpoint.into(),
            api_key: api_key.into(),
            model: model.into(),
            client: reqwest::Client::builder()
                .timeout(timeout)
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
    pub fn new(endpoint: impl Into<String>, model: impl Into<String>, timeout: Duration) -> Self {
        Self {
            endpoint: endpoint.into(),
            model: model.into(),
            client: reqwest::Client::builder()
                .timeout(timeout)
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

    async fn translate_text(&self, text: &str, language: &str) -> Result<String, String> {
        let source = detect_source_language(text);
        let target = capitalize(language);
        let prompt = format!("{source}: {text}\n{target}:");
        let body = serde_json::json!({
            "messages": [{"role": "user", "content": prompt}],
            "stream": false,
            "max_tokens": 256,
            "temperature": 0.1,
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

    async fn process_text(&self, text: &str, action: &str) -> Result<String, String> {
        let label: String = match action {
            "summarize" => "Summary".to_string(),
            "shorter" => "Shorter".to_string(),
            "longer" => "Expanded".to_string(),
            "grammar" => "Corrected".to_string(),
            a if a.starts_with("tone:") => capitalize(&a[5..]),
            a if a.starts_with("format:") => capitalize(&a[7..]),
            _ => return Err(format!("unknown action: {action}")),
        };
        let prompt = format!("{text}\n\n{label}:");
        let body = serde_json::json!({
            "messages": [{"role": "user", "content": prompt}],
            "stream": false,
            "max_tokens": 512,
            "temperature": 0.1,
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

/// Build a provider from saved settings. Returns Err if required fields are missing.
pub fn build_provider(settings: &crate::state::Settings) -> Result<Box<dyn LlmProvider>, String> {
    let cfg = config::load();
    let timeout = Duration::from_secs(cfg.request_timeout_secs);
    match settings.provider.as_str() {
        "local" => Ok(Box::new(LocalProvider::new(
            &settings.endpoint,
            &settings.model,
            timeout,
        ))),
        "ollama" => Ok(Box::new(OllamaProvider::new(
            &settings.endpoint,
            &settings.model,
            timeout,
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
                    timeout,
                )))
            }
        }
        "custom" => {
            if settings.endpoint.is_empty() {
                Err("URL is required for custom API provider".into())
            } else {
                Ok(Box::new(CustomProvider::new(
                    &settings.endpoint,
                    &settings.model,
                    &settings.headers,
                    timeout,
                )))
            }
        }
        other => Err(format!("Unknown provider: {other}")),
    }
}

pub struct CustomProvider {
    url: String,
    model: String,
    headers: Vec<(String, String)>,
    client: reqwest::Client,
}

impl CustomProvider {
    pub fn new(url: impl Into<String>, model: impl Into<String>, headers_json: &str, timeout: Duration) -> Self {
        let headers = if headers_json.trim().is_empty() {
            Vec::new()
        } else {
            serde_json::from_str::<serde_json::Value>(headers_json)
                .ok()
                .and_then(|v| v.as_object().map(|obj| {
                    obj.iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect()
                }))
                .unwrap_or_default()
        };

        Self {
            url: url.into(),
            model: model.into(),
            headers,
            client: reqwest::Client::builder()
                .timeout(timeout)
                .build()
                .unwrap_or_default(),
        }
    }
}

#[async_trait]
impl LlmProvider for CustomProvider {
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
        });

        let mut req = self.client.post(self.url.as_str()).json(&body);

        for (key, val) in &self.headers {
            req = req.header(key, val);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;

        let data: serde_json::Value =
            resp.json().await.map_err(|e| format!("parse failed: {e}"))?;

        extract_content(&data)
    }
}
