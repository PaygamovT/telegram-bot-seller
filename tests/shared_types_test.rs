use telegram_bot_seller::shared::types::{ChatId, ItemId, OrderId, ProductId};
use tracing::debug;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

fn setup_tracing() {
    let _ = tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::new("debug"))
        .try_init();
}

#[test]
fn test_chat_id_display_and_creation() {
    setup_tracing();
    debug!("[Test] Running: test_chat_id_display_and_creation");
    let chat_id = ChatId::from(123456789i64);
    assert_eq!(chat_id.0, 123456789);
    assert_eq!(format!("{chat_id}"), "123456789");
}

#[test]
fn test_product_id_display_and_creation() {
    setup_tracing();
    debug!("[Test] Running: test_product_id_display_and_creation");
    let product_id = ProductId::from(100500i64);
    assert_eq!(product_id.0, 100500);
    assert_eq!(format!("{product_id}"), "100500");
}

#[test]
fn test_item_id_display_and_creation() {
    setup_tracing();
    debug!("[Test] Running: test_item_id_display_and_creation");
    let item_id = ItemId::from(8888i64);
    assert_eq!(item_id.0, 8888);
    assert_eq!(format!("{item_id}"), "8888");
}

#[test]
fn test_order_id_generation_and_uniqueness() {
    setup_tracing();
    debug!("[Test] Running: test_order_id_generation_and_uniqueness");
    let id1 = OrderId::generate();
    // Sleep 2 milliseconds to guarantee distinct timestamp values
    std::thread::sleep(std::time::Duration::from_millis(2));
    let id2 = OrderId::generate();
    
    assert_ne!(id1.0, id2.0);
    assert!(!id1.0.is_empty());
    
    // The format should be numeric since it comes from timestamp millis
    assert!(id1.0.chars().all(|c| c.is_ascii_digit()));
}

#[test]
fn test_types_serde_roundtrip() {
    setup_tracing();
    debug!("[Test] Running: test_types_serde_roundtrip");
    
    let original_chat = ChatId::from(9999);
    let serialized_chat = serde_json::to_string(&original_chat).unwrap();
    let deserialized_chat: ChatId = serde_json::from_str(&serialized_chat).unwrap();
    assert_eq!(original_chat, deserialized_chat);

    let original_order = OrderId::from("1778900618944");
    let serialized_order = serde_json::to_string(&original_order).unwrap();
    let deserialized_order: OrderId = serde_json::from_str(&serialized_order).unwrap();
    assert_eq!(original_order, deserialized_order);
}
