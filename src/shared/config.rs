use std::env;
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub telegram_token: String,
    pub admin_chat_id: i64,
    pub minimax_api_key: String,
    pub minimax_group_id: String,
    pub gemini_api_key: String,
    pub openrouter_api_key: Option<String>,
    pub deepseek_api_key: Option<String>,
    pub database_path: String,
    pub admin_server_port: u16,
    pub rust_log: String,
}

impl AppConfig {
    pub fn load() -> anyhow::Result<Self> {
        debug!("[Config.load] Loading configuration from environment");

        // Skip loading .env file if we are running inside tests
        if env::var("TEST_ENV").is_err() {
            if let Err(err) = dotenvy::dotenv() {
                debug!("[Config.load] Note on .env loading: {}", err);
            }
        } else {
            debug!("[Config.load] Bypassing .env load in test environment");
        }

        let telegram_token = env::var("TELEGRAM_BOT_TOKEN")
            .map_err(|_| anyhow::anyhow!("TELEGRAM_BOT_TOKEN environment variable not set"))?;
        if telegram_token.trim().is_empty() {
            return Err(anyhow::anyhow!("TELEGRAM_BOT_TOKEN cannot be empty"));
        }

        let admin_chat_id_raw = env::var("ADMIN_CHAT_ID")
            .map_err(|_| anyhow::anyhow!("ADMIN_CHAT_ID environment variable not set"))?;
        let admin_chat_id = admin_chat_id_raw
            .trim()
            .parse::<i64>()
            .map_err(|_| anyhow::anyhow!("ADMIN_CHAT_ID must be a valid 64-bit integer"))?;

        let minimax_api_key = env::var("MINIMAX_API_KEY")
            .map_err(|_| anyhow::anyhow!("MINIMAX_API_KEY environment variable not set"))?;
        if minimax_api_key.trim().is_empty() {
            return Err(anyhow::anyhow!("MINIMAX_API_KEY cannot be empty"));
        }

        let minimax_group_id = env::var("MINIMAX_GROUP_ID")
            .map_err(|_| anyhow::anyhow!("MINIMAX_GROUP_ID environment variable not set"))?;
        if minimax_group_id.trim().is_empty() {
            return Err(anyhow::anyhow!("MINIMAX_GROUP_ID cannot be empty"));
        }

        let gemini_api_key = env::var("GEMINI_API_KEY")
            .map_err(|_| anyhow::anyhow!("GEMINI_API_KEY environment variable not set"))?;
        if gemini_api_key.trim().is_empty() {
            return Err(anyhow::anyhow!("GEMINI_API_KEY cannot be empty"));
        }

        let openrouter_api_key = env::var("OPENROUTER_API_KEY")
            .ok()
            .filter(|s| !s.trim().is_empty());
        if openrouter_api_key.is_none() {
            warn!("[Config.load] OPENROUTER_API_KEY not set, Gemini-only mode");
        }

        let deepseek_api_key = env::var("DEEPSEEK_API_KEY")
            .ok()
            .filter(|s| !s.trim().is_empty());

        let database_path =
            env::var("DATABASE_PATH").unwrap_or_else(|_| "./data/bot.db".to_string());
        if database_path.trim().is_empty() {
            return Err(anyhow::anyhow!("DATABASE_PATH cannot be empty"));
        }

        let admin_server_port_raw =
            env::var("ADMIN_SERVER_PORT").unwrap_or_else(|_| "8080".to_string());
        let admin_server_port = admin_server_port_raw.trim().parse::<u16>().map_err(|_| {
            anyhow::anyhow!("ADMIN_SERVER_PORT must be a valid 16-bit unsigned integer")
        })?;

        let rust_log = env::var("RUST_LOG").unwrap_or_else(|_| "debug".to_string());

        let config = Self {
            telegram_token,
            admin_chat_id,
            minimax_api_key,
            minimax_group_id,
            gemini_api_key,
            openrouter_api_key,
            deepseek_api_key,
            database_path,
            admin_server_port,
            rust_log,
        };

        // Log non-secret fields
        debug!("[Config.load] DATABASE_PATH={}", config.database_path);
        debug!("[Config.load] ADMIN_CHAT_ID={}", config.admin_chat_id);
        debug!(
            "[Config.load] ADMIN_SERVER_PORT={}",
            config.admin_server_port
        );
        debug!("[Config.load] RUST_LOG={}", config.rust_log);

        // NEVER log API keys or tokens, but log if they exist
        debug!("[Config.load] TELEGRAM_BOT_TOKEN set = true");
        debug!("[Config.load] MINIMAX_API_KEY set = true");
        debug!("[Config.load] MINIMAX_GROUP_ID set = true");
        debug!("[Config.load] GEMINI_API_KEY set = true");
        debug!(
            "[Config.load] OPENROUTER_API_KEY set = {}",
            config.openrouter_api_key.is_some()
        );
        debug!(
            "[Config.load] DEEPSEEK_API_KEY set = {}",
            config.deepseek_api_key.is_some()
        );

        info!("[Config.load] Configuration loaded successfully");

        Ok(config)
    }
}
