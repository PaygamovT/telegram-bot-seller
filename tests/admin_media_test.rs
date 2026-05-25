use std::env;
use std::fs;
use base64::{Engine as _, engine::general_purpose::STANDARD};

use telegram_bot_seller::shared::db;
use telegram_bot_seller::modules::media_manager::{self, AgentMedia};

fn setup_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("debug")
        .try_init();
}

#[tokio::test]
async fn test_media_crud_actions_and_allowance_toggling() {
    setup_tracing();

    // Setup temporary database file
    let temp_dir = env::temp_dir();
    let db_name = format!(
        "test_admin_media_{}.db",
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

    // 2. Perform mock uploads and inserts
    let media1 = AgentMedia {
        id: None,
        file_path: "./data/media/1001_creed.jpg".to_string(),
        telegram_file_id: Some("tg_file_111".to_string()),
        title: "Creed Aventus Promo".to_string(),
        purpose: "product_showcase".to_string(),
        is_allowed_for_ai: true,
    };

    let media2 = AgentMedia {
        id: None,
        file_path: "./data/media/1002_chanel.jpg".to_string(),
        telegram_file_id: None,
        title: "Bleu de Chanel Receipt".to_string(),
        purpose: "receipt_verification".to_string(),
        is_allowed_for_ai: false, // Disallowed for AI by default
    };

    media_manager::upload_media(&pool, &media1).await.unwrap();
    media_manager::upload_media(&pool, &media2).await.unwrap();

    // 3. Test get_all_media retrieves ALL records (not just allowed ones)
    let all_media = media_manager::get_all_media(&pool).await.unwrap();
    assert_eq!(all_media.len(), 2, "Media catalog should contain exactly two elements");

    // Order elements by ID logic
    let item1 = all_media.iter().find(|m| m.title == "Creed Aventus Promo").unwrap();
    let item2 = all_media.iter().find(|m| m.title == "Bleu de Chanel Receipt").unwrap();

    assert_eq!(item1.is_allowed_for_ai, true);
    assert_eq!(item2.is_allowed_for_ai, false);

    // 4. Test toggle_media_allowance switches AI permission status
    let id_to_toggle = item2.id.expect("Media ID should be loaded");
    media_manager::toggle_media_allowance(&pool, id_to_toggle).await.unwrap();

    // Reload and check toggled status
    let updated_media = media_manager::get_all_media(&pool).await.unwrap();
    let updated_item2 = updated_media.iter().find(|m| m.id == Some(id_to_toggle)).unwrap();
    assert_eq!(updated_item2.is_allowed_for_ai, true, "Allowance status should be toggled to true");

    // 5. Test remove_media drops row and returns correct file path
    let id_to_delete = item1.id.expect("Media ID should be loaded");
    let returned_path = media_manager::remove_media(&pool, id_to_delete).await.unwrap();
    assert_eq!(returned_path, "./data/media/1001_creed.jpg");

    // Reload and check database count decreases
    let final_media = media_manager::get_all_media(&pool).await.unwrap();
    assert_eq!(final_media.len(), 1, "Media catalog should contain only one element after deletion");
    assert!(final_media.iter().all(|m| m.id != Some(id_to_delete)));

    // Clean up temporary database file
    let _ = fs::remove_file(db_path);
}
