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

/// Best-effort, dependency-free language detection from the USER'S TEXT.
///
/// Distinct-script languages (CJK, Cyrillic, Arabic, etc.) are detected
/// reliably from their Unicode block. Latin-script detection uses common
/// stopwords and only returns a name when there is a clear, unique winner;
/// otherwise we return `None` and the caller instructs the model to continue
/// in the same language as the text. We NEVER consult the dictionary/settings
/// language — the text itself is the only source of truth.
fn detect_language(text: &str) -> Option<&'static str> {
    let mut cjk = 0usize;
    let mut kana = 0usize;
    let mut hangul = 0usize;
    let mut cyrillic = 0usize;
    let mut arabic = 0usize;
    let mut greek = 0usize;
    let mut hebrew = 0usize;
    let mut devanagari = 0usize;
    let mut thai = 0usize;
    let mut latin = 0usize;

    for ch in text.chars() {
        if !ch.is_alphabetic() {
            continue;
        }
        let cp = ch as u32;
        if (0x3040..=0x30FF).contains(&cp) {
            kana += 1; // Hiragana / Katakana
        } else if (0x4E00..=0x9FFF).contains(&cp) || (0x3400..=0x4DBF).contains(&cp) {
            cjk += 1;
        } else if (0xAC00..=0xD7AF).contains(&cp) || (0x1100..=0x11FF).contains(&cp) {
            hangul += 1;
        } else if (0x0400..=0x04FF).contains(&cp) {
            cyrillic += 1;
        } else if (0x0600..=0x06FF).contains(&cp) || (0x0750..=0x077F).contains(&cp) {
            arabic += 1;
        } else if (0x0370..=0x03FF).contains(&cp) {
            greek += 1;
        } else if (0x0590..=0x05FF).contains(&cp) {
            hebrew += 1;
        } else if (0x0900..=0x097F).contains(&cp) {
            devanagari += 1;
        } else if (0x0E00..=0x0E7F).contains(&cp) {
            thai += 1;
        } else if ch.is_ascii_alphabetic() {
            latin += 1;
        }
    }

    // Distinct scripts: detection is essentially certain.
    if kana > 0 {
        return Some("Japanese");
    }
    if hangul > 0 {
        return Some("Korean");
    }
    if cjk > 0 {
        return Some("Chinese");
    }
    if cyrillic > 0 {
        return Some("Russian");
    }
    if arabic > 0 {
        return Some("Arabic");
    }
    if greek > 0 {
        return Some("Greek");
    }
    if hebrew > 0 {
        return Some("Hebrew");
    }
    if devanagari > 0 {
        return Some("Hindi");
    }
    if thai > 0 {
        return Some("Thai");
    }

    // Latin script: score by distinctive stopwords. Only commit when there is
    // a unique winner — otherwise let the model continue in the text's language.
    if latin >= 2 {
        let lower = text.to_lowercase();
        let tokens: std::collections::HashSet<&str> = lower.split_whitespace().collect();
        let score = |words: &[&str]| {
            // Count each distinct stopword once so repeated entries can't
            // inflate a language's score.
            let mut seen = std::collections::HashSet::new();
            words.iter().filter(|w| tokens.contains(**w) && seen.insert(**w)).count()
        };

        // Lists favour high-specificity function words / contractions.
        let scores: [(&str, usize); 8] = [
            ("Portuguese", score(&["não", "você", "está", "sou", "estou", "uma", "isso", "olá", "tudo", "bom", "com", "para"])),
            ("Spanish", score(&["no", "qué", "está", "estoy", "muy", "bueno", "hola", "para", "con", "los", "las", "una"])),
            ("French", score(&["ne", "pas", "vous", "être", "avec", "les", "est", "je", "suis", "bon", "pour", "une"])),
            ("German", score(&["nicht", "ist", "ein", "eine", "ich", "und", "der", "die", "das", "mit", "für", "sehr"])),
            ("Italian", score(&["che", "sono", "per", "una", "come", "perché", "ciao", "non", "è", "molto", "buono", "con"])),
            ("Dutch", score(&["het", "een", "niet", "is", "en", "van", "ik", "ben", "met", "goed", "voor", "wel"])),
            ("English", score(&["the", "and", "is", "are", "you", "not", "with", "for", "hello", "good", "have", "this"])),
            ("Romanian", score(&["nu", "sunt", "pentru", "o", "cu", "mai", "acest", "bun", "salut", "foarte", "este", "îmi"])),
        ];

        let mut best = ("", 0usize);
        let mut tie = false;
        for (name, s) in scores {
            match s.cmp(&best.1) {
                std::cmp::Ordering::Greater => {
                    best = (name, s);
                    tie = false;
                }
                std::cmp::Ordering::Equal if best.1 > 0 => tie = true,
                _ => {}
            }
        }
        if !tie && best.1 >= 1 {
            return Some(match best.0 {
                "Portuguese" => "Portuguese",
                "Spanish" => "Spanish",
                "French" => "French",
                "German" => "German",
                "Italian" => "Italian",
                "Dutch" => "Dutch",
                "English" => "English",
                "Romanian" => "Romanian",
                _ => return None,
            });
        }
    }

    None
}

