use crate::shared::db::DbPool;
use crate::shared::error::{AppError, AppResult};
use crate::shared::types::{ChatId, ItemId, OrderId, ProductId};
use super::domain::{Order, OrderItem, OrderStatus};
use tracing::{debug, error};

pub async fn insert_order(pool: &DbPool, order: &Order) -> AppResult<()> {
    debug!(
        "[Orders.repo] Inserting new order: order_id={}, chat_id={}",
        order.order_id, order.chat_id
    );
    let conn = pool.get().await?;
    let order_clone = order.clone();
    conn
        .interact(move |conn| -> Result<_, rusqlite::Error> {
            let mut stmt = conn.prepare(
                "INSERT INTO orders (order_id, chat_id, status, delivery_address, total_amount) VALUES (?, ?, ?, ?, ?)"
            )?;
            stmt.execute((
                order_clone.order_id.0,
                order_clone.chat_id.0,
                order_clone.status.to_string(),
                order_clone.delivery_address,
                order_clone.total_amount,
            ))?;
            Ok(())
        })
        .await
        .map_err(|e| {
            error!("[Orders.repo] insert_order interact error: {e}");
            AppError::Database(rusqlite::Error::ToSqlConversionFailure(
                e.to_string().into(),
            ))
        })??;

    debug!("[Orders.repo] Order inserted successfully: order_id={}", order.order_id);
    Ok(())
}

pub async fn insert_order_items(pool: &DbPool, items: &[OrderItem]) -> AppResult<()> {
    if items.is_empty() {
        return Ok(());
    }
    debug!("[Orders.repo] Inserting {} order items", items.len());
    let conn = pool.get().await?;
    let items_clone = items.to_vec();
    conn
        .interact(move |conn| -> Result<_, rusqlite::Error> {
            let tx = conn.transaction()?;
            {
                let mut stmt = tx.prepare(
                    "INSERT INTO order_items (item_id, order_id, product_id, quantity, price_at_sale) VALUES (?, ?, ?, ?, ?)"
                )?;
                for item in items_clone {
                    stmt.execute((
                        item.item_id.0.to_string(),
                        item.order_id.0,
                        item.product_id.0,
                        item.quantity,
                        item.price_at_sale,
                    ))?;
                }
            }
            tx.commit()?;
            Ok(())
        })
        .await
        .map_err(|e| {
            error!("[Orders.repo] insert_order_items interact error: {e}");
            AppError::Database(rusqlite::Error::ToSqlConversionFailure(
                e.to_string().into(),
            ))
        })??;

    debug!("[Orders.repo] Order items inserted successfully");
    Ok(())
}

pub async fn fetch_orders_by_chat_id(pool: &DbPool, chat_id: ChatId) -> AppResult<Vec<Order>> {
    debug!("[Orders.repo] Fetching orders for chat_id: {}", chat_id);
    let conn = pool.get().await?;
    let orders = conn
        .interact(move |conn| -> Result<_, rusqlite::Error> {
            let mut stmt = conn.prepare(
                "SELECT order_id, chat_id, status, delivery_address, total_amount FROM orders WHERE chat_id = ?"
            )?;
            let order_iter = stmt.query_map([chat_id.0], |row| {
                let status_str: String = row.get(2)?;
                let status = status_str.parse().unwrap_or(OrderStatus::Pending);
                Ok(Order {
                    order_id: OrderId(row.get(0)?),
                    chat_id: ChatId(row.get(1)?),
                    status,
                    delivery_address: row.get(3)?,
                    total_amount: row.get(4)?,
                })
            })?;

            let mut list = Vec::new();
            for order in order_iter {
                list.push(order?);
            }
            Ok(list)
        })
        .await
        .map_err(|e| {
            error!("[Orders.repo] fetch_orders_by_chat_id interact error: {e}");
            AppError::Database(rusqlite::Error::ToSqlConversionFailure(
                e.to_string().into(),
            ))
        })??;

    Ok(orders)
}

pub async fn fetch_items_by_order_id(pool: &DbPool, order_id: &OrderId) -> AppResult<Vec<OrderItem>> {
    debug!("[Orders.repo] Fetching order items for order_id: {}", order_id);
    let conn = pool.get().await?;
    let order_id_clone = order_id.clone();
    let items = conn
        .interact(move |conn| -> Result<_, rusqlite::Error> {
            let mut stmt = conn.prepare(
                "SELECT item_id, order_id, product_id, quantity, price_at_sale FROM order_items WHERE order_id = ?"
            )?;
            let item_iter = stmt.query_map([order_id_clone.0], |row| {
                let item_id_str: String = row.get(0)?;
                let item_id = ItemId(item_id_str.parse().unwrap_or_default());
                Ok(OrderItem {
                    item_id,
                    order_id: OrderId(row.get(1)?),
                    product_id: ProductId(row.get(2)?),
                    quantity: row.get(3)?,
                    price_at_sale: row.get(4)?,
                })
            })?;

            let mut list = Vec::new();
            for item in item_iter {
                list.push(item?);
            }
            Ok(list)
        })
        .await
        .map_err(|e| {
            error!("[Orders.repo] fetch_items_by_order_id interact error: {e}");
            AppError::Database(rusqlite::Error::ToSqlConversionFailure(
                e.to_string().into(),
            ))
        })??;

    Ok(items)
}

pub async fn update_order(
    pool: &DbPool,
    order_id: &OrderId,
    status: OrderStatus,
    delivery_address: Option<String>,
    total_amount: i32,
) -> AppResult<()> {
    debug!("[Orders.repo] Updating order {}: status={:?}", order_id, status);
    let conn = pool.get().await?;
    let order_id_clone = order_id.clone();
    conn
        .interact(move |conn| -> Result<_, rusqlite::Error> {
            let mut stmt = conn.prepare(
                "UPDATE orders SET status = ?, delivery_address = ?, total_amount = ? WHERE order_id = ?"
            )?;
            let rows = stmt.execute((
                status.to_string(),
                delivery_address,
                total_amount,
                order_id_clone.0,
            ))?;
            if rows == 0 {
                return Err(rusqlite::Error::QueryReturnedNoRows);
            }
            Ok(())
        })
        .await
        .map_err(|e| {
            error!("[Orders.repo] update_order interact error: {e}");
            AppError::Database(rusqlite::Error::ToSqlConversionFailure(
                e.to_string().into(),
            ))
        })??;

    debug!("[Orders.repo] Order {} updated successfully", order_id);
    Ok(())
}

pub async fn update_order_item(
    pool: &DbPool,
    item_id: ItemId,
    quantity: i32,
    price_at_sale: i32,
) -> AppResult<()> {
    debug!("[Orders.repo] Updating order item {}: quantity={}, price={}", item_id, quantity, price_at_sale);
    let conn = pool.get().await?;
    conn
        .interact(move |conn| -> Result<_, rusqlite::Error> {
            let mut stmt = conn.prepare(
                "UPDATE order_items SET quantity = ?, price_at_sale = ? WHERE item_id = ?"
            )?;
            let rows = stmt.execute((quantity, price_at_sale, item_id.0.to_string()))?;
            if rows == 0 {
                return Err(rusqlite::Error::QueryReturnedNoRows);
            }
            Ok(())
        })
        .await
        .map_err(|e| {
            error!("[Orders.repo] update_order_item interact error: {e}");
            AppError::Database(rusqlite::Error::ToSqlConversionFailure(
                e.to_string().into(),
            ))
        })??;

    debug!("[Orders.repo] Order item {} updated successfully", item_id);
    Ok(())
}
