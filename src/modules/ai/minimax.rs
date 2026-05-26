use crate::shared::config::AppConfig;
use crate::shared::db::DbPool;
use crate::shared::error::{AppError, AppResult};
use crate::shared::types::ChatId;
use super::tools::{execute_tool, get_minimax_tools_schema};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use tracing::{debug, error, info, warn};

const DEFAULT_MINIMAX_MODEL: &str = "abab6.5g-chat";
const MAX_HISTORY_TURNS: usize = 20;

pub const SYSTEM_PROMPT: &str = "\
Вы — опытный и вежливый персональный ассистент и продавец парфюмерии в Telegram. Ваша цель — помочь клиенту выбрать парфюм из каталога, оформить заказ, собрать контактные данные (имя, телефон, адрес доставки) и подтвердить оплату.
У вас есть доступ к локальной базе данных через инструменты (tools). Всегда используйте их для получения актуального каталога, проверки остатков на складе, сохранения контактов и создания заказов.

Правила работы:
1. Валидация числовых кодов (Правило 2 цифр): Если клиент присылает числовой код подтверждения (для скидки, оплаты или верификации), вы должны убедиться, что код состоит ровно из 2 цифр (двузначное число). Если код неверный или содержит другое количество цифр, вежливо укажите на ошибку и попросите прислать корректный двухзначный код.
2. Реакции (Лайки): Когда клиент совершает позитивное или успешное действие (подтверждает контакты, делает заказ, успешно оплачивает товар или оставляет приятный отзыв), вы можете поставить реакцию на его сообщение. Для этого добавьте в самом конце вашего ответа (на отдельной строке) маркер вида:
[REACTION: 👍]
Вы можете использовать другие подходящие эмодзи, например: ❤️, 🔥, 👌. Наш бот автоматически считает этот маркер и отправит реакцию пользователю.
3. Общение: Общайтесь на русском языке, вежливо и дружелюбно. Будьте лаконичны и профессиональны.";

