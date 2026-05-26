use crate::shared::config::AppConfig;
use crate::shared::db::DbPool;
use crate::shared::error::{AppError, AppResult};
use crate::shared::types::ChatId;
use tracing::{info, warn, error, debug};

pub mod gemini;
pub mod minimax;
pub mod deepseek;
pub mod tools;

pub use gemini::{analyze_image, transcribe_voice};

/// Dynamic dialogue runner that queries SQLite settings for Primary & Fallback model selections,
/// executes the primary model, and automatically failovers to the fallback model in case of errors.
pub async fn run_dialog(
    client: &reqwest::Client,
    config: &AppConfig,
    pool: &DbPool,
    chat_id: ChatId,
    user_text: &str,
) -> AppResult<(String, Option<String>)> {
    // 1. Fetch AI Model settings dynamically from database settings table
    let models = {
        let conn = pool.get().await?;
        conn.interact(|conn| -> Result<(String, String), rusqlite::Error> {
            let mut primary_stmt = conn.prepare("SELECT value FROM settings WHERE key = 'primary_ai_model'")?;
            let primary = primary_stmt
                .query_row([], |r| r.get::<_, String>(0))
                .unwrap_or_else(|_| "minimax".to_string());
                
            let mut fallback_stmt = conn.prepare("SELECT value FROM settings WHERE key = 'fallback_ai_model'")?;
            let fallback = fallback_stmt
                .query_row([], |r| r.get::<_, String>(0))
                .unwrap_or_else(|_| "deepseek".to_string());
                
            Ok((primary, fallback))
        }).await
        .map_err(|e| AppError::Database(rusqlite::Error::ToSqlConversionFailure(e.to_string().into())))?
        .unwrap_or_else(|_| ("minimax".to_string(), "deepseek".to_string()))
    };

    let (primary_model, fallback_model) = models;
    info!("[AI.Router] Dialogue turn initialized. Primary Model: {primary_model}, Fallback Model: {fallback_model}");

    // 2. Execute Primary Model dialogue loop
    let primary_result = match primary_model.as_str() {
        "deepseek" => deepseek::run_dialog(client, config, pool, chat_id, user_text).await,
        "gemini" => {
            match gemini::run_text_dialog(client, config, user_text).await {
                Ok(text) => {
                    let tag = "[REACTION:";
                    if let Some(start_idx) = text.find(tag) {
                        if let Some(end_idx) = text[start_idx..].find(']') {
                            let emoji = text[start_idx + tag.len()..start_idx + end_idx].trim().to_string();
                            let before = &text[..start_idx];
                            let after = &text[start_idx + end_idx + 1..];
                            let cleaned = format!("{}{}", before, after).trim().to_string();
                            Ok((cleaned, Some(emoji)))
                        } else {
                            Ok((text, None))
                        }
                    } else {
                        Ok((text, None))
                    }
                }
                Err(err) => Err(err),
            }
        }
        _ => minimax::run_dialog(client, config, pool, chat_id, user_text).await,
    };

    match primary_result {
        Ok(res) => Ok(res),
        Err(primary_err) => {
            let err_msg = format!("Primary Model '{primary_model}' dialog failed: {primary_err}");
            warn!("[AI.Router] {err_msg}. Triggering fallback orchestration...");
            let _ = crate::shared::alerting::send_alert(&err_msg).await;

            // 3. Fallback Failover Routing
            if fallback_model == "none" || fallback_model == primary_model {
                debug!("[AI.Router] Fallback model is disabled ('none') or identical to primary. Aborting failover.");
                return Err(primary_err);
            }

            info!("[AI.Router] Routing dialogue failover to: {fallback_model}");
            let fallback_result = match fallback_model.as_str() {
                "deepseek" => deepseek::run_dialog(client, config, pool, chat_id, user_text).await,
                "gemini" => {
                    match gemini::run_text_dialog(client, config, user_text).await {
                        Ok(text) => {
                            let tag = "[REACTION:";
                            if let Some(start_idx) = text.find(tag) {
                                if let Some(end_idx) = text[start_idx..].find(']') {
                                    let emoji = text[start_idx + tag.len()..start_idx + end_idx].trim().to_string();
                                    let before = &text[..start_idx];
                                    let after = &text[start_idx + end_idx + 1..];
                                    let cleaned = format!("{}{}", before, after).trim().to_string();
                                    Ok((cleaned, Some(emoji)))
                                } else {
                                    Ok((text, None))
                                }
                            } else {
                                Ok((text, None))
                            }
                        }
                        Err(err) => Err(err),
                    }
                }
                _ => Err(AppError::AiApi(format!("Unknown fallback model configured: {fallback_model}"))),
            };

            match fallback_result {
                Ok(fallback_res) => {
                    info!("[AI.Router] Transparent dialogue failover to '{fallback_model}' succeeded! 🎉");
                    Ok(fallback_res)
                }
                Err(fallback_err) => {
                    error!("[AI.Router] Fallback model '{fallback_model}' also failed: {fallback_err}");
                    Err(fallback_err)
                }
            }
        }
    }
}
