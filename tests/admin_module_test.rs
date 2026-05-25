use std::env;
use axum::{
    extract::{FromRequestParts, Query, State},
    http::{header, Request, StatusCode},
    response::IntoResponse,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};

use telegram_bot_seller::shared::{config::AppConfig, db};
use telegram_bot_seller::modules::admin::server::{BasicAuth, RecentOrder};

fn setup_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("debug")
        .try_init();
}

#[tokio::test]
async fn test_basic_auth_extractor_missing_credentials() {
    let req = Request::builder()
        .uri("http://localhost:8080/")
        .body(())
        .unwrap();
    let (mut parts, _) = req.into_parts();

    let auth_res = BasicAuth::from_request_parts(&mut parts, &()).await;
    assert!(auth_res.is_err(), "Authentication should fail when Authorization header is missing");
    
    let resp = auth_res.unwrap_err();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    
    let www_auth = resp.headers().get(header::WWW_AUTHENTICATE);
    assert!(www_auth.is_some());
    assert_eq!(www_auth.unwrap(), "Basic realm=\"Admin Panel\", charset=\"UTF-8\"");
}

#[tokio::test]
async fn test_basic_auth_extractor_invalid_credentials() {
    // Encoding "admin:wrongpassword"
    let encoded = STANDARD.encode("admin:wrongpassword");
    let req = Request::builder()
        .uri("http://localhost:8080/")
        .header(header::AUTHORIZATION, format!("Basic {}", encoded))
        .body(())
        .unwrap();
    let (mut parts, _) = req.into_parts();

    let auth_res = BasicAuth::from_request_parts(&mut parts, &()).await;
    assert!(auth_res.is_err(), "Authentication should fail with incorrect password");
    
    let resp = auth_res.unwrap_err();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_basic_auth_extractor_correct_credentials() {
    // Encoding "admin:admin123"
    let encoded = STANDARD.encode("admin:admin123");
    let req = Request::builder()
        .uri("http://localhost:8080/")
        .header(header::AUTHORIZATION, format!("Basic {}", encoded))
        .body(())
        .unwrap();
    let (mut parts, _) = req.into_parts();

    let auth_res = BasicAuth::from_request_parts(&mut parts, &()).await;
    assert!(auth_res.is_ok(), "Authentication should succeed with correct credentials");
}

#[tokio::test]
async fn test_admin_metrics_queries_and_revenue_math() {
    setup_tracing();
    
    // Create temporary database file
    let temp_dir = env::temp_dir();
    let db_name = format!(
        "test_admin_metrics_{}.db",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );
    let db_path = temp_dir.join(db_name);
    let db_path_str = db_path.to_str().unwrap();

    // 1. Init DB and migrations
    let pool = db::init(db_path_str).await.unwrap();
    db::run_migrations(&pool).await.unwrap();

    let conn = pool.get().await.unwrap();

    // 2. Seed mock contact and multiple orders
    conn.interact(|conn| -> Result<_, rusqlite::Error> {
        // Seed contact
        conn.execute(
            "INSERT INTO contacts (chat_id, first_name, username) VALUES (?, ?, ?)",
            (9999i64, "Constantine", "const_tg"),
        )?;

        // Seed 3 orders: 2 paid, 1 pending
        let mut stmt = conn.prepare(
            "INSERT INTO orders (order_id, chat_id, status, delivery_address, total_amount) VALUES (?, ?, ?, ?, ?)"
        )?;
        
        stmt.execute(("order_1", 9999i64, "paid", "Nevsky Ave, 12", 15000i64))?;
        stmt.execute(("order_2", 9999i64, "pending", "Nevsky Ave, 12", 12000i64))?;
        stmt.execute(("order_3", 9999i64, "paid", "Arbat St, 5", 8500i64))?;

        Ok(())
    }).await.unwrap().unwrap();

    // 3. Test stats loading and verification
    let stats_res = conn.interact(|conn| -> Result<_, rusqlite::Error> {
        // Count total orders
        let mut cnt_stmt = conn.prepare("SELECT COUNT(*) FROM orders")?;
        let total_orders_i64: i64 = cnt_stmt.query_row([], |r| r.get(0))?;
        let total_orders = total_orders_i64 as usize;

        // Sum paid revenue
        let mut rev_stmt = conn.prepare("SELECT COALESCE(SUM(total_amount), 0) FROM orders WHERE status = 'paid'")?;
        let total_revenue: i64 = rev_stmt.query_row([], |r| r.get(0))?;

        // Load top 10 orders
        let mut recent_stmt = conn.prepare(
            "SELECT o.order_id, COALESCE(c.first_name, 'Unknown'), COALESCE(c.username, ''), o.status, o.total_amount \
             FROM orders o \
             LEFT JOIN contacts c ON o.chat_id = c.chat_id \
             ORDER BY o.ROWID DESC \
             LIMIT 10"
        )?;

        let recent_orders = recent_stmt.query_map([], |row| {
            Ok(RecentOrder {
                order_id: row.get(0)?,
                customer_name: row.get(1)?,
                username: row.get(2)?,
                status: row.get(3)?,
                total_amount: row.get(4)?,
            })
        })?.collect::<Result<Vec<RecentOrder>, rusqlite::Error>>()?;

        Ok((total_orders, total_revenue, recent_orders))
    }).await.unwrap().unwrap();

    let (total_orders, total_revenue, recent_orders) = stats_res;

    // Total orders should be exactly 3
    assert_eq!(total_orders, 3);
    
    // Total revenue should ONLY sum paid orders (15000 + 8500 = 23500)
    assert_eq!(total_revenue, 23500);

    // Recent orders size should be 3
    assert_eq!(recent_orders.len(), 3);
    
    // The order should be ROWID descending (order_3, order_2, order_1)
    assert_eq!(recent_orders[0].order_id, "order_3");
    assert_eq!(recent_orders[0].customer_name, "Constantine");
    assert_eq!(recent_orders[0].username, "const_tg");
    assert_eq!(recent_orders[0].total_amount, 8500);
    assert_eq!(recent_orders[0].status, "paid");

    assert_eq!(recent_orders[1].order_id, "order_2");
    assert_eq!(recent_orders[2].order_id, "order_1");

    // Clean up temporary database file
    let _ = std::fs::remove_file(db_path);
}
