use crate::shared::db::DbPool;
use crate::shared::error::{AppError, AppResult};
use crate::shared::types::ProductId;
use super::domain::Product;
use tracing::{debug, error};

pub async fn fetch_all(pool: &DbPool) -> AppResult<Vec<Product>> {
    debug!("[Catalog.repo] Fetching all products from catalog");
    let conn = pool.get().await?;
    let products = conn
        .interact(|conn| -> Result<_, rusqlite::Error> {
            let mut stmt = conn.prepare(
                "SELECT product_id, product_name, standard_price, stock_quantity, tags, notes, suitable_season, suitable_situation, duration, sillage FROM catalog"
            )?;
            let product_iter = stmt.query_map([], |row| {
                Ok(Product {
                    product_id: ProductId(row.get(0)?),
                    product_name: row.get(1)?,
                    standard_price: row.get(2)?,
                    stock_quantity: row.get(3)?,
                    tags: row.get(4)?,
                    notes: row.get(5)?,
                    suitable_season: row.get(6)?,
                    suitable_situation: row.get(7)?,
                    duration: row.get(8)?,
                    sillage: row.get(9)?,
                })
            })?;

            let mut list = Vec::new();
            for product in product_iter {
                list.push(product?);
            }
            Ok(list)
        })
        .await
        .map_err(|e| {
            error!("[Catalog.repo] fetch_all interact error: {e}");
            AppError::Database(rusqlite::Error::ToSqlConversionFailure(
                e.to_string().into(),
            ))
        })??;

    debug!("[Catalog.repo] Successfully fetched {} products", products.len());
    Ok(products)
}

pub async fn fetch_by_id(pool: &DbPool, product_id: ProductId) -> AppResult<Option<Product>> {
    debug!("[Catalog.repo] Fetching product by ID: {}", product_id);
    let conn = pool.get().await?;
    let product = conn
        .interact(move |conn| -> Result<_, rusqlite::Error> {
            let mut stmt = conn.prepare(
                "SELECT product_id, product_name, standard_price, stock_quantity, tags, notes, suitable_season, suitable_situation, duration, sillage FROM catalog WHERE product_id = ?"
            )?;
            let mut rows = stmt.query_and_then([product_id.0], |row| {
                Ok(Product {
                    product_id: ProductId(row.get(0)?),
                    product_name: row.get(1)?,
                    standard_price: row.get(2)?,
                    stock_quantity: row.get(3)?,
                    tags: row.get(4)?,
                    notes: row.get(5)?,
                    suitable_season: row.get(6)?,
                    suitable_situation: row.get(7)?,
                    duration: row.get(8)?,
                    sillage: row.get(9)?,
                })
            })?;

            match rows.next() {
                Some(Ok(p)) => Ok(Some(p)),
                Some(Err(e)) => Err(e),
                None => Ok(None),
            }
        })
        .await
        .map_err(|e| {
            error!("[Catalog.repo] fetch_by_id interact error: {e}");
            AppError::Database(rusqlite::Error::ToSqlConversionFailure(
                e.to_string().into(),
            ))
        })??;

    Ok(product)
}

pub async fn update_stock(pool: &DbPool, product_id: ProductId, new_qty: i32) -> AppResult<()> {
    debug!("[Catalog.repo] Updating stock for product {}: new_qty={}", product_id, new_qty);
    let conn = pool.get().await?;
    conn
        .interact(move |conn| -> Result<_, rusqlite::Error> {
            let mut stmt = conn.prepare(
                "UPDATE catalog SET stock_quantity = ? WHERE product_id = ?"
            )?;
            let rows_affected = stmt.execute([new_qty, product_id.0 as i32])?;
            if rows_affected == 0 {
                return Err(rusqlite::Error::QueryReturnedNoRows);
            }
            Ok(())
        })
        .await
        .map_err(|e| {
            error!("[Catalog.repo] update_stock interact error: {e}");
            AppError::Database(rusqlite::Error::ToSqlConversionFailure(
                e.to_string().into(),
            ))
        })??;

    debug!("[Catalog.repo] Stock updated successfully for product {}", product_id);
    Ok(())
}
