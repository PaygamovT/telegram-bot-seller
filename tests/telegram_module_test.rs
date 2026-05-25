use telegram_bot_seller::modules::telegram::business::{Update, UpdatesResponse, Message, Chat};
use tracing::debug;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

fn setup_tracing() {
    let _ = tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::new("debug"))
        .try_init();
}

#[test]
fn test_telegram_update_deserialization() {
    setup_tracing();
    debug!("[Test] Running: test_telegram_update_deserialization");

    // Mock JSON representing an incoming Telegram update
    let json_data = r#"
    {
        "update_id": 998877,
        "message": {
            "message_id": 54321,
            "chat": {
                "id": 112233,
                "first_name": "Alice",
                "username": "alice_test"
            },
            "date": 1779735400,
            "text": "Hello, bot!"
        }
    }
    "#;

    let update: Update = serde_json::from_str(json_data).expect("Failed to deserialize mock Update");
    assert_eq!(update.update_id, 998877);
    
    let message = update.message.expect("Message should not be empty");
    assert_eq!(message.message_id, 54321);
    assert_eq!(message.chat.id, 112233);
    assert_eq!(message.chat.first_name.as_deref(), Some("Alice"));
    assert_eq!(message.text.as_deref(), Some("Hello, bot!"));
}

#[test]
fn test_telegram_updates_list_deserialization() {
    setup_tracing();
    debug!("[Test] Running: test_telegram_updates_list_deserialization");

    let json_data = r#"
    {
        "ok": true,
        "result": [
            {
                "update_id": 1,
                "message": {
                    "message_id": 100,
                    "chat": {
                        "id": 200,
                        "first_name": "Bob"
                    },
                    "date": 1779735401,
                    "text": "Ping"
                }
            },
            {
                "update_id": 2,
                "business_message": {
                    "message_id": 101,
                    "chat": {
                        "id": 200,
                        "first_name": "Bob"
                    },
                    "date": 1779735402,
                    "text": "Pong"
                }
            }
        ]
    }
    "#;

    let response: UpdatesResponse = serde_json::from_str(json_data).expect("Failed to deserialize mock UpdatesResponse");
    assert!(response.ok);
    assert_eq!(response.result.len(), 2);
    
    // Check first (regular) message
    let update1 = &response.result[0];
    assert_eq!(update1.update_id, 1);
    assert!(update1.message.is_some());
    assert_eq!(update1.message.as_ref().unwrap().text.as_deref(), Some("Ping"));

    // Check second (business) message
    let update2 = &response.result[1];
    assert_eq!(update2.update_id, 2);
    assert!(update2.business_message.is_some());
    assert_eq!(update2.business_message.as_ref().unwrap().text.as_deref(), Some("Pong"));
}
