use crate::shared::config::AppConfig;
use crate::shared::error::{AppError, AppResult};
use base64::{prelude::BASE64_STANDARD, Engine};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};

const DEFAULT_OPENROUTER_MODEL: &str = "google/gemini-2.5-flash";
const DEFAULT_DIRECT_GEMINI_MODEL: &str = "gemini-1.5-flash";

// --- Direct Gemini API Payload Structures ---

#[derive(Debug, Serialize, Deserialize)]
pub struct GeminiRequest {
    pub contents: Vec<GeminiContent>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GeminiContent {
    pub parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GeminiPart {
    Text { text: String },
    InlineData { inline_data: GeminiInlineData },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GeminiInlineData {
    pub mime_type: String,
    pub data: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GeminiResponse {
    pub candidates: Option<Vec<GeminiCandidate>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GeminiCandidate {
    pub content: Option<GeminiResponseContent>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GeminiResponseContent {
    pub parts: Option<Vec<GeminiResponsePart>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GeminiResponsePart {
    pub text: Option<String>,
}

// --- OpenRouter API Payload Structures ---

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenRouterRequest {
    pub model: String,
    pub messages: Vec<OpenRouterMessage>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenRouterMessage {
    pub role: String,
    pub content: Vec<OpenRouterContent>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OpenRouterContent {
    Text { text: String },
    ImageUrl { image_url: OpenRouterImageUrl },
    InputAudio { input_audio: OpenRouterInputAudio },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenRouterImageUrl {
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenRouterInputAudio {
    pub data: String,
    pub format: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenRouterResponse {
    pub choices: Option<Vec<OpenRouterChoice>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenRouterChoice {
    pub message: Option<OpenRouterChoiceMessage>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenRouterChoiceMessage {
    pub content: Option<String>,
}

// --- High Level Functions ---

/// Transcribe an audio file using Gemini.
/// Expects a file path and automatically decodes and sends it.
pub async fn transcribe_voice(
    client: &Client,
    config: &AppConfig,
    file_path: &str,
) -> AppResult<String> {
    info!("[AI.Gemini] Transcribing voice file: {file_path}");
    
    // Default to audio/ogg for telegram voice notes
    let mime_type = "audio/ogg";
    let prompt = "Please transcribe this audio recording accurately. Only return the transcribed text, without any additional comments, prefix, or metadata. If there is no speech or it is silent, say exactly: '(silence)'";
    
    process_multimodal(client, config, file_path, mime_type, prompt).await
}

/// Analyze an image file (e.g. perfume photo or payment screenshot) using Gemini.
pub async fn analyze_image(
    client: &Client,
    config: &AppConfig,
    file_path: &str,
    prompt: &str,
) -> AppResult<String> {
    info!("[AI.Gemini] Analyzing image: {file_path}");
    
    // Deduce mime type from extension or default to image/jpeg
    let mime_type = if file_path.ends_with(".png") {
        "image/png"
    } else {
        "image/jpeg"
    };
    
    process_multimodal(client, config, file_path, mime_type, prompt).await
}

// --- Common Multimodal Processor ---

pub async fn process_multimodal(
    client: &Client,
    config: &AppConfig,
    file_path: &str,
    mime_type: &str,
    prompt: &str,
) -> AppResult<String> {
    debug!(
        "[AI.Gemini] Reading and encoding file: {file_path} (MIME: {mime_type})"
    );
    
    // 1. Read file bytes and encode to base64
    let bytes = std::fs::read(file_path)?;
    let base64_data = BASE64_STANDARD.encode(&bytes);
    
    // 2. Select route based on presence of OpenRouter API Key
    if let Some(ref openrouter_key) = config.openrouter_api_key {
        debug!("[AI.Gemini] Route: OpenRouter API");
        call_openrouter(client, openrouter_key, base64_data, mime_type, prompt).await
    } else {
        debug!("[AI.Gemini] Route: Direct Google Gemini API");
        call_direct_gemini(client, &config.gemini_api_key, base64_data, mime_type, prompt).await
    }
}

// --- Private Helper API Calls ---

async fn call_direct_gemini(
    client: &Client,
    api_key: &str,
    base64_data: String,
    mime_type: &str,
    prompt: &str,
) -> AppResult<String> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
        DEFAULT_DIRECT_GEMINI_MODEL
    );

    let request_payload = GeminiRequest {
        contents: vec![GeminiContent {
            parts: vec![
                GeminiPart::InlineData {
                    inline_data: GeminiInlineData {
                        mime_type: mime_type.to_string(),
                        data: base64_data,
                    },
                },
                GeminiPart::Text {
                    text: prompt.to_string(),
                },
            ],
        }],
    };

    debug!("[AI.Gemini] Dispatching direct Gemini API POST request...");
    let req_builder = client
        .post(&url)
        .header("x-goog-api-key", api_key)
        .header("Content-Type", "application/json")
        .json(&request_payload);

    let res = match crate::shared::alerting::send_with_retry(req_builder, 3).await {
        Ok(response) => response,
        Err(err) => {
            let err_msg = format!("Direct Gemini API request failed: {err}");
            error!("[AI.Gemini] {err_msg}");
            let _ = crate::shared::alerting::send_alert(&err_msg).await;
            return Err(AppError::Http(err));
        }
    };

    if !res.status().is_success() {
        let status = res.status();
        let err_text = res.text().await.unwrap_or_default();
        let err_msg = format!("Direct Gemini API failed: HTTP {status} - {err_text}");
        error!("[AI.Gemini] {err_msg}");
        let _ = crate::shared::alerting::send_alert(&err_msg).await;
        return Err(AppError::AiApi(err_msg));
    }

    let gemini_resp: GeminiResponse = res.json().await?;
    
    // Extract text from the candidate parts safely
    let parsed_text = gemini_resp
        .candidates
        .as_ref()
        .and_then(|cands| cands.first())
        .and_then(|cand| cand.content.as_ref())
        .and_then(|content| content.parts.as_ref())
        .and_then(|parts| parts.first())
        .and_then(|part| part.text.clone());

    match parsed_text {
        Some(text) => {
            debug!("[AI.Gemini] Successfully received direct Gemini transcription/description");
            Ok(text)
        }
        None => {
            error!("[AI.Gemini] Gemini API returned empty or invalid candidates response structure");
            Err(AppError::AiApi(
                "Gemini API candidates structure was empty or malformed".to_string(),
            ))
        }
    }
}

async fn call_openrouter(
    client: &Client,
    api_key: &str,
    base64_data: String,
    mime_type: &str,
    prompt: &str,
) -> AppResult<String> {
    let url = "https://openrouter.ai/api/v1/chat/completions";

    // Format content appropriately based on voice or image
    let media_content = if mime_type.starts_with("audio/") {
        // Extract voice format from mime type (e.g. audio/ogg -> ogg, audio/mp3 -> mp3)
        let format = mime_type
            .split('/')
            .nth(1)
            .unwrap_or("ogg")
            .to_string();
            
        OpenRouterContent::InputAudio {
            input_audio: OpenRouterInputAudio {
                data: base64_data,
                format,
            },
        }
    } else {
        OpenRouterContent::ImageUrl {
            image_url: OpenRouterImageUrl {
                url: format!("data:{mime_type};base64,{base64_data}"),
            },
        }
    };

    let request_payload = OpenRouterRequest {
        model: DEFAULT_OPENROUTER_MODEL.to_string(),
        messages: vec![OpenRouterMessage {
            role: "user".to_string(),
            content: vec![
                OpenRouterContent::Text {
                    text: prompt.to_string(),
                },
                media_content,
            ],
        }],
    };

    debug!("[AI.Gemini] Dispatching OpenRouter API completions POST request...");
    let req_builder = client
        .post(url)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&request_payload);

    let res = match crate::shared::alerting::send_with_retry(req_builder, 3).await {
        Ok(response) => response,
        Err(err) => {
            let err_msg = format!("OpenRouter API request failed: {err}");
            error!("[AI.Gemini] {err_msg}");
            let _ = crate::shared::alerting::send_alert(&err_msg).await;
            return Err(AppError::Http(err));
        }
    };

    if !res.status().is_success() {
        let status = res.status();
        let err_text = res.text().await.unwrap_or_default();
        let err_msg = format!("OpenRouter API failed: HTTP {status} - {err_text}");
        error!("[AI.Gemini] {err_msg}");
        let _ = crate::shared::alerting::send_alert(&err_msg).await;
        return Err(AppError::AiApi(err_msg));
    }

    let openrouter_resp: OpenRouterResponse = res.json().await?;
    
    let parsed_text = openrouter_resp
        .choices
        .as_ref()
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.message.as_ref())
        .and_then(|msg| msg.content.clone());

    match parsed_text {
        Some(text) => {
            debug!("[AI.Gemini] Successfully received OpenRouter transcription/description");
            Ok(text)
        }
        None => {
            error!("[AI.Gemini] OpenRouter API returned empty choices response structure");
            Err(AppError::AiApi(
                "OpenRouter API choices structure was empty or malformed".to_string(),
            ))
        }
    }
}

pub async fn run_text_dialog(
    client: &Client,
    config: &AppConfig,
    prompt: &str,
) -> AppResult<String> {
    debug!("[AI.Gemini] Initiating Gemini text dialogue fallback...");
    if let Some(ref openrouter_key) = config.openrouter_api_key {
        let url = "https://openrouter.ai/api/v1/chat/completions";
        let request_payload = serde_json::json!({
            "model": DEFAULT_OPENROUTER_MODEL,
            "messages": [
                { "role": "system", "content": crate::modules::ai::minimax::SYSTEM_PROMPT },
                { "role": "user", "content": prompt }
            ]
        });
        
        let req_builder = client
            .post(url)
            .header("Authorization", format!("Bearer {openrouter_key}"))
            .header("Content-Type", "application/json")
            .json(&request_payload);
            
        let res = crate::shared::alerting::send_with_retry(req_builder, 3).await
            .map_err(AppError::Http)?;
            
        if !res.status().is_success() {
            return Err(AppError::AiApi(format!("OpenRouter text dialog failed: HTTP {}", res.status())));
        }
        
        let resp: serde_json::Value = res.json().await?;
        let text = resp["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| AppError::AiApi("Empty OpenRouter text response".to_string()))?
            .to_string();
            
        Ok(text)
    } else {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            DEFAULT_DIRECT_GEMINI_MODEL, config.gemini_api_key
        );
        let request_payload = serde_json::json!({
            "contents": [
                {
                    "role": "user",
                    "parts": [
                        { "text": format!("System instructions:\n{}\n\nUser request:\n{}", crate::modules::ai::minimax::SYSTEM_PROMPT, prompt) }
                    ]
                }
            ]
        });
        
        let req_builder = client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request_payload);
            
        let res = crate::shared::alerting::send_with_retry(req_builder, 3).await
            .map_err(AppError::Http)?;
            
        if !res.status().is_success() {
            return Err(AppError::AiApi(format!("Gemini text dialog failed: HTTP {}", res.status())));
        }
        
        let resp: serde_json::Value = res.json().await?;
        let text = resp["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .ok_or_else(|| AppError::AiApi("Empty Gemini text response".to_string()))?
            .to_string();
            
        Ok(text)
    }
}
