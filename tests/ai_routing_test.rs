use telegram_bot_seller::modules::ai::deepseek::{OpenAiMessage, OpenAiRequest, OpenAiResponse};
use telegram_bot_seller::shared::{db, config::AppConfig};
use telegram_bot_seller::shared::types::ChatId;
use telegram_bot_seller::modules::ai::run_dialog;
use std::env;
use std::fs;

#[test]
fn test_deepseek_openai_request_serialization() {
    let msg = OpenAiMessage {
        role: "user".to_string(),
        content: Some("Привет, бот!".to_string()),
        tool_calls: None,
        tool_call_id: None,
        name: None,
    };

    let req = OpenAiRequest {
        model: "deepseek-chat".to_string(),
        messages: vec![msg],
        tools: None,
        tool_choice: None,
    };

    let serialized = serde_json::to_string(&req).expect("Failed to serialize OpenAiRequest");
    assert!(serialized.contains("\"model\":\"deepseek-chat\""));
    assert!(serialized.contains("\"role\":\"user\""));
    assert!(serialized.contains("\"content\":\"Привет, бот!\""));
}

#[test]
fn test_deepseek_openai_response_deserialization() {
    let mock_json = r#"
    {
        "choices": [
            {
                "message": {
                    "role": "assistant",
                    "content": "Здравствуйте! Чем могу помочь? [REACTION: 👍]"
                },
                "finish_reason": "stop"
            }
        ]
    }
    "#;

    let response: OpenAiResponse = serde_json::from_str(mock_json).expect("Failed to deserialize OpenAiResponse");
    let choice = response.choices.as_ref().unwrap().first().unwrap();
    assert_eq!(choice.finish_reason.as_deref(), Some("stop"));
    
    let msg = choice.message.as_ref().unwrap();
    assert_eq!(msg.role, "assistant");
    assert_eq!(msg.content.as_deref(), Some("Здравствуйте! Чем могу помочь? [REACTION: 👍]"));
}

#[tokio::test]
async fn test_ai_settings_database_loading() {
    // 1. Create temporary SQLite database
    let temp_dir = env::temp_dir();
    let db_name = format!(
        "test_ai_settings_{}.db",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );
    let db_path = temp_dir.join(db_name);
    let db_path_str = db_path.to_str().unwrap();

    let pool = db::init(db_path_str).await.expect("Failed to init database");
    db::run_migrations(&pool).await.expect("Failed to run migrations");

    let conn = pool.get().await.unwrap();

    // 2. Seed custom AI model preferences
    conn.interact(|conn| -> Result<(), rusqlite::Error> {
        let mut stmt = conn.prepare("INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)")?;
        stmt.execute(("primary_ai_model", "deepseek"))?;
        stmt.execute(("fallback_ai_model", "gemini"))?;
        Ok(())
    }).await.unwrap().unwrap();

    // 3. Verify SQLite settings are successfully read back
    let (primary, fallback) = conn.interact(|conn| -> Result<(String, String), rusqlite::Error> {
        let mut primary_stmt = conn.prepare("SELECT value FROM settings WHERE key = 'primary_ai_model'")?;
        let primary = primary_stmt
            .query_row([], |r| r.get::<_, String>(0))
            .unwrap_or_else(|_| "minimax".to_string());
            
        let mut fallback_stmt = conn.prepare("SELECT value FROM settings WHERE key = 'fallback_ai_model'")?;
        let fallback = fallback_stmt
            .query_row([], |r| r.get::<_, String>(0))
            .unwrap_or_else(|_| "deepseek".to_string());
            
        Ok((primary, fallback))
    }).await.unwrap().unwrap();

    assert_eq!(primary, "deepseek");
    assert_eq!(fallback, "gemini");

    // Clean up temporary files
    let _ = fs::remove_file(db_path);
}

#[tokio::test]
async fn test_orchestrator_failover_routing_on_error() {
    // Setup temporary database
    let temp_dir = env::temp_dir();
    let db_name = format!(
        "test_failover_{}.db",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );
    let db_path = temp_dir.join(db_name);
    let db_path_str = db_path.to_str().unwrap();

    let pool = db::init(db_path_str).await.expect("Failed to init database");
    db::run_migrations(&pool).await.expect("Failed to run migrations");

    let conn = pool.get().await.unwrap();

    // Configure primary to deepseek and fallback to none (so it throws straight away)
    conn.interact(|conn| -> Result<(), rusqlite::Error> {
        let mut stmt = conn.prepare("INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)")?;
        stmt.execute(("primary_ai_model", "deepseek"))?;
        stmt.execute(("fallback_ai_model", "none"))?;
        Ok(())
    }).await.unwrap().unwrap();

    // Verify it fails with missing key error because no keys are set in environment
    let client = reqwest::Client::new();
    let dummy_config = AppConfig {
        telegram_token: "dummy_tok".to_string(),
        admin_chat_id: 12345,
        minimax_api_key: "dummy_minimax".to_string(),
        minimax_group_id: "dummy_grp".to_string(),
        gemini_api_key: "dummy_gemini".to_string(),
        openrouter_api_key: None,
        deepseek_api_key: None,
        database_path: db_path_str.to_string(),
        admin_server_port: 8080,
        rust_log: "debug".to_string(),
    };

    let result = run_dialog(&client, &dummy_config, &pool, ChatId(9999), "Привет!").await;
    
    // It must return an Error since deepseek has no keys and fallback is 'none'
    assert!(result.is_err());
    let err_string = result.err().unwrap().to_string();
    assert!(err_string.contains("No DeepSeek API key") || err_string.contains("No API key"));

    // Clean up
    let _ = fs::remove_file(db_path);
}
