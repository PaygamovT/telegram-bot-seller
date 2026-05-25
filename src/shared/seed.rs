use crate::shared::db::DbPool;
use crate::shared::error::{AppError, AppResult};
use tracing::{info, debug, error};

pub async fn seed_database(pool: &DbPool) -> AppResult<()> {
    info!("[DB.seed] Checking if catalog database needs seeding...");
    let conn = pool.get().await?;

    let count: i64 = conn
        .interact(|conn| -> Result<_, rusqlite::Error> {
            let mut stmt = conn.prepare("SELECT COUNT(*) FROM catalog")?;
            stmt.query_row([], |row| row.get(0))
        })
        .await
        .map_err(|e| {
            error!("[DB.seed] Check error: {e}");
            AppError::Database(rusqlite::Error::ToSqlConversionFailure(
                e.to_string().into(),
            ))
        })??;

    if count > 0 {
        debug!("[DB.seed] Catalog is not empty ({} products found). Skipping seed.", count);
        return Ok(());
    }

    info!("[DB.seed] Catalog is empty. Populating with premium seed perfumes...");

    conn.interact(|conn| -> Result<_, rusqlite::Error> {
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO catalog (product_id, product_name, standard_price, stock_quantity, tags, notes, suitable_season, suitable_situation, duration, sillage) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            )?;

            // Seed 1: Creed Aventus
            stmt.execute((
                10001i64,
                "Creed Aventus",
                15000i32,
                50i32,
                Some("Luxury, Fruity, Woody"),
                Some("Pineapple, Bergamot, Birch, Patchouli, Musk"),
                Some("Spring, Summer, Autumn"),
                Some("Day, Evening, Business, Special Occasion"),
                Some("8-10 hours"),
                Some("Strong"),
            ))?;

            // Seed 2: Bleu de Chanel
            stmt.execute((
                10002i64,
                "Bleu de Chanel",
                12000i32,
                75i32,
                Some("Fresh, Citrus, Woody"),
                Some("Grapefruit, Mint, Incense, Ginger, Sandalwood"),
                Some("Any Season"),
                Some("Day, Evening, Date Night, Casual"),
                Some("6-8 hours"),
                Some("Moderate"),
            ))?;

            // Seed 3: Dior Sauvage
            stmt.execute((
                10003i64,
                "Dior Sauvage",
                11000i32,
                60i32,
                Some("Fresh, Spicy, Amber"),
                Some("Calabrian Bergamot, Sichuan Pepper, Lavender, Ambroxan"),
                Some("Spring, Summer, Autumn"),
                Some("Casual, Night Out, Date"),
                Some("8-10 hours"),
                Some("Heavy"),
            ))?;

            // Seed some default admin settings
            let mut settings_stmt = tx.prepare(
                "INSERT OR IGNORE INTO settings (key, value) VALUES (?, ?)"
            )?;
            settings_stmt.execute(("bot_status", "active"))?;
            settings_stmt.execute(("ai_mode", "autonomous"))?;

            // Seed a sample allowed agent media item
            let mut media_stmt = tx.prepare(
                "INSERT INTO agent_media (file_path, telegram_file_id, title, purpose, is_allowed_for_ai) VALUES (?, ?, ?, ?, ?)"
            )?;
            media_stmt.execute((
                "./assets/creed_aventus.jpg",
                Some("AgACAgIAAxkBAAM1ZH..."),
                "Creed Aventus Catalog Photo",
                "product_showcase",
                1i32,
            ))?;
        }
        tx.commit()?;
        Ok(())
    })
    .await
    .map_err(|e| {
        error!("[DB.seed] Seeding tx interact error: {e}");
        AppError::Database(rusqlite::Error::ToSqlConversionFailure(
            e.to_string().into(),
        ))
    })??;

    info!("[DB.seed] Seeding completed successfully!");
    Ok(())
}
