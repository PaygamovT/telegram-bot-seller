use crate::shared::db::DbPool;
use crate::shared::error::{AppError, AppResult};
use crate::shared::types::{ChatId, ItemId, OrderId, ProductId};
use crate::modules::{contacts, catalog, orders};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, error, info};

// --- Strongly Typed Tool Argument Structures ---

#[derive(Debug, Deserialize)]
pub struct GetContactsArgs {
    pub chat_id: i64,
}

#[derive(Debug, Deserialize)]
pub struct UpdateContactsArgs {
    pub chat_id: i64,
    pub first_name: Option<String>,
    pub address: Option<String>,
    pub phone_number: Option<String>,
    pub username: Option<String>,
    pub nickname: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCatalogArgs {
    pub product_id: i64,
    pub stock_quantity: i32,
}

#[derive(Debug, Deserialize)]
pub struct InsertOrderArgs {
    pub chat_id: i64,
    pub status: String,
    pub delivery_address: Option<String>,
    pub total_amount: i32,
}

#[derive(Debug, Deserialize)]
pub struct OrderItemArg {
    pub product_id: i64,
    pub quantity: i32,
    pub price_at_sale: i32,
}

#[derive(Debug, Deserialize)]
pub struct InsertOrderItemsArgs {
    pub order_id: String,
    pub items: Vec<OrderItemArg>,
}

#[derive(Debug, Deserialize)]
pub struct GetOrdersArgs {
    pub chat_id: i64,
}

#[derive(Debug, Deserialize)]
pub struct GetOrderItemsArgs {
    pub order_id: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateOrderArgs {
    pub order_id: String,
    pub status: Option<String>,
    pub delivery_address: Option<String>,
    pub total_amount: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateOrderItemsArgs {
    pub item_id: String,
    pub quantity: Option<i32>,
    pub price_at_sale: Option<i32>,
}

// --- Dynamic Tool Execution Dispatcher ---

pub async fn execute_tool(
    name: &str,
    arguments_json: &str,
    pool: &DbPool,
) -> AppResult<String> {
    info!("[AI.Tools] Executing tool '{name}' with args: {arguments_json}");

    match name {
        "get_contacts" => {
            let args: GetContactsArgs = serde_json::from_str(arguments_json)?;
            let contact = contacts::get_contacts(pool, ChatId(args.chat_id)).await?;
            Ok(serde_json::to_string(&contact)?)
        }
        
        "update_contacts" => {
            let args: UpdateContactsArgs = serde_json::from_str(arguments_json)?;
            
            // Fetch existing contact to preserve unspecified fields
            let existing = contacts::get_contacts(pool, ChatId(args.chat_id)).await?;
            let updated_contact = contacts::Contact {
                chat_id: ChatId(args.chat_id),
                first_name: args.first_name.or(existing.as_ref().and_then(|c| c.first_name.clone())),
                address: args.address.or(existing.as_ref().and_then(|c| c.address.clone())),
                phone_number: args.phone_number.or(existing.as_ref().and_then(|c| c.phone_number.clone())),
                username: args.username.or(existing.as_ref().and_then(|c| c.username.clone())),
                nickname: args.nickname.or(existing.as_ref().and_then(|c| c.nickname.clone())),
            };
            
            contacts::update_contacts(pool, &updated_contact).await?;
            Ok(json!({ "status": "success", "message": "Contact updated successfully" }).to_string())
        }
        
        "get_catalog" => {
            let products = catalog::get_catalog(pool).await?;
            Ok(serde_json::to_string(&products)?)
        }
        
        "update_catalog" => {
            let args: UpdateCatalogArgs = serde_json::from_str(arguments_json)?;
            catalog::update_catalog_stock(pool, ProductId(args.product_id), args.stock_quantity).await?;
            Ok(json!({ "status": "success", "message": "Catalog stock updated successfully" }).to_string())
        }
        
        "insert_order" => {
            let args: InsertOrderArgs = serde_json::from_str(arguments_json)?;
            let order_id = OrderId::generate();
            
            let status = args.status.parse::<orders::OrderStatus>()
                .map_err(|e| AppError::Validation(e))?;
                
            let new_order = orders::Order {
                order_id: order_id.clone(),
                chat_id: ChatId(args.chat_id),
                status,
                delivery_address: args.delivery_address,
                total_amount: args.total_amount,
            };
            
            orders::insert_order(pool, &new_order).await?;
            Ok(json!({ "status": "success", "order_id": order_id.0 }).to_string())
        }
        
        "insert_order_items" => {
            let args: InsertOrderItemsArgs = serde_json::from_str(arguments_json)?;
            
            let mut domain_items = Vec::new();
            for item in args.items {
                // Generate a unique microsecond-based item_id
                let unique_id = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_micros() as i64;
                    
                // Pause slightly if executing in batch to ensure uniqueness
                tokio::time::sleep(std::time::Duration::from_nanos(100)).await;
                
                domain_items.push(orders::OrderItem {
                    item_id: ItemId(unique_id),
                    order_id: OrderId(args.order_id.clone()),
                    product_id: ProductId(item.product_id),
                    quantity: item.quantity,
                    price_at_sale: item.price_at_sale,
                });
            }
            
            orders::insert_order_items(pool, &domain_items).await?;
            Ok(json!({ "status": "success", "message": "Order items added successfully" }).to_string())
        }
        
        "get_orders" => {
            let args: GetOrdersArgs = serde_json::from_str(arguments_json)?;
            let orders_list = orders::get_orders(pool, ChatId(args.chat_id)).await?;
            Ok(serde_json::to_string(&orders_list)?)
        }
        
        "get_order_items" => {
            let args: GetOrderItemsArgs = serde_json::from_str(arguments_json)?;
            let items = orders::get_order_items(pool, &OrderId(args.order_id)).await?;
            Ok(serde_json::to_string(&items)?)
        }
        
        "update_order" => {
            let args: UpdateOrderArgs = serde_json::from_str(arguments_json)?;
            let order_id = OrderId(args.order_id);
            
            // Load existing order to preserve unspecified fields
            // Wait, does the order module provide a way to load a specific order by ID?
            // Let's check: orders::get_orders gets orders by ChatId.
            // Let's fallback to retrieving orders for a chat ID, or update specific fields
            // Wait! The repository `repo::update_order` performs a SQL update:
            // "UPDATE orders SET status = ?1, delivery_address = ?2, total_amount = ?3 WHERE order_id = ?4"
            // Wait! The repository `repo::update_order` updates all three fields.
            // So we need to fetch the existing order first. Let's write a small helper or retrieve
            // the order. Let's check what repo methods we have in orders module.
            // Wait, we had orders::get_orders, but does it fetch all order items? Yes.
            // Let's query SQLite directly or get orders for the chat_id. But wait! The `OrderId` itself
            // has `chat_id` inside the table, let's see if we can execute a custom fetch.
            // Actually, in repo.rs: is there a fetch_by_order_id? Let's check `src/modules/orders/repo.rs`.
            
            let existing_order = fetch_order_by_id_helper(pool, &order_id).await?;
            
            let status = match args.status {
                Some(s) => s.parse::<orders::OrderStatus>().map_err(|e| AppError::Validation(e))?,
                None => existing_order.status,
            };
            let delivery_address = args.delivery_address.or(existing_order.delivery_address);
            let total_amount = args.total_amount.unwrap_or(existing_order.total_amount);
            
            orders::update_order(pool, &order_id, status, delivery_address, total_amount).await?;
            Ok(json!({ "status": "success", "message": "Order updated successfully" }).to_string())
        }
        
        "update_order_items" => {
            let args: UpdateOrderItemsArgs = serde_json::from_str(arguments_json)?;
            let item_id = ItemId(args.item_id.parse::<i64>().map_err(|_| AppError::Validation("item_id must be an integer".to_string()))?);
            
            // To update order items: repo::update_order_item(pool, item_id, quantity, price_at_sale)
            // Let's fetch the existing item to preserve fields if some are None
            let existing_item = fetch_order_item_by_id_helper(pool, item_id).await?;
            
            let quantity = args.quantity.unwrap_or(existing_item.quantity);
            let price_at_sale = args.price_at_sale.unwrap_or(existing_item.price_at_sale);
            
            orders::update_order_items(pool, item_id, quantity, price_at_sale).await?;
            Ok(json!({ "status": "success", "message": "Order item updated successfully" }).to_string())
        }
        
        _ => {
            error!("[AI.Tools] Unknown tool: {name}");
            Err(AppError::UnknownTool(name.to_string()))
        }
    }
}

// --- Internal helpers to fetch individual records since they aren't directly exposed in mod.rs public APIs ---

async fn fetch_order_by_id_helper(pool: &DbPool, order_id: &OrderId) -> AppResult<orders::Order> {
    let conn = pool.get().await?;
    let order_id_str = order_id.0.clone();
    
    conn.interact(move |conn| {
        let mut stmt = conn.prepare("SELECT order_id, chat_id, status, delivery_address, total_amount FROM orders WHERE order_id = ?1")?;
        let mut rows = stmt.query([order_id_str])?;
        if let Some(row) = rows.next()? {
            let status_str: String = row.get(2)?;
            let status = status_str.parse::<orders::OrderStatus>()
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;
                
            Ok(orders::Order {
                order_id: OrderId(row.get(0)?),
                chat_id: ChatId(row.get(1)?),
                status,
                delivery_address: row.get(3)?,
                total_amount: row.get(4)?,
            })
        } else {
            Err(rusqlite::Error::QueryReturnedNoRows)
        }
    })
    .await
    .map_err(|e| AppError::Database(rusqlite::Error::ToSqlConversionFailure(e.to_string().into())))?
    .map_err(AppError::Database)
}

async fn fetch_order_item_by_id_helper(pool: &DbPool, item_id: ItemId) -> AppResult<orders::OrderItem> {
    let conn = pool.get().await?;
    let id = item_id.0;
    
    conn.interact(move |conn| {
        let mut stmt = conn.prepare("SELECT item_id, order_id, product_id, quantity, price_at_sale FROM order_items WHERE item_id = ?1")?;
        let mut rows = stmt.query([id])?;
        if let Some(row) = rows.next()? {
            let item_id_raw: i64 = row.get(0)?;
            let product_id_raw: i64 = row.get(2)?;
            Ok(orders::OrderItem {
                item_id: ItemId(item_id_raw),
                order_id: OrderId(row.get(1)?),
                product_id: ProductId(product_id_raw),
                quantity: row.get(3)?,
                price_at_sale: row.get(4)?,
            })
        } else {
            Err(rusqlite::Error::QueryReturnedNoRows)
        }
    })
    .await
    .map_err(|e| AppError::Database(rusqlite::Error::ToSqlConversionFailure(e.to_string().into())))?
    .map_err(AppError::Database)
}

// --- JSON Tool Schemas for MiniMax API ---

pub fn get_minimax_tools_schema() -> serde_json::Value {
    json!([
        {
            "type": "function",
            "function": {
                "name": "get_contacts",
                "description": "Fetch user's current contact record details including delivery address and phone number.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "chat_id": {
                            "type": "integer",
                            "description": "The Telegram chat ID of the user."
                        }
                    },
                    "required": ["chat_id"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "update_contacts",
                "description": "Create or update user's profile info (first name, phone, address, username, nickname).",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "chat_id": {
                            "type": "integer",
                            "description": "The Telegram chat ID of the user."
                        },
                        "first_name": { "type": "string" },
                        "address": { "type": "string" },
                        "phone_number": { "type": "string" },
                        "username": { "type": "string" },
                        "nickname": { "type": "string" }
                    },
                    "required": ["chat_id"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "get_catalog",
                "description": "Fetch the full catalog of perfume products with stock levels, notes, tags, prices, sillage, season, etc.",
                "parameters": {
                    "type": "object",
                    "properties": {}
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "update_catalog",
                "description": "Update product stock quantity levels directly.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "product_id": { "type": "integer" },
                        "stock_quantity": { "type": "integer" }
                    },
                    "required": ["product_id", "stock_quantity"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "insert_order",
                "description": "Create a new perfume order. Returns the unique order_id which you MUST save to add order items subsequently.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "chat_id": { "type": "integer" },
                        "status": {
                            "type": "string",
                            "enum": ["pending", "paid", "shipped", "cancelled"]
                        },
                        "delivery_address": { "type": "string" },
                        "total_amount": { "type": "integer" }
                    },
                    "required": ["chat_id", "status", "total_amount"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "insert_order_items",
                "description": "Add multiple line items to a previously created order ID.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "order_id": { "type": "string" },
                        "items": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "product_id": { "type": "integer" },
                                    "quantity": { "type": "integer" },
                                    "price_at_sale": { "type": "integer" }
                                },
                                "required": ["product_id", "quantity", "price_at_sale"]
                            }
                        }
                    },
                    "required": ["order_id", "items"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "get_orders",
                "description": "Retrieve order history records associated with a specific user's chat ID.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "chat_id": { "type": "integer" }
                    },
                    "required": ["chat_id"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "get_order_items",
                "description": "Retrieve all line item entries belonging to a given order ID.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "order_id": { "type": "string" }
                    },
                    "required": ["order_id"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "update_order",
                "description": "Update existing order fields (status, delivery address, or total amount).",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "order_id": { "type": "string" },
                        "status": {
                            "type": "string",
                            "enum": ["pending", "paid", "shipped", "cancelled"]
                        },
                        "delivery_address": { "type": "string" },
                        "total_amount": { "type": "integer" }
                    },
                    "required": ["order_id"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "update_order_items",
                "description": "Update details of a specific line item entry inside an order.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "item_id": {
                            "type": "string",
                            "description": "The unique item_id (line entry ID)."
                        },
                        "quantity": { "type": "integer" },
                        "price_at_sale": { "type": "integer" }
                    },
                    "required": ["item_id"]
                }
            }
        }
    ])
}
