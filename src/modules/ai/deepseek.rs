use crate::shared::config::AppConfig;
use crate::shared::db::DbPool;
use crate::shared::error::{AppError, AppResult};
use crate::shared::types::ChatId;
use super::tools::{execute_tool, get_minimax_tools_schema};
use super::minimax::SYSTEM_PROMPT;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use tracing::{debug, error, info, warn};

const MAX_HISTORY_TURNS: usize = 20;

// --- OpenAI-Compatible Payload Structures ---

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OpenAiMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<OpenAiToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OpenAiToolCall {
    pub id: String,
    pub r#type: String, // Always "function"
    pub function: OpenAiFunctionCall,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OpenAiFunctionCall {
    pub name: String,
    pub arguments: String, // JSON string
}

#[derive(Debug, Serialize)]
pub struct OpenAiRequest {
    pub model: String,
    pub messages: Vec<OpenAiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct OpenAiResponse {
    pub choices: Option<Vec<OpenAiChoice>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct OpenAiChoice {
    pub message: Option<OpenAiMessage>,
    pub finish_reason: Option<String>,
}

// --- Global Dialog History Cache ---

pub static DEEPSEEK_CHAT_HISTORIES: LazyLock<Mutex<HashMap<ChatId, Vec<OpenAiMessage>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

// --- Main DeepSeek dialogue Manager ---

pub async fn run_dialog(
    client: &Client,
    config: &AppConfig,
    pool: &DbPool,
    chat_id: ChatId,
    user_text: &str,
) -> AppResult<(String, Option<String>)> {
    info!("[AI.DeepSeek] Executing dialog turn for chat_id: {chat_id}");

    // 1. Load keys from the database settings table dynamically (has priority over .env)
    let keys = {
        let conn = pool.get().await?;
        conn.interact(|conn| -> Result<(Option<String>, Option<String>), rusqlite::Error> {
            let mut stmt = conn.prepare("SELECT key, value FROM settings WHERE key IN ('DEEPSEEK_API_KEY', 'OPENROUTER_API_KEY')")?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            
            let mut ds = None;
            let mut or = None;
            for row in rows {
                let (k, v) = row?;
                if k == "DEEPSEEK_API_KEY" && !v.trim().is_empty() {
                    ds = Some(v);
                } else if k == "OPENROUTER_API_KEY" && !v.trim().is_empty() {
                    or = Some(v);
                }
            }
            Ok((ds, or))
        }).await
        .map_err(|e| AppError::Database(rusqlite::Error::ToSqlConversionFailure(e.to_string().into())))?
        .unwrap_or((None, None))
    };

    let (db_deepseek_key, db_openrouter_key) = keys;

    // 2. Select API Endpoint, Key, and Model
    let (url, api_key, model, is_openrouter) = if let Some(key) = db_deepseek_key.or_else(|| config.deepseek_api_key.clone()) {
        debug!("[AI.DeepSeek] Route: Direct DeepSeek API");
        ("https://api.deepseek.com/v1/chat/completions", key, "deepseek-chat".to_string(), false)
    } else if let Some(key) = db_openrouter_key.or_else(|| config.openrouter_api_key.clone()) {
        debug!("[AI.DeepSeek] Route: OpenRouter DeepSeek API");
        ("https://openrouter.ai/api/v1/chat/completions", key, "deepseek/deepseek-chat".to_string(), true)
    } else {
        let err_msg = "No DeepSeek API key or OpenRouter API key is configured in settings or environment.";
        error!("[AI.DeepSeek] {err_msg}");
        return Err(AppError::AiApi(err_msg.to_string()));
    };

    // 3. Get or create history for this ChatId
    let mut history = {
        let mut histories = DEEPSEEK_CHAT_HISTORIES.lock().unwrap();
        histories.entry(chat_id).or_insert_with(|| {
            vec![OpenAiMessage {
                role: "system".to_string(),
                content: Some(SYSTEM_PROMPT.to_string()),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            }]
        }).clone()
    };

    // 4. Append new user message to history
    history.push(OpenAiMessage {
        role: "user".to_string(),
        content: Some(user_text.to_string()),
        tool_calls: None,
        tool_call_id: None,
        name: None,
    });

    // 5. Initiate Completions loop to handle tool calls
    let mut loop_count = 0;
    loop {
        loop_count += 1;
        if loop_count > 6 {
            error!("[AI.DeepSeek] Tool calling exceeded loop recursion limit (6 turns)");
            return Err(AppError::AiApi("Too many sequential tool calls".to_string()));
        }

        debug!("[AI.DeepSeek] Sending completions request (turn {loop_count})...");
        let payload = OpenAiRequest {
            model: model.clone(),
            messages: history.clone(),
            tools: Some(get_minimax_tools_schema()), // Reuse the standard JSON tool schemas
            tool_choice: Some("auto".to_string()),
        };

        let mut req_builder = client
            .post(url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&payload);

        if is_openrouter {
            req_builder = req_builder.header("X-Title", "Perfume Bot Seller");
        }

        let res = match crate::shared::alerting::send_with_retry(req_builder, 3).await {
            Ok(response) => response,
            Err(err) => {
                let err_msg = format!("DeepSeek API completions request failed: {err}");
                error!("[AI.DeepSeek] {err_msg}");
                let _ = crate::shared::alerting::send_alert(&err_msg).await;
                return Err(AppError::Http(err));
            }
        };

        if !res.status().is_success() {
            let status = res.status();
            let err_text = res.text().await.unwrap_or_default();
            let err_msg = format!("DeepSeek completions failed: HTTP {status} - {err_text}");
            error!("[AI.DeepSeek] {err_msg}");
            let _ = crate::shared::alerting::send_alert(&err_msg).await;
            return Err(AppError::AiApi(err_msg));
        }

        let response: OpenAiResponse = res.json().await?;
        let choice = response
            .choices
            .as_ref()
            .and_then(|c| c.first())
            .ok_or_else(|| {
                error!("[AI.DeepSeek] API returned empty choices response structure");
                AppError::AiApi("DeepSeek API choices structure was empty".to_string())
            })?;

        let assistant_message = choice.message.as_ref().ok_or_else(|| {
            error!("[AI.DeepSeek] API choice contains empty message");
            AppError::AiApi("DeepSeek API response message was empty".to_string())
        })?;

        // Append assistant message to local history tracker
        history.push(assistant_message.clone());

        // Check if the assistant wants to execute one or more tools
        if let Some(ref tool_calls) = assistant_message.tool_calls {
            if tool_calls.is_empty() {
                break;
            }

            info!("[AI.DeepSeek] Model returned {} tool_calls", tool_calls.len());
            for tool_call in tool_calls {
                let tool_name = &tool_call.function.name;
                let tool_args = &tool_call.function.arguments;

                // Asynchronously execute database tool
                let tool_result = match execute_tool(tool_name, tool_args, pool).await {
                    Ok(result) => result,
                    Err(err) => {
                        warn!("[AI.DeepSeek] Tool '{tool_name}' failed with error: {err}");
                        json!({ "status": "error", "error": err.to_string() }).to_string()
                    }
                };

                debug!("[AI.DeepSeek] Tool '{tool_name}' result: {tool_result}");

                // Append tool completion back to history
                history.push(OpenAiMessage {
                    role: "tool".to_string(),
                    content: Some(tool_result),
                    tool_calls: None,
                    tool_call_id: Some(tool_call.id.clone()),
                    name: Some(tool_name.clone()),
                });
            }

            // Continue the loop to resubmit histories
            continue;
        }

        // No tool calls returned, we have the final textual response!
        break;
    }

    // 6. Retrieve final assistant message text
    let final_msg = history.last().ok_or_else(|| {
        AppError::AiApi("Dialog history trace is unexpectedly empty".to_string())
    })?;
    let raw_text = final_msg.content.as_deref().unwrap_or("").to_string();

    // 7. Parse for [REACTION: 👍] tag
    let (cleaned_text, reaction) = parse_reaction_marker(&raw_text);

    // 8. Update global history in cache (keeping history trimmed to size)
    {
        let mut histories = DEEPSEEK_CHAT_HISTORIES.lock().unwrap();
        if history.len() > MAX_HISTORY_TURNS {
            let system_message = history.first().cloned();
            let mut trimmed = history.split_off(history.len() - MAX_HISTORY_TURNS);
            if let Some(sys) = system_message {
                trimmed.insert(0, sys);
            }
            histories.insert(chat_id, trimmed);
        } else {
            histories.insert(chat_id, history);
        }
    }

    info!(
        "[AI.DeepSeek] Dialogue turn completed. Cleaned text len: {}. Reaction: {:?}",
        cleaned_text.len(),
        reaction
    );
    Ok((cleaned_text, reaction))
}

// --- Helper Reaction Parser ---

fn parse_reaction_marker(text: &str) -> (String, Option<String>) {
    let tag = "[REACTION:";
    if let Some(start_idx) = text.find(tag) {
        if let Some(end_idx) = text[start_idx..].find(']') {
            let emoji_part = &text[start_idx + tag.len()..start_idx + end_idx];
            let emoji = emoji_part.trim().to_string();
            
            let before = &text[..start_idx];
            let after = &text[start_idx + end_idx + 1..];
            
            let mut cleaned = format!("{}{}", before, after);
            cleaned = cleaned.trim().to_string();
            
            return (cleaned, Some(emoji));
        }
    }
    (text.trim().to_string(), None)
}
