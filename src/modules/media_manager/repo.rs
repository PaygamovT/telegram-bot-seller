use crate::shared::db::DbPool;
use crate::shared::error::{AppError, AppResult};
use super::domain::AgentMedia;
use tracing::{debug, error};

pub async fn fetch_all_allowed(pool: &DbPool) -> AppResult<Vec<AgentMedia>> {
    debug!("[MediaManager.repo] Fetching all allowed media for AI");
    let conn = pool.get().await?;
    let media = conn
        .interact(|conn| -> Result<_, rusqlite::Error> {
            let mut stmt = conn.prepare(
                "SELECT id, file_path, telegram_file_id, title, purpose, is_allowed_for_ai FROM agent_media WHERE is_allowed_for_ai = 1"
            )?;
            let media_iter = stmt.query_map([], |row| {
                Ok(AgentMedia {
                    id: Some(row.get(0)?),
                    file_path: row.get(1)?,
                    telegram_file_id: row.get(2)?,
                    title: row.get(3)?,
                    purpose: row.get(4)?,
                    is_allowed_for_ai: row.get(5)?,
                })
            })?;

            let mut list = Vec::new();
            for item in media_iter {
                list.push(item?);
            }
            Ok(list)
        })
        .await
        .map_err(|e| {
            error!("[MediaManager.repo] fetch_all_allowed interact error: {e}");
            AppError::Database(rusqlite::Error::ToSqlConversionFailure(
                e.to_string().into(),
            ))
        })??;

    Ok(media)
}

pub async fn insert_media(pool: &DbPool, media: &AgentMedia) -> AppResult<()> {
    debug!("[MediaManager.repo] Inserting agent media: title={}", media.title);
    let conn = pool.get().await?;
    let media_clone = media.clone();
    conn
        .interact(move |conn| -> Result<_, rusqlite::Error> {
            let mut stmt = conn.prepare(
                "INSERT INTO agent_media (file_path, telegram_file_id, title, purpose, is_allowed_for_ai) VALUES (?, ?, ?, ?, ?)"
            )?;
            stmt.execute((
                media_clone.file_path,
                media_clone.telegram_file_id,
                media_clone.title,
                media_clone.purpose,
                media_clone.is_allowed_for_ai,
            ))?;
            Ok(())
        })
        .await
        .map_err(|e| {
            error!("[MediaManager.repo] insert_media interact error: {e}");
            AppError::Database(rusqlite::Error::ToSqlConversionFailure(
                e.to_string().into(),
            ))
        })??;

    debug!("[MediaManager.repo] Media inserted successfully");
    Ok(())
}

pub async fn fetch_all(pool: &DbPool) -> AppResult<Vec<AgentMedia>> {
    debug!("[MediaManager.repo] Fetching all agent media files");
    let conn = pool.get().await?;
    let media = conn
        .interact(|conn| -> Result<_, rusqlite::Error> {
            let mut stmt = conn.prepare(
                "SELECT id, file_path, telegram_file_id, title, purpose, is_allowed_for_ai FROM agent_media"
            )?;
            let media_iter = stmt.query_map([], |row| {
                Ok(AgentMedia {
                    id: Some(row.get(0)?),
                    file_path: row.get(1)?,
                    telegram_file_id: row.get(2)?,
                    title: row.get(3)?,
                    purpose: row.get(4)?,
                    is_allowed_for_ai: row.get(5)?,
                })
            })?;

            let mut list = Vec::new();
            for item in media_iter {
                list.push(item?);
            }
            Ok(list)
        })
        .await
        .map_err(|e| {
            error!("[MediaManager.repo] fetch_all interact error: {e}");
            AppError::Database(rusqlite::Error::ToSqlConversionFailure(
                e.to_string().into(),
            ))
        })??;

    Ok(media)
}

pub async fn toggle_allowance(pool: &DbPool, id: i64) -> AppResult<()> {
    debug!("[MediaManager.repo] Toggling allowance status for media_id={}", id);
    let conn = pool.get().await?;
    conn
        .interact(move |conn| -> Result<_, rusqlite::Error> {
            conn.execute(
                "UPDATE agent_media SET is_allowed_for_ai = NOT is_allowed_for_ai WHERE id = ?",
                [id],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| {
            error!("[MediaManager.repo] toggle_allowance interact error: {e}");
            AppError::Database(rusqlite::Error::ToSqlConversionFailure(
                e.to_string().into(),
            ))
        })??;

    Ok(())
}

pub async fn delete_media(pool: &DbPool, id: i64) -> AppResult<String> {
    debug!("[MediaManager.repo] Deleting agent media record id={}", id);
    let conn = pool.get().await?;
    let file_path = conn
        .interact(move |conn| -> Result<String, rusqlite::Error> {
            let file_path: String = conn.query_row(
                "SELECT file_path FROM agent_media WHERE id = ?",
                [id],
                |row| row.get(0),
            )?;
            conn.execute("DELETE FROM agent_media WHERE id = ?", [id])?;
            Ok(file_path)
        })
        .await
        .map_err(|e| {
            error!("[MediaManager.repo] delete_media interact error: {e}");
            AppError::Database(rusqlite::Error::ToSqlConversionFailure(
                e.to_string().into(),
            ))
        })??;

    Ok(file_path)
}

