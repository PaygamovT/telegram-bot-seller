use crate::shared::db::DbPool;
use crate::shared::error::{AppError, AppResult};
use crate::shared::types::ChatId;
use super::domain::Contact;
use tracing::{debug, error};

pub async fn fetch_by_chat_id(pool: &DbPool, chat_id: ChatId) -> AppResult<Option<Contact>> {
    debug!("[Contacts.repo] Fetching contact by chat_id: {}", chat_id);
    let conn = pool.get().await?;
    let contact = conn
        .interact(move |conn| -> Result<_, rusqlite::Error> {
            let mut stmt = conn.prepare(
                "SELECT chat_id, first_name, address, phone_number, username, nickname FROM contacts WHERE chat_id = ?"
            )?;
            let mut rows = stmt.query_and_then([chat_id.0], |row| {
                Ok(Contact {
                    chat_id: ChatId(row.get(0)?),
                    first_name: row.get(1)?,
                    address: row.get(2)?,
                    phone_number: row.get(3)?,
                    username: row.get(4)?,
                    nickname: row.get(5)?,
                })
            })?;

            match rows.next() {
                Some(Ok(c)) => Ok(Some(c)),
                Some(Err(e)) => Err(e),
                None => Ok(None),
            }
        })
        .await
        .map_err(|e| {
            error!("[Contacts.repo] fetch_by_chat_id interact error: {e}");
            AppError::Database(rusqlite::Error::ToSqlConversionFailure(
                e.to_string().into(),
            ))
        })??;

    Ok(contact)
}

pub async fn save_or_update(pool: &DbPool, contact: &Contact) -> AppResult<()> {
    debug!(
        "[Contacts.repo] Saving or updating contact: chat_id={}, first_name={:?}",
        contact.chat_id, contact.first_name
    );
    let conn = pool.get().await?;
    let contact_clone = contact.clone();
    conn
        .interact(move |conn| -> Result<_, rusqlite::Error> {
            let mut stmt = conn.prepare(
                "INSERT OR REPLACE INTO contacts (chat_id, first_name, address, phone_number, username, nickname) VALUES (?, ?, ?, ?, ?, ?)"
            )?;
            stmt.execute((
                contact_clone.chat_id.0,
                contact_clone.first_name,
                contact_clone.address,
                contact_clone.phone_number,
                contact_clone.username,
                contact_clone.nickname,
            ))?;
            Ok(())
        })
        .await
        .map_err(|e| {
            error!("[Contacts.repo] save_or_update interact error: {e}");
            AppError::Database(rusqlite::Error::ToSqlConversionFailure(
                e.to_string().into(),
            ))
        })??;

    debug!("[Contacts.repo] Contact saved successfully for chat_id={}", contact.chat_id);
    Ok(())
}