/// Parse an LLM "next-words" response into a clean, de-duplicated list.
///
/// Handles numbered/bulleted lines, comma-separated options, surrounding
/// quotes, and trailing punctuation. Only drops a candidate when it is an
/// exact echo of the last typed word or a case-insensitive duplicate — never
/// for merely sharing a substring with the user's text.
fn parse_suggestions(raw: &str, context: &str, count: usize) -> Vec<String> {
    let last_word = context
        .split_whitespace()
        .next_back()
        .map(str::to_lowercase)
        .unwrap_or_default();

    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut out: Vec<String> = Vec::new();

    for line in raw.lines() {
        for cand in line.split(',') {
            let mut s = cand.trim();
            // Repeatedly strip leading list markers / numbers / dots so that
            // patterns like "1. ", "- ", ". . " all vanish.
            loop {
                let stripped = s
                    .trim_start_matches(['-', '*', '.', ' '])
                    .trim_start_matches(|c: char| c.is_ascii_digit())
                    .trim_start_matches(['.', ' ']);
                if stripped.len() == s.len() {
                    break;
                }
                s = stripped;
            }
            let s = s
                .trim_matches(|c: char| c == '"' || c == '\'' || c == '“' || c == '”' || c == '«' || c == '»')
                .trim_end_matches(|c: char| c.is_ascii_punctuation())
                .trim();
            if s.is_empty() {
                continue;
            }
            let lower = s.to_lowercase();
            if !last_word.is_empty() && lower == last_word {
                continue;
            }
            if !seen.insert(lower) {
                continue;
            }
            out.push(s.to_string());
            if out.len() >= count {
                return out;
            }
        }
    }
    out
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
        .ok_or_else(|| {
            let finish = data["choices"][0]["finish_reason"].as_str().unwrap_or("unknown");
            let model = data["model"].as_str().unwrap_or("?");
            format!("model {model} returned empty content (finish_reason={finish}). The model may not have a matching chat template or the prompt was rejected.")
        })?;

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
    async fn complete_raw(&self, system: &str, prompt: &str, max_tokens: u32) -> Result<String, String>;

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
        self.complete_raw(&system, &prompt, 300).await
    }

    async fn explain_text(&self, text: &str) -> Result<String, String> {
        let system = "You are an expert explainer. \
                      The user sends you text and you explain what it means in simple terms. \
                      Explain in the SAME language as the original text — do NOT translate. \
                      Output ONLY the explanation — no thinking, no repetition of the original."
            .to_string();
        let prompt = format!("Explain this text:\n\n{text}\n\nExplanation:");
        self.complete_raw(&system, &prompt, 400).await
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
        self.complete_raw(&system, &prompt, 500).await
    }

    async fn suggest_next_words(&self, text: &str, count: usize) -> Result<Vec<String>, String> {
        // Use only the trailing words as context — it keeps the request fast
        // for local models and reduces the urge to echo the whole text back.
        let last_words: Vec<&str> = text.split_whitespace().rev().take(12).collect();
        let context = last_words.iter().rev().copied().collect::<Vec<_>>().join(" ");
        if context.trim().is_empty() {
            return Ok(Vec::new());
        }

        // The language is derived ONLY from the user's text — never from the
        // dictionary selection or any other setting. If we can name it
        // confidently we tell the model; otherwise we instruct it to match the
        // text's language, which it can read directly.
        let lang_clause = match detect_language(&context) {
            Some(lang) => format!(
                "The text is written in {lang}. \
                 Continue EXCLUSIVELY in {lang}. \
                 Do NOT translate it or switch to another language."
            ),
            None => "Continue EXCLUSIVELY in the SAME language as the text. \
                     Detect the language from the text itself and keep it. \
                     Do NOT translate it or switch to another language."
                .to_string(),
        };

        let system = format!(
            "You are an autocomplete engine. \
             Given text that a user is typing, you predict the most likely word(s) or phrase(s) that come NEXT.\n\
             Complete the user's sentence with something that makes sense in the context of the rest of the sentence.\n\
             {lang_clause}\n\
             The user's text so far: \"{context}\"\n\
             Suggest {count} different continuations (word or phrase).\n\
             RULES:\n\
             - Output ONLY the suggestions, one per line. No list, numbers, bullets, quotes, or markdown.\n\
             - Each suggestion must be a grammatically correct continuation of the text \
             (match gender, number, and tense; articles, prepositions, pronouns, and auxiliary \
             verbs are perfectly fine when they are the natural next word).\n\
             - Make the options genuinely different from each other.\n\
             - At most ONE option may be a short phrase (up to 4 words); the rest must be single words.\n\
             - Do NOT repeat or copy the user's text back.\n\
             - Do not repeat previous phrases or words; create only the sequences.\n\
             - Do NOT add explanations, labels, introductions, or any text other than the suggestions."
        );
        let prompt = format!("List the {count} most likely next word or phrase for: \"{context}\"");
        let raw = self.complete_raw(&system, &prompt, 20).await?;
        eprintln!("[plume] ai_suggest raw response: {raw:?}");

        Ok(parse_suggestions(&raw, &context, count))
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
    async fn complete_raw(&self, system: &str, prompt: &str, max_tokens: u32) -> Result<String, String> {
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": prompt}
            ],
            "stream": false,
            "max_tokens": max_tokens,
            "temperature": 0.1,
            "reasoning_format": "none",
        });

        let url = format!("{}/v1/chat/completions", self.endpoint);
        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Ollama request to {url} failed after {max_tokens} max tokens: {e}"))?;

        let data: serde_json::Value =
            resp.json().await.map_err(|e| format!("Ollama parse error from {url}: {e}"))?;

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
    async fn complete_raw(&self, system: &str, prompt: &str, max_tokens: u32) -> Result<String, String> {
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": prompt}
            ],
            "max_tokens": max_tokens,
            "temperature": 0.1,
        });

        let url = self.endpoint.as_str();
        let resp = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("OpenAI request to {url} failed: {e}"))?;

        let data: serde_json::Value =
            resp.json().await.map_err(|e| format!("OpenAI parse error from {url}: {e}"))?;

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
    async fn complete_raw(&self, system: &str, prompt: &str, max_tokens: u32) -> Result<String, String> {
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": prompt}
            ],
            "stream": false,
            "max_tokens": max_tokens,
            "temperature": 0.1,
            "reasoning_format": "none",
            "chat_template_kwargs": {
                "enable_thinking": false
            },
        });

        let url = format!("{}/v1/chat/completions", self.endpoint);
        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("llama.cpp request to {url} failed (max_tokens={max_tokens}): {e}"))?;

        let data: serde_json::Value =
            resp.json().await.map_err(|e| format!("llama.cpp parse error from {url}: {e}"))?;

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
    async fn complete_raw(&self, system: &str, prompt: &str, max_tokens: u32) -> Result<String, String> {
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": prompt}
            ],
            "stream": false,
            "max_tokens": max_tokens,
            "temperature": 0.1,
        });

        let mut req = self.client.post(self.url.as_str()).json(&body);

        for (key, val) in &self.headers {
            req = req.header(key, val);
        }

        let url = &self.url;
        let resp = req
            .send()
            .await
            .map_err(|e| format!("Custom API request to {url} failed: {e}"))?;

        let data: serde_json::Value =
            resp.json().await.map_err(|e| format!("Custom API parse error from {url}: {e}"))?;

        extract_content(&data)
    }
}
