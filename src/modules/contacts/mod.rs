mod domain;
mod repo;

pub use domain::Contact;

use crate::shared::db::DbPool;
use crate::shared::error::AppResult;
use crate::shared::types::ChatId;

/// Get contact info by Telegram Chat ID
pub async fn get_contacts(pool: &DbPool, chat_id: ChatId) -> AppResult<Option<Contact>> {
    repo::fetch_by_chat_id(pool, chat_id).await
}

/// Create or update contact information
pub async fn update_contacts(pool: &DbPool, contact: &Contact) -> AppResult<()> {
    repo::save_or_update(pool, contact).await
}