// --- MiniMax OpenAI-compatible Payload Structures ---

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MiniMaxMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<MiniMaxToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MiniMaxToolCall {
    pub id: String,
    pub r#type: String, // Always "function"
    pub function: MiniMaxFunctionCall,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MiniMaxFunctionCall {
    pub name: String,
    pub arguments: String, // JSON string
}

#[derive(Debug, Serialize)]
struct MiniMaxRequest {
    model: String,
    messages: Vec<MiniMaxMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MiniMaxResponse {
    pub choices: Option<Vec<MiniMaxChoice>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MiniMaxChoice {
    pub message: Option<MiniMaxMessage>,
    pub finish_reason: Option<String>,
}

// --- Global Dialog History Memory Cache ---

pub static CHAT_HISTORIES: LazyLock<Mutex<HashMap<ChatId, Vec<MiniMaxMessage>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

// --- Main Dialog Manager Entry Point ---

pub async fn run_dialog(
    client: &Client,
    config: &AppConfig,
    pool: &DbPool,
    chat_id: ChatId,
    user_text: &str,
) -> AppResult<(String, Option<String>)> {
    if config.minimax_api_key.trim().is_empty() || config.minimax_api_key == "minimax_dummy_key" {
        let err_msg = "MiniMax API key is not configured in settings or environment. Please go to web settings to configure it.";
        error!("[AI.MiniMax] {err_msg}");
        return Err(AppError::AiApi(err_msg.to_string()));
    }

    info!("[AI.MiniMax] Executing dialog turn for chat_id: {chat_id}");

    // 1. Get or create history for this ChatId
    let mut history = {
        let mut histories = CHAT_HISTORIES.lock().unwrap();
        histories.entry(chat_id).or_insert_with(|| {
            vec![MiniMaxMessage {
                role: "system".to_string(),
                content: Some(SYSTEM_PROMPT.to_string()),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            }]
        }).clone()
    };

    // 2. Append new user message to history
    history.push(MiniMaxMessage {
        role: "user".to_string(),
        content: Some(user_text.to_string()),
        tool_calls: None,
        tool_call_id: None,
        name: None,
    });

    // 3. Initiate Chat completions loop to handle tool calls
    let mut loop_count = 0;
    let url = format!(
        "https://api.minimax.io/v1/chat/completions?GroupId={}",
        config.minimax_group_id
    );

    loop {
        loop_count += 1;
        if loop_count > 6 {
            error!("[AI.MiniMax] Tool calling exceeded loop recursion limit (6 turns)");
            return Err(AppError::AiApi("Too many sequential tool calls".to_string()));
        }

        debug!("[AI.MiniMax] Sending completions request (turn {loop_count})...");
        let payload = MiniMaxRequest {
            model: DEFAULT_MINIMAX_MODEL.to_string(),
            messages: history.clone(),
            tools: Some(get_minimax_tools_schema()),
            tool_choice: Some("auto".to_string()),
        };

        let req_builder = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", config.minimax_api_key))
            .header("Content-Type", "application/json")
            .json(&payload);

        let res = match crate::shared::alerting::send_with_retry(req_builder, 3).await {
            Ok(response) => response,
            Err(err) => {
                let err_msg = format!("MiniMax API completions request failed: {err}");
                error!("[AI.MiniMax] {err_msg}");
                let _ = crate::shared::alerting::send_alert(&err_msg).await;
                return Err(AppError::Http(err));
            }
        };

        if !res.status().is_success() {
            let status = res.status();
            let err_text = res.text().await.unwrap_or_default();
            let err_msg = format!("MiniMax completions failed: HTTP {status} - {err_text}");
            error!("[AI.MiniMax] {err_msg}");
            let _ = crate::shared::alerting::send_alert(&err_msg).await;
            return Err(AppError::AiApi(err_msg));
        }

        let response: MiniMaxResponse = res.json().await?;
        let choice = response
            .choices
            .as_ref()
            .and_then(|c| c.first())
            .ok_or_else(|| {
                error!("[AI.MiniMax] API returned empty choices response structure");
                AppError::AiApi("MiniMax API choices structure was empty".to_string())
            })?;

        let assistant_message = choice.message.as_ref().ok_or_else(|| {
            error!("[AI.MiniMax] API choice contains empty message");
            AppError::AiApi("MiniMax API response message was empty".to_string())
        })?;

        // Append assistant message to local history tracker
        history.push(assistant_message.clone());

        // Check if the assistant wants to execute one or more tools
        if let Some(ref tool_calls) = assistant_message.tool_calls {
            if tool_calls.is_empty() {
                break;
            }

            info!("[AI.MiniMax] Model returned {} tool_calls", tool_calls.len());
            for tool_call in tool_calls {
                let tool_name = &tool_call.function.name;
                let tool_args = &tool_call.function.arguments;

                // Asynchronously execute database tool
                let tool_result = match execute_tool(tool_name, tool_args, pool).await {
                    Ok(result) => result,
                    Err(err) => {
                        warn!("[AI.MiniMax] Tool '{tool_name}' failed with error: {err}");
                        json!({ "status": "error", "error": err.to_string() }).to_string()
                    }
                };

                debug!("[AI.MiniMax] Tool '{tool_name}' result: {tool_result}");

                // Append tool completion back to history
                history.push(MiniMaxMessage {
                    role: "tool".to_string(),
                    content: Some(tool_result),
                    tool_calls: None,
                    tool_call_id: Some(tool_call.id.clone()),
                    name: Some(tool_name.clone()),
                });
            }

            // Continue the loop to resubmit histories to MiniMax
            continue;
        }

        // No tool calls returned, we have the final textual response!
        break;
    }

    // 4. Retrieve final assistant message text
    let final_msg = history.last().ok_or_else(|| {
        AppError::AiApi("Dialog history trace is unexpectedly empty".to_string())
    })?;
    let raw_text = final_msg.content.as_deref().unwrap_or("").to_string();

    // 5. Parse for [REACTION: 👍] tag
    let (cleaned_text, reaction) = parse_reaction_marker(&raw_text);

    // 6. Update global history in cache (keeping history trimmed to size)
    {
        let mut histories = CHAT_HISTORIES.lock().unwrap();
        // Trim old elements if they exceed history turn count limit
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
        "[AI.MiniMax] Dialogue turn completed. Cleaned text len: {}. Reaction: {:?}",
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
            
            // Clean up text by stripping the entire reaction line/tag
            let before = &text[..start_idx];
            let after = &text[start_idx + end_idx + 1..];
            
            let mut cleaned = format!("{}{}", before, after);
            cleaned = cleaned.trim().to_string();
            
            return (cleaned, Some(emoji));
        }
    }
    (text.trim().to_string(), None)
}
