use crate::shared::config::AppConfig;
use crate::shared::db::DbPool;
use crate::shared::error::{AppError, AppResult};
use crate::shared::types::ChatId;
use super::tools::execute_tool;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use tracing::{debug, error, info, warn};

pub const SYSTEM_PROMPT: &str = "\
Вы — опытный и вежливый персональный ассистент и продавец парфюмерии в Telegram. Ваша цель — помочь клиенту выбрать парфюм из каталога, оформить заказ, собрать контактные данные (имя, телефон, адрес доставки) и подтвердить оплату.
У вас есть доступ к локальной базе данных через инструменты (tools). Всегда используйте их для получения актуального каталога, проверки остатков на складе, сохранения контактов и создания заказов.

Правила работы:
1. Валидация числовых кодов (Правило 2 цифр): Если клиент присылает числовой код подтверждения (для скидки, оплаты или верификации), вы должны убедиться, что код состоит ровно из 2 цифр (двузначное число). Если код неверный или содержит другое количество цифр, вежливо укажите на ошибку и попросите прислать корректный двухзначный код.
2. Реакции (Лайки): Когда клиент совершает позитивное или успешное действие (подтверждает контакты, делает заказ, успешно оплачивает товар или оставляет приятный отзыв), вы можете поставить реакцию на его сообщение. Для этого добавьте в самом конце вашего ответа (на отдельной строке) маркер вида:
[REACTION: 👍]
Вы можете использовать другие подходящие эмодзи, например: ❤️, 🔥, 👌. Наш бот автоматически считает этот маркер и отправит реакцию пользователю.
3. Общение: Общайтесь на русском языке, вежливо и дружелюбно. Будьте лаконичны и профессиональны.";

const MAX_HISTORY_TURNS: usize = 20;

