use std::env;
use std::fs;
use telegram_bot_seller::shared::{db, types::ProductId};
use telegram_bot_seller::modules::catalog;

fn setup_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("debug")
        .try_init();
}

#[tokio::test]
async fn test_catalog_crud_lifecycle() {
    setup_tracing();

    // Setup temporary database file
    let temp_dir = env::temp_dir();
    let db_name = format!(
        "test_catalog_crud_{}.db",
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

    // 2. Clear out any seeded products if they exist to start fresh
    let conn = pool.get().await.unwrap();
    conn.interact(|conn| -> Result<_, rusqlite::Error> {
        conn.execute("DELETE FROM catalog", [])?;
        Ok(())
    }).await.unwrap().unwrap();

    // 3. Test catalog is empty initially
    let initial_products = catalog::get_catalog(&pool).await.unwrap();
    assert!(initial_products.is_empty(), "Catalog should be empty after clearing");

    // 4. Create / Insert new products
    conn.interact(|conn| -> Result<_, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "INSERT INTO catalog (product_id, product_name, standard_price, stock_quantity, tags, notes, suitable_season, suitable_situation, duration, sillage) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )?;
        stmt.execute((
            1001i64,
            "Tom Ford Lost Cherry",
            18000i32,
            15i32,
            "сладкий, вишня, вечерний",
            "вишня, ликер, горький миндаль",
            "Зима",
            "Вечерний",
            "8 часов",
            "умеренно-сильный"
        ))?;
        stmt.execute((
            1002i64,
            "Creed Aventus",
            25000i32,
            5i32,
            "свежий, древесный, мужественный",
            "ананас, береза, мускус",
            "Лето",
            "Дневной",
            "6 часов",
            "умеренный"
        ))?;
        Ok(())
    }).await.unwrap().unwrap();

    // 5. Test retrieve catalog (Read)
    let products = catalog::get_catalog(&pool).await.unwrap();
    assert_eq!(products.len(), 2, "Catalog should contain 2 items");

    let tf = products.iter().find(|p| p.product_name == "Tom Ford Lost Cherry").expect("Tom Ford product not found");
    assert_eq!(tf.product_id, ProductId(1001));
    assert_eq!(tf.standard_price, 18000);
    assert_eq!(tf.stock_quantity, 15);
    assert_eq!(tf.tags.as_deref(), Some("сладкий, вишня, вечерний"));
    assert_eq!(tf.notes.as_deref(), Some("вишня, ликер, горький миндаль"));
    assert_eq!(tf.suitable_season.as_deref(), Some("Зима"));
    assert_eq!(tf.suitable_situation.as_deref(), Some("Вечерний"));
    assert_eq!(tf.duration.as_deref(), Some("8 часов"));
    assert_eq!(tf.sillage.as_deref(), Some("умеренно-сильный"));

    let creed = products.iter().find(|p| p.product_name == "Creed Aventus").expect("Creed product not found");
    assert_eq!(creed.product_id, ProductId(1002));
    assert_eq!(creed.stock_quantity, 5);

    // 6. Test update (Update)
    // Inline price & stock update
    conn.interact(|conn| -> Result<_, rusqlite::Error> {
        let mut stmt = conn.prepare("UPDATE catalog SET standard_price = ?, stock_quantity = ? WHERE product_id = ?")?;
        stmt.execute((19500i32, 12i32, 1001i64))?;
        Ok(())
    }).await.unwrap().unwrap();

    // Verify update
    let updated_tf = catalog::get_product(&pool, ProductId(1001)).await.unwrap().expect("Product not found");
    assert_eq!(updated_tf.standard_price, 19500);
    assert_eq!(updated_tf.stock_quantity, 12);

    // Test update_catalog_stock helper function
    catalog::update_catalog_stock(&pool, ProductId(1002), 8).await.unwrap();
    let updated_creed = catalog::get_product(&pool, ProductId(1002)).await.unwrap().expect("Product not found");
    assert_eq!(updated_creed.stock_quantity, 8);

    // 7. Test delete (Delete)
    conn.interact(|conn| -> Result<_, rusqlite::Error> {
        let mut stmt = conn.prepare("DELETE FROM catalog WHERE product_id = ?")?;
        stmt.execute([1002i64])?;
        Ok(())
    }).await.unwrap().unwrap();

    // Verify deletion
    let deleted_creed = catalog::get_product(&pool, ProductId(1002)).await.unwrap();
    assert!(deleted_creed.is_none(), "Creed Aventus should have been deleted");

    let final_products = catalog::get_catalog(&pool).await.unwrap();
    assert_eq!(final_products.len(), 1, "Only one product should remain");

    // Clean up temporary database file
    let _ = fs::remove_file(db_path);
}
