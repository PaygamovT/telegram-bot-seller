use std::env;
use std::sync::Mutex;
use telegram_bot_seller::shared::config::AppConfig;
use tracing::debug;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

static ENV_MUTEX: Mutex<()> = Mutex::new(());

fn setup_tracing() {
    let _ = tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::new("debug"))
        .try_init();
}

fn clear_env() {
    unsafe {
        env::set_var("TEST_ENV", "true");
        env::remove_var("TELEGRAM_BOT_TOKEN");
        env::remove_var("ADMIN_CHAT_ID");
        env::remove_var("MINIMAX_API_KEY");
        env::remove_var("MINIMAX_GROUP_ID");
        env::remove_var("GEMINI_API_KEY");
        env::remove_var("OPENROUTER_API_KEY");
        env::remove_var("DATABASE_PATH");
        env::remove_var("ADMIN_SERVER_PORT");
        env::remove_var("RUST_LOG");
    }
}

#[test]
fn test_config_load_success() {
    setup_tracing();
    debug!("[Test] Running: test_config_load_success");
    let _guard = ENV_MUTEX.lock().unwrap();
    clear_env();

    unsafe {
        env::set_var("TELEGRAM_BOT_TOKEN", "test_bot_token");
        env::set_var("ADMIN_CHAT_ID", "12345");
        env::set_var("MINIMAX_API_KEY", "test_minimax_key");
        env::set_var("MINIMAX_GROUP_ID", "test_minimax_group");
        env::set_var("GEMINI_API_KEY", "test_gemini_key");
        env::set_var("OPENROUTER_API_KEY", "test_openrouter_key");
        env::set_var("DATABASE_PATH", "./data/test.db");
        env::set_var("ADMIN_SERVER_PORT", "9090");
    }

    let config = AppConfig::load();
    assert!(config.is_ok());
    let config = config.unwrap();

    assert_eq!(config.telegram_token, "test_bot_token");
    assert_eq!(config.admin_chat_id, 12345);
    assert_eq!(config.minimax_api_key, "test_minimax_key");
    assert_eq!(config.minimax_group_id, "test_minimax_group");
    assert_eq!(config.gemini_api_key, "test_gemini_key");
    assert_eq!(config.openrouter_api_key, Some("test_openrouter_key".to_string()));
    assert_eq!(config.database_path, "./data/test.db");
    assert_eq!(config.admin_server_port, 9090);
}

#[test]
fn test_config_load_missing_required() {
    setup_tracing();
    debug!("[Test] Running: test_config_load_missing_required");
    let _guard = ENV_MUTEX.lock().unwrap();
    clear_env();

    // Set some, but omit TELEGRAM_BOT_TOKEN
    unsafe {
        env::set_var("ADMIN_CHAT_ID", "12345");
        env::set_var("MINIMAX_API_KEY", "test_minimax_key");
        env::set_var("MINIMAX_GROUP_ID", "test_minimax_group");
        env::set_var("GEMINI_API_KEY", "test_gemini_key");
    }

    let config = AppConfig::load();
    assert!(config.is_err());
    let err = config.err().unwrap().to_string();
    assert!(err.contains("TELEGRAM_BOT_TOKEN"));
}

#[test]
fn test_config_load_optional_missing() {
    setup_tracing();
    debug!("[Test] Running: test_config_load_optional_missing");
    let _guard = ENV_MUTEX.lock().unwrap();
    clear_env();

    unsafe {
        env::set_var("TELEGRAM_BOT_TOKEN", "test_bot_token");
        env::set_var("ADMIN_CHAT_ID", "12345");
        env::set_var("MINIMAX_API_KEY", "test_minimax_key");
        env::set_var("MINIMAX_GROUP_ID", "test_minimax_group");
        env::set_var("GEMINI_API_KEY", "test_gemini_key");
    }

    let config = AppConfig::load();
    assert!(config.is_ok());
    let config = config.unwrap();
    assert_eq!(config.openrouter_api_key, None);
}
