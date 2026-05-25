mod domain;
mod repo;

pub use domain::Product;

use crate::shared::db::DbPool;
use crate::shared::error::AppResult;
use crate::shared::types::ProductId;

/// Get all products in the catalog
pub async fn get_catalog(pool: &DbPool) -> AppResult<Vec<Product>> {
    repo::fetch_all(pool).await
}

/// Get a specific product by ID
pub async fn get_product(pool: &DbPool, product_id: ProductId) -> AppResult<Option<Product>> {
    repo::fetch_by_id(pool, product_id).await
}

/// Update stock quantity for a product in the catalog
pub async fn update_catalog_stock(pool: &DbPool, product_id: ProductId, new_qty: i32) -> AppResult<()> {
    repo::update_stock(pool, product_id, new_qty).await
}
