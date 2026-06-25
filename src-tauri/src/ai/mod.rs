use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::config;

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().to_string() + c.as_str()).unwrap_or_default()
}

fn lang_native(lang: &str) -> &str {
    match lang.to_lowercase().as_str() {
        "portuguese" => "Português",
        "english" => "English",
        "spanish" => "Español",
        "french" => "Français",
        "german" => "Deutsch",
        "italian" => "Italiano",
        "russian" => "Русский",
        "japanese" => "日本語",
        "chinese" => "中文",
        "korean" => "한국어",
        "arabic" => "العربية",
        "dutch" => "Nederlands",
        _ => "",
    }
}

fn strip_thinking(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some(idx) = trimmed.find("\n\n") {
        trimmed[..idx].trim().to_string()
    } else {
        trimmed.to_string()
    }
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
        let target = capitalize(language);
        let native = lang_native(&target);
        let system = if native.is_empty() {
            format!(
                "You are a translator. \
                 The user sends you text and you translate it to {target}. \
                 TARGET LANGUAGE: {target}. \
                 Output ONLY the {target} translation — no explanations, no thinking, no original text."
            )
        } else {
            format!(
                "You are a translator. \
                 The user sends you text and you translate it to {target} ({native}). \
                 TARGET LANGUAGE: {target} / {native}. \
                 Output ONLY the {target} translation — no explanations, no thinking, no original text."
            )
        };
        let prompt = format!("Translate to {target}:\n\n{text}");
        self.complete_raw(&system, &prompt).await
    }

    async fn explain_text(&self, text: &str) -> Result<String, String> {
        let system = "You are an expert explainer. \
                      The user sends you text and you explain what it means in simple terms. \
                      Explain in the SAME language as the original text — do NOT translate. \
                      Output ONLY the explanation — no thinking, no repetition of the original."
            .to_string();
        let prompt = format!("Explain this text:\n\n{text}\n\nExplanation:");
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
            "makesense" => (
                "You are an expert explainer. \
                 Your ONLY task: explain what the user's text means in simple terms.\n\
                 Rules:\n\
                 - Rephrase the text to make its meaning clear.\n\
                 - Keep the same language as the original text.\n\
                 - Output ONLY the clarified text.\n\
                 - Do NOT add quotes or markdown."
                    .to_string(),
                format!("What does this text mean? Explain in simple terms, in the same language:\n\n{text}"),
            ),
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

    async fn complete_text(&self, prompt: &str, n_predict: u32, stop: &[&str]) -> Result<String, String> {
        eprintln!("[plume] complete_text prompt: {prompt}");

        let body = serde_json::json!({
            "prompt": prompt,
            "n_predict": n_predict,
            "temperature": 0.1,
            "stop": stop,
        });

        let resp = self
            .client
            .post(format!("{}/completion", self.endpoint))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;

        let data: serde_json::Value =
            resp.json().await.map_err(|e| format!("parse failed: {e}"))?;

        eprintln!("[plume] completion response: {data}");

        let raw = data["content"]
            .as_str()
            .or_else(|| data["choices"][0]["text"].as_str())
            .ok_or_else(|| "no content in response".to_string())?;

        let result = strip_thinking(raw);
        eprintln!("[plume] complete_text result: {result}");
        Ok(result)
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

    async fn suggest_next_words(&self, text: &str, count: usize, _lang: &str) -> Result<Vec<String>, String> {
        let last_words: Vec<&str> = text.split_whitespace().rev().take(10).collect();
        let context = last_words.iter().rev().copied().collect::<Vec<_>>().join(" ");

        // Few-shot: model sees pattern of "text → next words" and completes
        let prompt = format!(
            "The cat jumped on the → roof, fence, table\n\
             I went to the → store, park, beach\n\
             {context} →"
        );
        let raw = self.complete_text(&prompt, 64, &["\n"]).await?;
        eprintln!("[plume] ai_suggest raw response: {raw:?}");

        let last_word = text.split_whitespace().last().unwrap_or("").to_lowercase();

        let words: Vec<String> = raw
            .split([',', '\n'])
            .map(|s| s.trim().trim_start_matches(|c: char| c == '-' || c == '*' || c == '.' || c == ' ').to_string())
            .filter(|s| !s.is_empty())
            .filter(|s| {
                let s_lower = s.to_lowercase();
                s_lower != last_word && !last_word.contains(&s_lower)
            })
            .take(count)
            .collect();
        eprintln!("[plume] ai_suggest result: {words:?}");
        Ok(words)
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
