use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentMedia {
    pub id: Option<i64>,
    pub file_path: String,
    pub telegram_file_id: Option<String>,
    pub title: String,
    pub purpose: String,
    pub is_allowed_for_ai: bool,
}
