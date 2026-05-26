use crate::shared::error::{AppError, AppResult};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, error};

#[derive(Debug, Deserialize, Clone)]
pub struct Update {
    pub update_id: i64,
    pub message: Option<Message>,
    pub business_message: Option<Message>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Message {
    pub message_id: i64,
    pub chat: Chat,
    pub date: i64,
    pub text: Option<String>,
    pub voice: Option<Voice>,
    pub photo: Option<Vec<PhotoSize>>,
    pub business_connection_id: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Chat {
    pub id: i64,
    pub first_name: Option<String>,
    pub username: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Voice {
    pub file_id: String,
    pub duration: i32,
    pub mime_type: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PhotoSize {
    pub file_id: String,
    pub file_size: Option<i32>,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FileInfoResponse {
    pub ok: bool,
    pub result: Option<FileInfo>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FileInfo {
    pub file_id: String,
    pub file_path: Option<String>,
    pub file_size: Option<i64>,
}

/// Helper structure for getUpdates response
#[derive(Debug, Deserialize, Clone)]
pub struct UpdatesResponse {
    pub ok: bool,
    pub result: Vec<Update>,
}

/// Send a text message to a chat
pub async fn send_message(
    client: &Client,
    token: &str,
    chat_id: i64,
    text: &str,
    business_connection_id: Option<&str>,
) -> AppResult<()> {
    debug!("[Telegram.business] Sending message to chat {chat_id} (business_connection: {:?}): {text}", business_connection_id);
    let url = format!("https://api.telegram.org/bot{token}/sendMessage");

    let mut payload = serde_json::json!({
        "chat_id": chat_id,
        "text": text
    });

    if let Some(conn_id) = business_connection_id {
        if let Some(obj) = payload.as_object_mut() {
            obj.insert("business_connection_id".to_string(), serde_json::json!(conn_id));
        }
    }

    let res = client.post(&url)
        .json(&payload)
        .send()
        .await?;

    if !res.status().is_success() {
        let status = res.status();
        let err_text = res.text().await.unwrap_or_default();
        error!("[Telegram.business] Failed to send message: HTTP {status} - {err_text}");
        return Err(AppError::Telegram(format!("HTTP {status}: {err_text}")));
    }

    Ok(())
}

/// Add a reaction emoji to an existing message (using setMessageReaction endpoint)
pub async fn send_reaction(
    client: &Client,
    token: &str,
    chat_id: i64,
    message_id: i64,
    emoji: &str,
    business_connection_id: Option<&str>,
) -> AppResult<()> {
    debug!("[Telegram.business] Adding reaction '{emoji}' to message {message_id} in chat {chat_id} (business_connection: {:?})", business_connection_id);
    let url = format!("https://api.telegram.org/bot{token}/setMessageReaction");

    let mut payload = serde_json::json!({
        "chat_id": chat_id,
        "message_id": message_id,
        "reaction": [
            {
                "type": "emoji",
                "emoji": emoji
            }
        ]
    });

    if let Some(conn_id) = business_connection_id {
        if let Some(obj) = payload.as_object_mut() {
            obj.insert("business_connection_id".to_string(), serde_json::json!(conn_id));
        }
    }

    let res = client.post(&url)
        .json(&payload)
        .send()
        .await?;

    if !res.status().is_success() {
        let status = res.status();
        let err_text = res.text().await.unwrap_or_default();
        error!("[Telegram.business] Failed to set reaction: HTTP {status} - {err_text}");
        return Err(AppError::Telegram(format!("HTTP {status}: {err_text}")));
    }

    Ok(())
}
