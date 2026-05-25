use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use tracing::debug;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChatId(pub i64);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OrderId(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProductId(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ItemId(pub i64);

// Display implementations
impl fmt::Display for ChatId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for OrderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for ProductId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for ItemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// From conversions
impl From<i64> for ChatId {
    fn from(val: i64) -> Self {
        Self(val)
    }
}

impl From<i64> for ProductId {
    fn from(val: i64) -> Self {
        Self(val)
    }
}

impl From<i64> for ItemId {
    fn from(val: i64) -> Self {
        Self(val)
    }
}

impl From<String> for OrderId {
    fn from(val: String) -> Self {
        Self(val)
    }
}

impl From<&str> for OrderId {
    fn from(val: &str) -> Self {
        Self(val.to_string())
    }
}

impl OrderId {
    pub fn generate() -> Self {
        let ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let id = format!("{}", ms);
        
        debug!("[OrderId.generate] Generated new order ID: {id}");
        
        Self(id)
    }
}