// --- Anthropic API Payload Structures ---

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AnthropicMessage {
    pub role: String, // "user" or "assistant"
    pub content: AnthropicContent,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum AnthropicContent {
    SingleText(String),
    MultipleBlocks(Vec<AnthropicBlock>),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnthropicBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

pub static CHAT_HISTORIES: LazyLock<Mutex<HashMap<ChatId, Vec<AnthropicMessage>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

// Helper to convert OpenAI schema to Anthropic tools schema
pub fn get_anthropic_tools_schema() -> serde_json::Value {
    let minimax_schema = crate::modules::ai::tools::get_minimax_tools_schema();
    let mut anthropic_tools = Vec::new();
    if let Some(arr) = minimax_schema.as_array() {
        for entry in arr {
            if let Some(func) = entry.get("function") {
                let name = func.get("name").cloned().unwrap_or(serde_json::Value::Null);
                let description = func.get("description").cloned().unwrap_or(serde_json::Value::Null);
                let input_schema = func.get("parameters").cloned().unwrap_or(serde_json::Value::Null);
                
                anthropic_tools.push(serde_json::json!({
                    "name": name,
                    "description": description,
                    "input_schema": input_schema
                }));
            }
        }
    }
    serde_json::Value::Array(anthropic_tools)
}

pub async fn run_dialog(
    client: &Client,
    config: &AppConfig,
    pool: &DbPool,
    chat_id: ChatId,
    user_text: &str,
) -> AppResult<(String, Option<String>)> {
    if config.minimax_api_key.trim().is_empty() || config.minimax_api_key == "minimax_dummy_key" {
        let err_msg = "Anthropic/MiniMax API key is not configured in settings or environment. Please go to web settings to configure it.";
        error!("[AI.MiniMax] {err_msg}");
        return Err(AppError::AiApi(err_msg.to_string()));
    }

    info!("[AI.MiniMax] Executing dialog turn (Anthropic Messages API) for chat_id: {chat_id}");

    // 1. Get or create history for this ChatId
    let mut history = {
        let mut histories = CHAT_HISTORIES.lock().unwrap();
        histories.entry(chat_id).or_insert_with(|| vec![]).clone()
    };

    // 2. Append new user message to history
    history.push(AnthropicMessage {
        role: "user".to_string(),
        content: AnthropicContent::SingleText(user_text.to_string()),
    });

    // 3. Initiate completions loop to handle tool calls
    let mut loop_count = 0;
    let url = "https://api.anthropic.com/v1/messages";

    loop {
        loop_count += 1;
        if loop_count > 6 {
            error!("[AI.MiniMax] Tool calling exceeded loop recursion limit (6 turns)");
            return Err(AppError::AiApi("Too many sequential tool calls".to_string()));
        }

        debug!("[AI.MiniMax] Sending Anthropic completions request (turn {loop_count})...");
        let payload = serde_json::json!({
            "model": "MiniMax-M2.7",
            "max_tokens": 1024,
            "system": SYSTEM_PROMPT,
            "messages": history,
            "tools": get_anthropic_tools_schema()
        });

        let req_builder = client
            .post(url)
            .header("x-api-key", &config.minimax_api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&payload);

        let res = match crate::shared::alerting::send_with_retry(req_builder, 3).await {
            Ok(response) => response,
            Err(err) => {
                let err_msg = format!("Anthropic/MiniMax API request failed: {err}");
                error!("[AI.MiniMax] {err_msg}");
                let _ = crate::shared::alerting::send_alert(&err_msg).await;
                return Err(AppError::Http(err));
            }
        };

        if !res.status().is_success() {
            let status = res.status();
            let err_text = res.text().await.unwrap_or_default();
            let err_msg = format!("Anthropic/MiniMax completions failed: HTTP {status} - {err_text}");
            error!("[AI.MiniMax] {err_msg}");
            let _ = crate::shared::alerting::send_alert(&err_msg).await;
            return Err(AppError::AiApi(err_msg));
        }

        let response: serde_json::Value = res.json().await?;
        
        let role = response["role"].as_str().unwrap_or("assistant").to_string();
        let content_val = response["content"].clone();
        
        let assistant_blocks: Vec<AnthropicBlock> = serde_json::from_value(content_val)
            .map_err(|e| AppError::AiApi(format!("Failed to parse Anthropic response blocks: {e}")))?;

        // Append assistant message to local history tracker
        history.push(AnthropicMessage {
            role,
            content: AnthropicContent::MultipleBlocks(assistant_blocks.clone()),
        });

        // Check if the assistant wants to execute one or more tools
        let mut tool_uses = Vec::new();
        for block in &assistant_blocks {
            if let AnthropicBlock::ToolUse { id, name, input } = block {
                tool_uses.push((id.clone(), name.clone(), input.clone()));
            }
        }

        if tool_uses.is_empty() {
            break;
        }

        info!("[AI.MiniMax] Model returned {} tool_calls", tool_uses.len());
        let mut tool_results = Vec::new();
        
        for (id, name, input) in tool_uses {
            let args_str = input.to_string();

            // Asynchronously execute database tool
            let tool_result = match execute_tool(&name, &args_str, pool).await {
                Ok(result) => result,
                Err(err) => {
                    warn!("[AI.MiniMax] Tool '{name}' failed with error: {err}");
                    serde_json::json!({ "status": "error", "error": err.to_string() }).to_string()
                }
            };

            debug!("[AI.MiniMax] Tool '{name}' result: {tool_result}");

            // Append tool completion back to history
            tool_results.push(AnthropicBlock::ToolResult {
                tool_use_id: id,
                content: tool_result,
            });
        }

        history.push(AnthropicMessage {
            role: "user".to_string(),
            content: AnthropicContent::MultipleBlocks(tool_results),
        });

        // Continue the loop to resubmit histories
        continue;
    }

    // 4. Retrieve final assistant message text by combining text blocks
    let mut raw_text = String::new();
    if let Some(last_msg) = history.last() {
        match &last_msg.content {
            AnthropicContent::SingleText(t) => {
                raw_text = t.clone();
            }
            AnthropicContent::MultipleBlocks(blocks) => {
                for block in blocks {
                    if let AnthropicBlock::Text { text } = block {
                        raw_text.push_str(text);
                    }
                }
            }
        }
    }

    // 5. Parse for [REACTION: 👍] tag
    let (cleaned_text, reaction) = parse_reaction_marker(&raw_text);

    // 6. Update global history in cache (keeping history trimmed to size)
    {
        let mut histories = CHAT_HISTORIES.lock().unwrap();
        if history.len() > MAX_HISTORY_TURNS {
            let trimmed = history.split_off(history.len() - MAX_HISTORY_TURNS);
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
