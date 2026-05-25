use std::env;
use telegram_bot_seller::shared::db;
use tracing::debug;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

fn setup_tracing() {
    let _ = tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::new("debug"))
        .try_init();
}

#[tokio::test]
async fn test_db_initialization_and_migrations() {
    setup_tracing();
    debug!("[Test] Running: test_db_initialization_and_migrations");

    // Use a unique database file in standard temp directory
    let temp_dir = env::temp_dir();
    let db_name = format!("test_bot_{}.db", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis());
    let db_path = temp_dir.join(db_name);
    let db_path_str = db_path.to_str().unwrap();

    // 1. Initialize Pool
    let pool = db::init(db_path_str).await;
    assert!(pool.is_ok(), "Database initialization failed: {:?}", pool.err());
    let pool = pool.unwrap();

    // Verify DB file is created
    assert!(db_path.exists(), "SQLite database file was not created at expected path");

    // 2. Run migrations
    let migration_res = db::run_migrations(&pool).await;
    assert!(migration_res.is_ok(), "Database migrations failed: {:?}", migration_res.err());

    // 3. Test basic INSERT + SELECT on contacts
    let conn = pool.get().await.unwrap();
    let insert_res = conn.interact(|conn| {
        conn.execute(
            "INSERT INTO contacts (chat_id, first_name, username) VALUES (?, ?, ?)",
            (12345678i64, "Alice", "alice_tg")
        )
    })
    .await
    .unwrap();
    assert_eq!(insert_res.unwrap(), 1);

    let contact_res: (i64, String, String) = conn.interact(|conn| {
        let mut stmt = conn.prepare("SELECT chat_id, first_name, username FROM contacts WHERE chat_id = ?").unwrap();
        stmt.query_row([12345678i64], |row| {
            Ok((row.get(0).unwrap(), row.get(1).unwrap(), row.get(2).unwrap()))
        })
    })
    .await
    .unwrap()
    .unwrap();

    assert_eq!(contact_res.0, 12345678);
    assert_eq!(contact_res.1, "Alice");
    assert_eq!(contact_res.2, "alice_tg");

    // 4. Test foreign key constraints (order_items references orders)
    // First, try inserting an order item without an order -> should fail because of foreign key constraint
    let fk_fail_res = conn.interact(|conn| {
        conn.execute(
            "INSERT INTO order_items (item_id, order_id, product_id, quantity, price_at_sale) VALUES (?, ?, ?, ?, ?)",
            ("item_1", "order_does_not_exist", 101, 2, 5000)
        )
    })
    .await
    .unwrap();
    assert!(fk_fail_res.is_err(), "Foreign key constraint on order_items should have failed");

    // Clean up DB file
    let _ = std::fs::remove_file(db_path);
}
