use crate::shared::error::{AppError, AppResult};
use std::sync::OnceLock;
use tracing::{error, info, warn};

struct AlertConfig {
    admin_chat_id: i64,
    telegram_token: String,
}

static ALERT_CONFIG: OnceLock<AlertConfig> = OnceLock::new();

pub fn install_panic_hook(admin_chat_id: i64, telegram_token: &str) {
    info!("[Alerting.install_panic_hook] Panic hook installed for chat_id={admin_chat_id}");

    let config = AlertConfig {
        admin_chat_id,
        telegram_token: telegram_token.to_string(),
    };
    if ALERT_CONFIG.set(config).is_err() {
        warn!("[Alerting.install_panic_hook] AlertConfig was already set");
    }

    std::panic::set_hook(Box::new(|panic_info| {
        let message = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            *s
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.as_str()
        } else {
            "Unknown panic"
        };

        let location = if let Some(loc) = panic_info.location() {
            format!("{}:{}:{}", loc.file(), loc.line(), loc.column())
        } else {
            "unknown location".to_string()
        };

        let formatted_msg = format!("🚨 *Application Panic!* 💥\n\n*Error*: {}\n*Location*: {}", message, location);
        error!("[PANIC] {message} at {location}");

        if let Some(config) = ALERT_CONFIG.get() {
            let token = config.telegram_token.clone();
            let chat_id = config.admin_chat_id;
            let formatted_msg_clone = formatted_msg.clone();
            
            // Send warning synchronously by blocking a single-threaded runtime on a dedicated thread
            let _ = std::thread::spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build();
                if let Ok(runtime) = rt {
                    let _ = runtime.block_on(async {
                        let client = reqwest::Client::builder()
                            .timeout(std::time::Duration::from_secs(15))
                            .build()
                            .unwrap_or_else(|_| reqwest::Client::new());
                        let url = format!("https://api.telegram.org/bot{token}/sendMessage");
                        let payload = serde_json::json!({
                            "chat_id": chat_id,
                            "text": formatted_msg_clone,
                            "parse_mode": "Markdown"
                        });
                        let _ = client.post(&url).json(&payload).send().await;
                    });
                }
            }).join();
        }
    }));
}

pub async fn send_alert(message: &str) -> AppResult<()> {
    warn!("[Alerting.send_alert] {message}");
    if let Some(config) = ALERT_CONFIG.get() {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        let url = format!("https://api.telegram.org/bot{}/sendMessage", config.telegram_token);
        let payload = serde_json::json!({
            "chat_id": config.admin_chat_id,
            "text": format!("🚨 *System Alert Warning*:\n\n{}", message),
            "parse_mode": "Markdown"
        });
        
        let res = client.post(&url).json(&payload).send().await?;
        if !res.status().is_success() {
            error!("[Alerting.send_alert] Failed to deliver alert to Telegram chat: HTTP {}", res.status());
        }
    }
    Ok(())
}

pub async fn send_with_retry(
    req_builder: reqwest::RequestBuilder,
    max_retries: usize,
) -> Result<reqwest::Response, reqwest::Error> {
    let mut attempts = 0;
    loop {
        attempts += 1;
        let builder = req_builder.try_clone()
            .expect("RequestBuilder should be cloneable (no stream body)");
            
        match builder.send().await {
            Ok(res) => {
                // Retry if it is a 5xx server error
                if res.status().is_server_error() && attempts < max_retries {
                    warn!("[HTTP.retry] Server error (HTTP {}). Retrying attempt {}/{}...", res.status(), attempts, max_retries);
                    tokio::time::sleep(std::time::Duration::from_millis(500 * attempts as u64)).await;
                    continue;
                }
                return Ok(res);
            }
            Err(err) => {
                if attempts >= max_retries {
                    error!("[HTTP.retry] HTTP request failed after {} attempts: {}", max_retries, err);
                    return Err(err);
                }
                warn!("[HTTP.retry] Request failed: {err}. Retrying attempt {}/{}...", attempts, max_retries);
                tokio::time::sleep(std::time::Duration::from_millis(500 * attempts as u64)).await;
            }
        }
    }
}
