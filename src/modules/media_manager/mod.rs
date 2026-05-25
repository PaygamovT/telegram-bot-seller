mod domain;
mod repo;

pub use domain::AgentMedia;

use crate::shared::db::DbPool;
use crate::shared::error::AppResult;

/// Fetch all media files allowed for AI recommendations
pub async fn get_media(pool: &DbPool) -> AppResult<Vec<AgentMedia>> {
    repo::fetch_all_allowed(pool).await
}

/// Register a new media file in the database
pub async fn upload_media(pool: &DbPool, media: &AgentMedia) -> AppResult<()> {
    repo::insert_media(pool, media).await
}

/// Fetch all registered media files
pub async fn get_all_media(pool: &DbPool) -> AppResult<Vec<AgentMedia>> {
    repo::fetch_all(pool).await
}

/// Toggle permission setting for AI recommendation
pub async fn toggle_media_allowance(pool: &DbPool, id: i64) -> AppResult<()> {
    repo::toggle_allowance(pool, id).await
}

/// Remove a media file and return its path
pub async fn remove_media(pool: &DbPool, id: i64) -> AppResult<String> {
    repo::delete_media(pool, id).await
}

