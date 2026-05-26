use crate::shared::db::DbPool;
use crate::shared::config::AppConfig;
use crate::shared::error::{AppError, AppResult};
use super::business::{UpdatesResponse, send_message};
use super::handlers::handle_message_update;
use reqwest::Client;
use std::time::Duration;
use tracing::{info, warn, error, debug};

pub async fn run(pool: DbPool, config: AppConfig) -> AppResult<()> {
    info!("🤖 Starting Telegram Bot polling loop...");

    let client = Client::builder()
        .timeout(Duration::from_secs(45))
        .build()?;

    let mut offset = 0i64;

    loop {
        // Load the dynamic config on every iteration
        let active_config = config.load_dynamic(&pool).await;
        let token = active_config.telegram_token.trim();

        // Check if the token is empty or dummy
        if token.is_empty() || token == "123456789:ABCdefGhIJKlmNoPQRsTUVwxyZ" {
            warn!(
                "⚠️ TELEGRAM_BOT_TOKEN is not configured or uses a dummy placeholder! Polling will enter standby mode (checking again in 10s)..."
            );
            tokio::time::sleep(Duration::from_secs(10)).await;
            continue;
        }

        debug!("[Telegram.bot] Long-polling for updates (offset: {offset})...");
        let url = format!(
            "https://api.telegram.org/bot{token}/getUpdates?offset={offset}&timeout=30&allowed_updates=[\"message\",\"business_message\"]"
        );

        let response = match client.get(&url).send().await {
            Ok(res) => res,
            Err(err) => {
                error!("[Telegram.bot] Connectivity error during getUpdates: {err}");
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        if !response.status().is_success() {
            let status = response.status();
            error!("[Telegram.bot] getUpdates returned HTTP {status}");
            tokio::time::sleep(Duration::from_secs(10)).await;
            continue;
        }

        let updates_resp: UpdatesResponse = match response.json().await {
            Ok(resp) => resp,
            Err(err) => {
                error!("[Telegram.bot] Failed to parse JSON updates response: {err}");
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        if !updates_resp.ok {
            error!("[Telegram.bot] getUpdates returned ok: false response");
            tokio::time::sleep(Duration::from_secs(10)).await;
            continue;
        }

        for update in &updates_resp.result {
            debug!("[Telegram.bot] Received update: {}", update.update_id);
            offset = update.update_id + 1;

            // Extract message or business_message
            let message_opt = update.message.as_ref().or(update.business_message.as_ref());
            if let Some(message) = message_opt {
                let client_clone = client.clone();
                let pool_clone = pool.clone();
                let config_clone = active_config.clone();
                let msg_clone = message.clone();

                // Process update asynchronously to prevent blocking the poll loop
                tokio::spawn(async move {
                    if let Err(err) = handle_message_update(&client_clone, &pool_clone, &config_clone, &msg_clone).await {
                        error!("[Telegram.bot] Error handling message {}: {:?}", msg_clone.message_id, err);
                        
                        // Attempt to send error reply back
                        let _ = send_message(
                            &client_clone,
                            &config_clone.telegram_token,
                            msg_clone.chat.id,
                            "⚠️ An error occurred while processing your message.",
                            msg_clone.business_connection_id.as_deref(),
                        ).await;
                    }
                });
            }
        }
    }
}
