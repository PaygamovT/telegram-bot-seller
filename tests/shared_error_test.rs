use telegram_bot_seller::shared::error::AppError;
use tracing::debug;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

fn setup_tracing() {
    let _ = tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::new("debug"))
        .try_init();
}

#[test]
fn test_app_error_display() {
    setup_tracing();
    debug!("[Test] Running: test_app_error_display");

    let db_err = AppError::Database(rusqlite::Error::QueryReturnedNoRows);
    assert_eq!(db_err.to_string(), "Database error: Query returned no rows");

    let cfg_err = AppError::Config("missing key".to_string());
    assert_eq!(cfg_err.to_string(), "Configuration error: missing key");

    let tg_err = AppError::Telegram("network timeout".to_string());
    assert_eq!(tg_err.to_string(), "Telegram API error: network timeout");

    let ai_err = AppError::AiApi("rate limited".to_string());
    assert_eq!(ai_err.to_string(), "AI API error: rate limited");

    let tool_err = AppError::UnknownTool("calculator".to_string());
    assert_eq!(tool_err.to_string(), "Unknown tool: calculator");

    let val_err = AppError::Validation("invalid email".to_string());
    assert_eq!(val_err.to_string(), "Validation error: invalid email");
}

#[test]
fn test_from_conversions() {
    setup_tracing();
    debug!("[Test] Running: test_from_conversions");

    // From rusqlite::Error
    let raw_db_err = rusqlite::Error::QueryReturnedNoRows;
    let converted_db_err: AppError = raw_db_err.into();
    match converted_db_err {
        AppError::Database(rusqlite::Error::QueryReturnedNoRows) => {}
        _ => panic!("Expected AppError::Database variant"),
    }

    // From std::io::Error
    let raw_io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let converted_io_err: AppError = raw_io_err.into();
    match converted_io_err {
        AppError::Io(err) => assert_eq!(err.to_string(), "file not found"),
        _ => panic!("Expected AppError::Io variant"),
    }
}
