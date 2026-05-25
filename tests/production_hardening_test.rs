use std::env;
use std::fs;
use std::time::Duration;
use telegram_bot_seller::shared::rate_limiter::RateLimiter;
use telegram_bot_seller::shared::db;

fn setup_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("debug")
        .try_init();
}

#[tokio::test]
async fn test_rate_limiter_sliding_window_throttling() {
    setup_tracing();

    // 1. Create a RateLimiter allowing max 3 requests per 100 milliseconds
    let limiter = RateLimiter::new(3, Duration::from_millis(100));
    let user_id = 8888i64;

    // First 3 requests should succeed
    assert!(limiter.check(user_id));
    assert!(limiter.check(user_id));
    assert!(limiter.check(user_id));

    // 4th request within 100ms should be blocked
    assert!(!limiter.check(user_id), "4th request within window must be throttled");

    // Other user should not be affected
    assert!(limiter.check(9999i64), "Different user should not be throttled");

    // Wait 120ms to allow window to reset
    tokio::time::sleep(Duration::from_millis(120)).await;

    // Request should succeed again after window expiration
    assert!(limiter.check(user_id), "Request should pass after sliding window resets");
}

#[tokio::test]
async fn test_sqlite_vacuum_into_atomic_backup() {
    setup_tracing();

    // Setup temporary database directory
    let temp_dir = env::temp_dir();
    let db_name = format!(
        "test_bot_hardening_{}.db",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );
    let db_path = temp_dir.join(db_name);
    let db_path_str = db_path.to_str().unwrap();

    // 1. Initialize Pool and Migrations
    let pool = db::init(db_path_str).await.unwrap();
    db::run_migrations(&pool).await.unwrap();

    let conn = pool.get().await.unwrap();
    
    // Seed some mock contact
    conn.interact(|conn| {
        conn.execute(
            "INSERT INTO contacts (chat_id, first_name) VALUES (?, ?)",
            (7777i64, "BackupUser"),
        )
    }).await.unwrap().unwrap();

    // 2. Perform atomic backup using VACUUM INTO
    let backup_name = format!("backup_{}.db", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
    let backup_path = temp_dir.join(backup_name);
    let backup_path_str = backup_path.to_str().unwrap();

    let backup_path_clone = backup_path_str.to_string();
    conn.interact(move |conn| -> Result<(), rusqlite::Error> {
        conn.execute(&format!("VACUUM INTO '{}'", backup_path_clone), [])?;
        Ok(())
    }).await.unwrap().unwrap();

    // Verify backup file is written to disk
    assert!(backup_path.exists(), "SQLite backup file should exist on disk");

    // 3. Connect to backup DB and check seeded contact data is copied successfully
    let backup_pool = db::init(backup_path_str).await.unwrap();
    let backup_conn = backup_pool.get().await.unwrap();

    let contact_name: String = backup_conn.interact(|conn| {
        conn.query_row(
            "SELECT first_name FROM contacts WHERE chat_id = ?",
            [7777i64],
            |row| row.get(0),
        )
    }).await.unwrap().unwrap();

    assert_eq!(contact_name, "BackupUser", "Backup database should contain seeded records");

    // Clean up temporary database files
    let _ = fs::remove_file(db_path);
    let _ = fs::remove_file(backup_path);
}
