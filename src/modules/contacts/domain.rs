use serde::{Deserialize, Serialize};
use crate::shared::types::ChatId;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Contact {
    pub chat_id: ChatId,
    pub first_name: Option<String>,
    pub address: Option<String>,
    pub phone_number: Option<String>,
    pub username: Option<String>,
    pub nickname: Option<String>,
}
