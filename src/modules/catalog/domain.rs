use serde::{Deserialize, Serialize};
use crate::shared::types::ProductId;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Product {
    pub product_id: ProductId,
    pub product_name: String,
    pub standard_price: i32,
    pub stock_quantity: i32,
    pub tags: Option<String>,
    pub notes: Option<String>,
    pub suitable_season: Option<String>,
    pub suitable_situation: Option<String>,
    pub duration: Option<String>,
    pub sillage: Option<String>,
}
