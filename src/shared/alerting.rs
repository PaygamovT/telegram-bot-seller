use crate::shared::error::AppResult;
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

        error!("[PANIC] {message} at {location}");
    }));
}

pub async fn send_alert(message: &str) -> AppResult<()> {
    warn!("[Alerting.send_alert] {message}");
    // TODO: Implement Telegram alert notification in Milestone 4 using ALERT_CONFIG
    Ok(())
}
