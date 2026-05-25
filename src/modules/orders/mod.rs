mod domain;
mod repo;

pub use domain::{Order, OrderItem, OrderStatus};

use crate::shared::db::DbPool;
use crate::shared::error::AppResult;
use crate::shared::types::{ChatId, ItemId, OrderId};

/// Insert a new order
pub async fn insert_order(pool: &DbPool, order: &Order) -> AppResult<()> {
    repo::insert_order(pool, order).await
}

/// Insert multiple items for an order
pub async fn insert_order_items(pool: &DbPool, items: &[OrderItem]) -> AppResult<()> {
    repo::insert_order_items(pool, items).await
}

/// Get all orders for a Telegram Chat ID
pub async fn get_orders(pool: &DbPool, chat_id: ChatId) -> AppResult<Vec<Order>> {
    repo::fetch_orders_by_chat_id(pool, chat_id).await
}

/// Get all items associated with a specific Order ID
pub async fn get_order_items(pool: &DbPool, order_id: &OrderId) -> AppResult<Vec<OrderItem>> {
    repo::fetch_items_by_order_id(pool, order_id).await
}

/// Update details of an order
pub async fn update_order(
    pool: &DbPool,
    order_id: &OrderId,
    status: OrderStatus,
    delivery_address: Option<String>,
    total_amount: i32,
) -> AppResult<()> {
    repo::update_order(pool, order_id, status, delivery_address, total_amount).await
}

/// Update details of a specific order item (line item)
pub async fn update_order_items(
    pool: &DbPool,
    item_id: ItemId,
    quantity: i32,
    price_at_sale: i32,
) -> AppResult<()> {
    repo::update_order_item(pool, item_id, quantity, price_at_sale).await
}
