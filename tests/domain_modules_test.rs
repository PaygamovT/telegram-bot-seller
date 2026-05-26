use std::env;
use telegram_bot_seller::shared::{db, seed};
use telegram_bot_seller::shared::types::{ChatId, ItemId, OrderId, ProductId};
use telegram_bot_seller::modules::{catalog, contacts, orders};
use tracing::debug;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

fn setup_tracing() {
    let _ = tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::new("debug"))
        .try_init();
}

#[tokio::test]
async fn test_domain_modules_end_to_end() {
    setup_tracing();
    debug!("[Test] Running: test_domain_modules_end_to_end");

    // 1. Setup temporary database
    let temp_dir = env::temp_dir();
    let db_name = format!(
        "test_domain_{}.db",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );
    let db_path = temp_dir.join(db_name);
    let db_path_str = db_path.to_str().unwrap();

    let pool = db::init(db_path_str).await.expect("Failed to init database");
    db::run_migrations(&pool).await.expect("Failed to run migrations");

    // 2. Test database seeding
    seed::seed_database(&pool).await.expect("Failed to seed database");

    // 3. Test Catalog Fetching (get_catalog, get_product)
    let catalog_items = catalog::get_catalog(&pool).await.expect("Failed to get catalog");
    assert_eq!(catalog_items.len(), 3, "Catalog should have 3 seeded products");

    let creed_id = ProductId(10001);
    let product_opt = catalog::get_product(&pool, creed_id).await.expect("Failed to get product");
    assert!(product_opt.is_some());
    let product = product_opt.unwrap();
    assert_eq!(product.product_name, "Creed Aventus");
    assert_eq!(product.standard_price, 15000);
    assert_eq!(product.stock_quantity, 50);

    // 4. Test Catalog stock update (update_catalog_stock)
    catalog::update_catalog_stock(&pool, creed_id, 45).await.expect("Failed to update stock");
    let product = catalog::get_product(&pool, creed_id).await.expect("Failed to get product").unwrap();
    assert_eq!(product.stock_quantity, 45);

    // 5. Test Contacts (get_contacts, update_contacts)
    let test_chat_id = ChatId(998877);
    let contact_opt = contacts::get_contacts(&pool, test_chat_id).await.expect("Failed to get contact");
    assert!(contact_opt.is_none());

    let new_contact = contacts::Contact {
        chat_id: test_chat_id,
        first_name: Some("John".to_string()),
        address: Some("123 Fragrance Lane".to_string()),
        phone_number: Some("+123456789".to_string()),
        username: Some("john_doe".to_string()),
        nickname: Some("Johnny".to_string()),
    };
    contacts::update_contacts(&pool, &new_contact).await.expect("Failed to insert contact");

    let contact = contacts::get_contacts(&pool, test_chat_id).await.expect("Failed to get contact").unwrap();
    assert_eq!(contact.first_name.as_deref(), Some("John"));
    assert_eq!(contact.address.as_deref(), Some("123 Fragrance Lane"));

    // Update address
    let mut updated_contact = new_contact.clone();
    updated_contact.address = Some("456 Sillage Blvd".to_string());
    contacts::update_contacts(&pool, &updated_contact).await.expect("Failed to update contact");

    let contact = contacts::get_contacts(&pool, test_chat_id).await.expect("Failed to get contact").unwrap();
    assert_eq!(contact.address.as_deref(), Some("456 Sillage Blvd"));

    // 6. Test Orders & Items (insert_order, insert_order_items, get_orders, get_order_items)
    let test_order_id = OrderId::generate();
    let new_order = orders::Order {
        order_id: test_order_id.clone(),
        chat_id: test_chat_id,
        status: orders::OrderStatus::Pending,
        delivery_address: Some("456 Sillage Blvd".to_string()),
        total_amount: 27000,
    };
    orders::insert_order(&pool, &new_order).await.expect("Failed to insert order");

    let order_item1 = orders::OrderItem {
        item_id: ItemId(1111),
        order_id: test_order_id.clone(),
        product_id: ProductId(10001), // Creed Aventus
        quantity: 1,
        price_at_sale: 15000,
    };
    let order_item2 = orders::OrderItem {
        item_id: ItemId(2222),
        order_id: test_order_id.clone(),
        product_id: ProductId(10002), // Bleu de Chanel
        quantity: 1,
        price_at_sale: 12000,
    };
    orders::insert_order_items(&pool, &[order_item1.clone(), order_item2.clone()]).await.expect("Failed to insert items");

    // Fetch order
    let user_orders = orders::get_orders(&pool, test_chat_id).await.expect("Failed to get orders");
    assert_eq!(user_orders.len(), 1);
    assert_eq!(user_orders[0].order_id, test_order_id);
    assert_eq!(user_orders[0].status, orders::OrderStatus::Pending);

    // Fetch items
    let order_items = orders::get_order_items(&pool, &test_order_id).await.expect("Failed to get order items");
    assert_eq!(order_items.len(), 2);
    assert!(order_items.iter().any(|item| item.product_id == ProductId(10001)));
    assert!(order_items.iter().any(|item| item.product_id == ProductId(10002)));

    // 7. Update order status & delivery details (update_order)
    orders::update_order(
        &pool,
        &test_order_id,
        orders::OrderStatus::Paid,
        Some("789 Premium Way".to_string()),
        30000,
    ).await.expect("Failed to update order");

    let user_orders = orders::get_orders(&pool, test_chat_id).await.expect("Failed to get orders");
    assert_eq!(user_orders[0].status, orders::OrderStatus::Paid);
    assert_eq!(user_orders[0].delivery_address.as_deref(), Some("789 Premium Way"));
    assert_eq!(user_orders[0].total_amount, 30000);

    // 8. Update line items (update_order_items)
    orders::update_order_items(&pool, ItemId(1111), 2, 14000).await.expect("Failed to update order item");
    let order_items = orders::get_order_items(&pool, &test_order_id).await.expect("Failed to get order items");
    let item1 = order_items.iter().find(|i| i.item_id == ItemId(1111)).unwrap();
    assert_eq!(item1.quantity, 2);
    assert_eq!(item1.price_at_sale, 14000);

    // Cleanup DB file
    let _ = std::fs::remove_file(db_path);
}
