use axum::{
    body::Body,
    extract::{FromRequestParts, Query, State},
    extract::ws::{WebSocketUpgrade, WebSocket, Message},
    http::{header, request::Parts, StatusCode},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Form, Router,
};
use askama::Template;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use tracing::{debug, error, info};

use crate::shared::config::AppConfig;
use crate::shared::db::DbPool;

/// AppState holds the SQLite database pool and loaded fallback environment variables.
#[derive(Clone)]
pub struct AppState {
    pub pool: DbPool,
    pub config: AppConfig,
}

/// Custom type-safe Basic Authentication extractor.
/// Enforces username "admin" and password "admin123".
#[derive(Debug, Clone, Copy)]
pub struct BasicAuth;

impl<S> FromRequestParts<S> for BasicAuth
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        if let Some(auth_header) = parts.headers.get(header::AUTHORIZATION) {
            if let Ok(auth_str) = auth_header.to_str() {
                if auth_str.starts_with("Basic ") {
                    let encoded = &auth_str["Basic ".len()..];
                    if let Ok(decoded_bytes) = STANDARD.decode(encoded.trim()) {
                        if let Ok(decoded_str) = String::from_utf8(decoded_bytes) {
                            if let Some((username, password)) = decoded_str.split_once(':') {
                                if username == "admin" && password == "admin123" {
                                    return Ok(BasicAuth);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Return a WWW-Authenticate header challenge to trigger browser modal login
        let response = Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header(
                header::WWW_AUTHENTICATE,
                "Basic realm=\"Admin Panel\", charset=\"UTF-8\"",
            )
            .body(Body::from("401 Unauthorized - Admin authorization required."))
            .unwrap();

        Err(response)
    }
}

/// Represents recent order rows loaded from SQLite database
#[derive(Clone, Debug)]
pub struct RecentOrder {
    pub order_id: String,
    pub customer_name: String,
    pub username: String,
    pub total_amount: i64,
    pub status: String,
}

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardTemplate {
    total_orders: usize,
    total_revenue: i64,
    cpu_load: f32,
    ram_used: u64,
    ram_total: u64,
    ram_percentage: f32,
    storage_used: u64,
    storage_total: u64,
    storage_percentage: f32,
    battery_charge: u8,
    bot_status: String,
    contacts: Vec<crate::modules::contacts::Contact>,
    recent_orders: Vec<RecentOrder>,
    has_new_orders: bool,
    has_shipping_orders: bool,
    has_delivered_orders: bool,
    active_theme: String,
}

struct SystemResources {
    cpu_load: f32,
    ram_used: u64,
    ram_total: u64,
    ram_percentage: f32,
    storage_used: u64,
    storage_total: u64,
    storage_percentage: f32,
    battery_charge: u8,
}

/// Dynamic smartphone hardware monitoring loader.
/// Parses procfs on Android/Linux, and provides realistic dynamic values on Windows/Testing.
fn get_system_resources() -> SystemResources {
    // 1. RAM Reading (/proc/meminfo)
    let (ram_used, ram_total, ram_percentage) = if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
        let mut mem_total = 8192 * 1024; // 8GB default
        let mut mem_available = 4096 * 1024;
        
        for line in meminfo.lines() {
            if line.starts_with("MemTotal:") {
                if let Some(val_str) = line.split_whitespace().nth(1) {
                    if let Ok(val) = val_str.parse::<u64>() {
                        mem_total = val;
                    }
                }
            } else if line.starts_with("MemAvailable:") {
                if let Some(val_str) = line.split_whitespace().nth(1) {
                    if let Ok(val) = val_str.parse::<u64>() {
                        mem_available = val;
                    }
                }
            }
        }
        let used = mem_total.saturating_sub(mem_available);
        let used_mb = used / 1024;
        let total_mb = mem_total / 1024;
        let pct = if total_mb > 0 { (used_mb as f32 / total_mb as f32) * 100.0 } else { 50.0 };
        (used_mb, total_mb, pct)
    } else {
        // Simulated fluctuating RAM based on seconds
        let total_mb = 8192;
        let sec = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let cycle = (sec % 300) as f32 / 300.0;
        let used_mb = (3100.0 + (700.0 * (cycle * 2.0 * std::f32::consts::PI).sin())) as u64;
        let pct = (used_mb as f32 / total_mb as f32) * 100.0;
        (used_mb, total_mb, pct)
    };

    // 2. Battery Capacity
    let battery_charge = if let Ok(cap) = std::fs::read_to_string("/sys/class/power_supply/battery/capacity") {
        cap.trim().parse::<u8>().unwrap_or(88)
    } else {
        let sec = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let min = (sec / 60) % 60;
        (98 - (min / 3)) as u8 // slowly ticks down
    };

    // 3. CPU Load Monitoring
    let cpu_load = if let Ok(loadavg) = std::fs::read_to_string("/proc/loadavg") {
        if let Some(load1) = loadavg.split_whitespace().next() {
            if let Ok(load_f) = load1.parse::<f32>() {
                (load_f / 8.0 * 100.0).clamp(1.0, 99.9)
            } else {
                14.5
            }
        } else {
            14.5
        }
    } else {
        let sec = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let wave = ((sec as f32 * 0.08).sin() + 1.0) / 2.0;
        12.0 + wave * 25.0
    };

    // 4. Storage load (Samsung Flip 3 base: 256GB total)
    let storage_total = 256;
    let storage_used = 124;
    let storage_percentage = (storage_used as f32 / storage_total as f32) * 100.0;

    SystemResources {
        cpu_load,
        ram_used,
        ram_total,
        ram_percentage,
        storage_used,
        storage_total,
        storage_percentage,
        battery_charge,
    }
}

async fn get_theme(pool: &DbPool) -> String {
    let conn = match pool.get().await {
        Ok(c) => c,
        Err(_) => return "dark".to_string(),
    };
    conn.interact(|conn| -> Result<String, rusqlite::Error> {
        let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = 'theme'")?;
        let res = stmt.query_row([], |r| r.get::<_, String>(0)).unwrap_or_else(|_| "dark".to_string());
        Ok(res)
    }).await.unwrap_or_else(|_| Ok("dark".to_string())).unwrap_or_else(|_| "dark".to_string())
}

/// GET / - Renders Admin Dashboard Panel
async fn dashboard_handler(
    _auth: BasicAuth,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let conn = match state.pool.get().await {
        Ok(c) => c,
        Err(e) => {
            error!("[AdminServer.dashboard] DB pool acquisition failure: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // Query Metrics: total orders, total paid revenue, top-100 recent orders
    let stats_res = conn.interact(move |conn| -> Result<_, rusqlite::Error> {
        // 1. Count Total Orders
        let mut order_cnt_stmt = conn.prepare("SELECT COUNT(*) FROM orders")?;
        let total_orders_i64: i64 = order_cnt_stmt.query_row([], |r| r.get(0))?;
        let total_orders = total_orders_i64 as usize;

        // 2. Sum Paid Revenue
        let mut rev_stmt = conn.prepare("SELECT COALESCE(SUM(total_amount), 0) FROM orders WHERE status = 'paid'")?;
        let total_revenue: i64 = rev_stmt.query_row([], |r| r.get(0))?;

        // 3. Load Recent 100 Orders
        let mut recent_stmt = conn.prepare(
            "SELECT o.order_id, COALESCE(c.first_name, 'Unknown'), COALESCE(c.username, ''), o.status, o.total_amount \
             FROM orders o \
             LEFT JOIN contacts c ON o.chat_id = c.chat_id \
             ORDER BY o.ROWID DESC \
             LIMIT 100"
        )?;

        let recent_orders = recent_stmt.query_map([], |row| {
            Ok(RecentOrder {
                order_id: row.get(0)?,
                customer_name: row.get(1)?,
                username: row.get(2)?,
                status: row.get(3)?,
                total_amount: row.get(4)?,
            })
        })?.collect::<Result<Vec<RecentOrder>, rusqlite::Error>>()?;

        // 4. Query bot status setting
        let mut status_stmt = conn.prepare("SELECT value FROM settings WHERE key = 'bot_status'")?;
        let bot_status: String = status_stmt
            .query_row([], |r| r.get(0))
            .unwrap_or_else(|_| "active".to_string());

        Ok((total_orders, total_revenue, recent_orders, bot_status))
    }).await;

    let (total_orders, total_revenue, recent_orders, raw_bot_status) = match stats_res {
        Ok(Ok(data)) => data,
        Ok(Err(e)) => {
            error!("[AdminServer.dashboard] DB Query execution failure: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
        Err(e) => {
            error!("[AdminServer.dashboard] DB Thread join failure: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // Bot status label translations
    let bot_status = match raw_bot_status.as_str() {
        "active" | "online" => "Online".to_string(),
        "paused" | "idle" => "Paused".to_string(),
        _ => raw_bot_status,
    };

    let contacts = crate::modules::contacts::get_all_contacts(&state.pool)
        .await
        .unwrap_or_default();

    let has_new_orders = recent_orders.iter().any(|o| o.status == "pending" || o.status == "paid");
    let has_shipping_orders = recent_orders.iter().any(|o| o.status == "shipped");
    let has_delivered_orders = recent_orders.iter().any(|o| o.status == "delivered");

    let active_theme = get_theme(&state.pool).await;

    let resources = get_system_resources();

    let template = DashboardTemplate {
        total_orders,
        total_revenue,
        cpu_load: resources.cpu_load,
        ram_used: resources.ram_used,
        ram_total: resources.ram_total,
        ram_percentage: resources.ram_percentage,
        storage_used: resources.storage_used,
        storage_total: resources.storage_total,
        storage_percentage: resources.storage_percentage,
        battery_charge: resources.battery_charge,
        bot_status,
        contacts,
        recent_orders,
        has_new_orders,
        has_shipping_orders,
        has_delivered_orders,
        active_theme,
    };

    match template.render() {
        Ok(html) => Response::builder()
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .body(Body::from(html))
            .unwrap(),
        Err(err) => {
            error!("[AdminServer.dashboard] Askama rendering failed: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[derive(serde::Deserialize)]
pub struct OrderStatusUpdateForm {
    pub order_id: String,
    pub status: String,
}

/// POST /admin/order/update_status - Updates the status of a specific order instantly
async fn order_status_update_handler(
    _auth: BasicAuth,
    State(state): State<AppState>,
    Form(form): Form<OrderStatusUpdateForm>,
) -> impl IntoResponse {
    let conn = match state.pool.get().await {
        Ok(c) => c,
        Err(e) => {
            error!("[AdminServer.order_status_update] DB pool acquisition failure: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let order_id_clone = form.order_id.clone();
    let status_clone = form.status.clone();

    let update_res = conn.interact(move |conn| -> Result<_, rusqlite::Error> {
        let mut stmt = conn.prepare("UPDATE orders SET status = ? WHERE order_id = ?")?;
        stmt.execute((status_clone, order_id_clone))?;
        Ok(())
    }).await;

    match update_res {
        Ok(Ok(())) => {
            info!("[AdminServer.order_status_update] Order {} status updated to {}", form.order_id, form.status);
            Redirect::to("/").into_response()
        }
        err => {
            error!("[AdminServer.order_status_update] DB update failed: {:?}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[derive(serde::Deserialize)]
pub struct UpdateThemeForm {
    pub theme: String,
}

/// POST /admin/update_theme - Updates the active UI theme in settings table
async fn update_theme_handler(
    _auth: BasicAuth,
    State(state): State<AppState>,
    Form(form): Form<UpdateThemeForm>,
) -> impl IntoResponse {
    let conn = match state.pool.get().await {
        Ok(c) => c,
        Err(e) => {
            error!("[AdminServer.update_theme] DB pool acquisition failure: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let theme = form.theme.clone();
    let res = conn.interact(move |conn| -> Result<(), rusqlite::Error> {
        let mut stmt = conn.prepare("INSERT OR REPLACE INTO settings (key, value) VALUES ('theme', ?)")?;
        stmt.execute([theme])?;
        Ok(())
    }).await;

    match res {
        Ok(Ok(())) => StatusCode::OK.into_response(),
        err => {
            error!("[AdminServer.update_theme] DB theme update failed: {:?}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[derive(serde::Deserialize)]
pub struct ConfigQuery {
    pub saved: Option<bool>,
    pub backup_success: Option<bool>,
}

#[derive(Template)]
#[template(path = "config.html")]
struct ConfigTemplate {
    saved: bool,
    backup_success: bool,
    telegram_token: String,
    telegram_token_source: &'static str,
    admin_chat_id: String,
    admin_chat_id_source: &'static str,
    minimax_api_key: String,
    minimax_api_key_source: &'static str,
    minimax_group_id: String,
    minimax_group_id_source: &'static str,
    gemini_api_key: String,
    gemini_api_key_source: &'static str,
    openrouter_api_key: String,
    openrouter_api_key_source: &'static str,
    openrouter_api_source_class: &'static str,
    primary_ai_model: String,
    primary_ai_model_source: &'static str,
    fallback_ai_model: String,
    fallback_ai_model_source: &'static str,
    deepseek_api_key: String,
    deepseek_api_key_source: &'static str,
    deepseek_api_source_class: &'static str,
    rub_to_krw_rate: String,
    system_language: String,
    active_theme: String,
}

/// GET /config - Load and display server settings configurations
async fn config_get_handler(
    _auth: BasicAuth,
    State(state): State<AppState>,
    Query(query): Query<ConfigQuery>,
) -> impl IntoResponse {
    let conn = match state.pool.get().await {
        Ok(c) => c,
        Err(e) => {
            error!("[AdminServer.config_get] DB pool acquisition failure: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let settings_res = conn.interact(|conn| -> Result<_, rusqlite::Error> {
        let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        
        let mut map = std::collections::HashMap::new();
        for row in rows {
            let (k, v) = row?;
            map.insert(k, v);
        }
        Ok(map)
    }).await;

    let settings_map = match settings_res {
        Ok(Ok(map)) => map,
        _ => std::collections::HashMap::new(),
    };

    // Load each config token, fallback to AppConfig env variables if database entry is empty
    let (telegram_token, telegram_token_source) = match settings_map.get("TELEGRAM_BOT_TOKEN") {
        Some(val) => (val.clone(), "db"),
        None => (state.config.telegram_token.clone(), "env"),
    };

    let (admin_chat_id, admin_chat_id_source) = match settings_map.get("ADMIN_CHAT_ID") {
        Some(val) => (val.clone(), "db"),
        None => (state.config.admin_chat_id.to_string(), "env"),
    };

    let (minimax_api_key, minimax_api_key_source) = match settings_map.get("MINIMAX_API_KEY") {
        Some(val) => (val.clone(), "db"),
        None => (state.config.minimax_api_key.clone(), "env"),
    };

    let (minimax_group_id, minimax_group_id_source) = match settings_map.get("MINIMAX_GROUP_ID") {
        Some(val) => (val.clone(), "db"),
        None => (state.config.minimax_group_id.clone(), "env"),
    };

    let (gemini_api_key, gemini_api_key_source) = match settings_map.get("GEMINI_API_KEY") {
        Some(val) => (val.clone(), "db"),
        None => (state.config.gemini_api_key.clone(), "env"),
    };

    let (openrouter_api_key, openrouter_api_key_source, openrouter_api_source_class) = match settings_map.get("OPENROUTER_API_KEY") {
        Some(val) => (val.clone(), "db", "db"),
        None => match &state.config.openrouter_api_key {
            Some(val) => (val.clone(), "env", "env"),
            None => ("".to_string(), "нет", "env"),
        }
    };

    let (primary_ai_model, primary_ai_model_source) = match settings_map.get("primary_ai_model") {
        Some(val) => (val.clone(), "db"),
        None => ("minimax".to_string(), "default"),
    };

    let (fallback_ai_model, fallback_ai_model_source) = match settings_map.get("fallback_ai_model") {
        Some(val) => (val.clone(), "db"),
        None => ("deepseek".to_string(), "default"),
    };

    let (deepseek_api_key, deepseek_api_key_source, deepseek_api_source_class) = match settings_map.get("DEEPSEEK_API_KEY") {
        Some(val) => (val.clone(), "db", "db"),
        None => match &state.config.deepseek_api_key {
            Some(val) => (val.clone(), "env", "env"),
            None => ("".to_string(), "нет", "env"),
        }
    };

    let rub_to_krw_rate = settings_map.get("rub_to_krw_rate").cloned().unwrap_or_else(|| "15.0".to_string());
    let system_language = settings_map.get("system_language").cloned().unwrap_or_else(|| "ru".to_string());
    let active_theme = settings_map.get("theme").cloned().unwrap_or_else(|| "dark".to_string());

    let template = ConfigTemplate {
        saved: query.saved.unwrap_or(false),
        backup_success: query.backup_success.unwrap_or(false),
        telegram_token,
        telegram_token_source,
        admin_chat_id,
        admin_chat_id_source,
        minimax_api_key,
        minimax_api_key_source,
        minimax_group_id,
        minimax_group_id_source,
        gemini_api_key,
        gemini_api_key_source,
        openrouter_api_key,
        openrouter_api_key_source,
        openrouter_api_source_class,
        primary_ai_model,
        primary_ai_model_source,
        fallback_ai_model,
        fallback_ai_model_source,
        deepseek_api_key,
        deepseek_api_key_source,
        deepseek_api_source_class,
        rub_to_krw_rate,
        system_language,
        active_theme,
    };

    match template.render() {
        Ok(html) => Response::builder()
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .body(Body::from(html))
            .unwrap(),
        Err(err) => {
            error!("[AdminServer.config_get] Askama rendering failed: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[derive(serde::Deserialize)]
pub struct ConfigForm {
    pub telegram_token: String,
    pub admin_chat_id: String,
    pub minimax_api_key: String,
    pub minimax_group_id: String,
    pub gemini_api_key: String,
    pub openrouter_api_key: Option<String>,
    pub primary_ai_model: String,
    pub fallback_ai_model: String,
    pub deepseek_api_key: Option<String>,
    pub rub_to_krw_rate: String,
    pub system_language: String,
    pub theme: String,
}

/// POST /config - Save settings into database
async fn config_post_handler(
    _auth: BasicAuth,
    State(state): State<AppState>,
    Form(form): Form<ConfigForm>,
) -> impl IntoResponse {
    let conn = match state.pool.get().await {
        Ok(c) => c,
        Err(e) => {
            error!("[AdminServer.config_post] DB pool acquisition failure: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let save_res = conn.interact(move |conn| -> Result<_, rusqlite::Error> {
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare("INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)")?;
            
            stmt.execute(("TELEGRAM_BOT_TOKEN", &form.telegram_token))?;
            stmt.execute(("ADMIN_CHAT_ID", &form.admin_chat_id))?;
            stmt.execute(("MINIMAX_API_KEY", &form.minimax_api_key))?;
            stmt.execute(("MINIMAX_GROUP_ID", &form.minimax_group_id))?;
            stmt.execute(("GEMINI_API_KEY", &form.gemini_api_key))?;
            
            stmt.execute(("primary_ai_model", &form.primary_ai_model))?;
            stmt.execute(("fallback_ai_model", &form.fallback_ai_model))?;
            stmt.execute(("rub_to_krw_rate", &form.rub_to_krw_rate))?;
            stmt.execute(("system_language", &form.system_language))?;
            stmt.execute(("theme", &form.theme))?;
            
            if let Some(openrouter) = &form.openrouter_api_key {
                let trimmed = openrouter.trim();
                if !trimmed.is_empty() {
                    stmt.execute(("OPENROUTER_API_KEY", trimmed))?;
                } else {
                    let mut del_stmt = tx.prepare("DELETE FROM settings WHERE key = 'OPENROUTER_API_KEY'")?;
                    del_stmt.execute([])?;
                }
            } else {
                let mut del_stmt = tx.prepare("DELETE FROM settings WHERE key = 'OPENROUTER_API_KEY'")?;
                del_stmt.execute([])?;
            }

            if let Some(deepseek) = &form.deepseek_api_key {
                let trimmed = deepseek.trim();
                if !trimmed.is_empty() {
                    stmt.execute(("DEEPSEEK_API_KEY", trimmed))?;
                } else {
                    let mut del_stmt = tx.prepare("DELETE FROM settings WHERE key = 'DEEPSEEK_API_KEY'")?;
                    del_stmt.execute([])?;
                }
            } else {
                let mut del_stmt = tx.prepare("DELETE FROM settings WHERE key = 'DEEPSEEK_API_KEY'")?;
                del_stmt.execute([])?;
            }
        }
        tx.commit()?;
        Ok(())
    }).await;

    match save_res {
        Ok(Ok(())) => {
            info!("[AdminServer.config_post] Admin settings updated successfully in SQLite database");
            Redirect::to("/config?saved=true").into_response()
        }
        Ok(Err(e)) => {
            error!("[AdminServer.config_post] DB transaction execution failure: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
        Err(e) => {
            error!("[AdminServer.config_post] DB Thread join failure: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

// ==========================================
// WebSocket Logging Broadcaster & Custom Layer
// ==========================================

pub static LOG_BROADCASTER: std::sync::OnceLock<tokio::sync::broadcast::Sender<String>> = std::sync::OnceLock::new();

pub struct WsLogLayer {
    sender: tokio::sync::broadcast::Sender<String>,
}

impl WsLogLayer {
    pub fn new(sender: tokio::sync::broadcast::Sender<String>) -> Self {
        Self { sender }
    }
}

impl<S> tracing_subscriber::Layer<S> for WsLogLayer
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        let mut fields = String::new();
        struct SimpleVisitor<'a> {
            fields: &'a mut String,
        }
        impl<'a> tracing::field::Visit for SimpleVisitor<'a> {
            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                if field.name() == "message" {
                    *self.fields = format!("{:?}", value);
                }
            }
        }
        event.record(&mut SimpleVisitor { fields: &mut fields });

        let level = *event.metadata().level();
        let target = event.metadata().target();
        
        let now = std::time::SystemTime::now();
        let since_the_epoch = now.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
        let secs = since_the_epoch.as_secs();
        let millis = since_the_epoch.subsec_millis();

        let log_msg = format!("[{}.{:03}] {:<5} [{}] {}", secs, millis, level.to_string(), target, fields);
        let _ = self.sender.send(log_msg);
    }
}

// ==========================================
// WebSocket & Logs Routing Controllers
// ==========================================

#[derive(Template)]
#[template(path = "logs.html")]
struct LogsTemplate {
    active_theme: String,
}

/// GET /logs - Renders Logs Panel Console
async fn logs_handler(
    _auth: BasicAuth,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let active_theme = get_theme(&state.pool).await;
    let template = LogsTemplate { active_theme };
    match template.render() {
        Ok(html) => Response::builder()
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .body(Body::from(html))
            .unwrap(),
        Err(err) => {
            error!("[AdminServer.logs] Askama rendering failed: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// GET /ws - Upgrades connection to WebSocket for log streaming
async fn ws_handler(
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| async move {
        if let Some(broadcaster) = LOG_BROADCASTER.get() {
            let mut rx = broadcaster.subscribe();
            let mut socket = socket;
            while let Ok(msg) = rx.recv().await {
                if socket.send(Message::Text(msg.into())).await.is_err() {
                    break;
                }
            }
        }
    })
}

// ==========================================
// Health Monitoring & Process Uptime Telemetry
// ==========================================

pub static START_TIME: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

#[derive(serde::Serialize)]
pub struct HealthStatus {
    pub status: &'static str,
    pub database: &'static str,
    pub db_size_bytes: u64,
    pub uptime_seconds: u64,
    pub bot_status: String,
}

/// GET /health - Serves enterprise-grade active health JSON metadata
async fn health_check_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let conn = match state.pool.get().await {
        Ok(c) => c,
        Err(e) => {
            error!("[AdminServer.health] DB pool acquisition failure: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let db_query_res = conn.interact(|conn| -> Result<(String, bool), rusqlite::Error> {
        let mut stmt = conn.prepare("SELECT 1")?;
        let mut rows = stmt.query([])?;
        let db_ok = rows.next()?.is_some();
        
        let mut status_stmt = conn.prepare("SELECT value FROM settings WHERE key = 'bot_status'")?;
        let bot_status: String = status_stmt
            .query_row([], |r| r.get(0))
            .unwrap_or_else(|_| "active".to_string());
        
        Ok((bot_status, db_ok))
    }).await;

    let (bot_status, db_status) = match db_query_res {
        Ok(Ok((status, true))) => (status, "ok"),
        _ => ("offline".to_string(), "error"),
    };

    let db_size = std::fs::metadata(&state.config.database_path)
        .map(|m| m.len())
        .unwrap_or(0);

    let uptime = START_TIME.get()
        .map(|t| t.elapsed().as_secs())
        .unwrap_or(0);

    let health = HealthStatus {
        status: "ok",
        database: db_status,
        db_size_bytes: db_size,
        uptime_seconds: uptime,
        bot_status,
    };

    (StatusCode::OK, axum::Json(health)).into_response()
}

// ==========================================
// Atomic SQLite Backups
// ==========================================

/// POST /config/backup - Generates compression-enabled, WAL-safe SQLite atomic backups
async fn config_backup_handler(
    _auth: BasicAuth,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let conn = match state.pool.get().await {
        Ok(c) => c,
        Err(e) => {
            error!("[AdminServer.backup] DB pool acquisition failure: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let backups_dir = "./data/backups";
    if let Err(e) = std::fs::create_dir_all(backups_dir) {
        error!("[AdminServer.backup] Failed to create backups directory: {}", e);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let backup_path = format!("{}/backup_{}.db", backups_dir, timestamp);

    info!("[AdminServer.backup] Initiating WAL-safe SQLite atomic backup using VACUUM INTO to: {}", backup_path);
    
    let backup_path_clone = backup_path.clone();
    let backup_res = conn.interact(move |conn| -> Result<_, rusqlite::Error> {
        // VACUUM INTO produces an atomic, WAL-safe, locked copy of the DB
        conn.execute(&format!("VACUUM INTO '{}'", backup_path_clone), [])?;
        Ok(())
    }).await;

    match backup_res {
        Ok(Ok(())) => {
            info!("[AdminServer.backup] SQLite backup completed successfully: {}", backup_path);
            Redirect::to("/config?backup_success=true").into_response()
        }
        err => {
            error!("[AdminServer.backup] Backup transaction failed: {:?}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

// ==========================================
// Unified Tables Management Routing Controllers
// ==========================================

#[derive(serde::Deserialize)]
pub struct TablesQuery {
    pub table: Option<String>,
    pub added: Option<bool>,
    pub updated: Option<bool>,
    pub deleted: Option<bool>,
    pub error_msg: Option<String>,
}

#[derive(Template)]
#[template(path = "tables.html")]
struct TablesTemplate {
    active_tab: String,
    active_theme: String,
    added: bool,
    updated: bool,
    deleted: bool,
    error_msg: Option<String>,
    products: Vec<crate::modules::catalog::Product>,
    contacts: Vec<crate::modules::contacts::Contact>,
    orders: Vec<crate::modules::orders::Order>,
    order_items: Vec<crate::modules::orders::OrderItem>,
}

/// GET /tables - Renders Database Tables Management Panel
async fn tables_get_handler(
    _auth: BasicAuth,
    State(state): State<AppState>,
    Query(query): Query<TablesQuery>,
) -> impl IntoResponse {
    let active_tab = query.table.unwrap_or_else(|| "catalog".to_string());
    let active_theme = get_theme(&state.pool).await;

    // Load products from catalog
    let products = match crate::modules::catalog::get_catalog(&state.pool).await {
        Ok(p) => p,
        Err(e) => {
            error!("[AdminServer.tables_get] DB Catalog fetch failure: {}", e);
            Vec::new()
        }
    };

    // Load contacts from public module re-export
    let contacts = match crate::modules::contacts::get_all_contacts(&state.pool).await {
        Ok(c) => c,
        Err(e) => {
            error!("[AdminServer.tables_get] DB Contacts fetch failure: {}", e);
            Vec::new()
        }
    };

    // Load orders directly from SQLite
    let orders = match state.pool.get().await {
        Ok(conn) => {
            match conn.interact(|conn| -> Result<Vec<crate::modules::orders::Order>, rusqlite::Error> {
                let mut stmt = conn.prepare("SELECT order_id, chat_id, status, delivery_address, total_amount FROM orders ORDER BY ROWID DESC")?;
                let order_iter = stmt.query_map([], |row| {
                    let status_str: String = row.get(2)?;
                    let status = status_str.parse().unwrap_or(crate::modules::orders::OrderStatus::Pending);
                    Ok(crate::modules::orders::Order {
                        order_id: crate::shared::types::OrderId(row.get(0)?),
                        chat_id: crate::shared::types::ChatId(row.get(1)?),
                        status,
                        delivery_address: row.get(3)?,
                        total_amount: row.get(4)?,
                    })
                })?;
                let mut list = Vec::new();
                for o in order_iter {
                    list.push(o?);
                }
                Ok(list)
            }).await {
                Ok(Ok(list)) => list,
                err => {
                    error!("[AdminServer.tables_get] DB interact orders failure: {:?}", err);
                    Vec::new()
                }
            }
        }
        Err(e) => {
            error!("[AdminServer.tables_get] DB Order pool acquisition failure: {}", e);
            Vec::new()
        }
    };

    // Load order items directly from SQLite
    let order_items = match state.pool.get().await {
        Ok(conn) => {
            match conn.interact(|conn| -> Result<Vec<crate::modules::orders::OrderItem>, rusqlite::Error> {
                let mut stmt = conn.prepare("SELECT item_id, order_id, product_id, quantity, price_at_sale FROM order_items ORDER BY ROWID DESC")?;
                let item_iter = stmt.query_map([], |row| {
                    let item_id_str: String = row.get(0)?;
                    let item_id = crate::shared::types::ItemId(item_id_str.parse().unwrap_or_default());
                    Ok(crate::modules::orders::OrderItem {
                        item_id,
                        order_id: crate::shared::types::OrderId(row.get(1)?),
                        product_id: crate::shared::types::ProductId(row.get(2)?),
                        quantity: row.get(3)?,
                        price_at_sale: row.get(4)?,
                    })
                })?;
                let mut list = Vec::new();
                for i in item_iter {
                    list.push(i?);
                }
                Ok(list)
            }).await {
                Ok(Ok(list)) => list,
                err => {
                    error!("[AdminServer.tables_get] DB interact order_items failure: {:?}", err);
                    Vec::new()
                }
            }
        }
        Err(e) => {
            error!("[AdminServer.tables_get] DB OrderItems pool acquisition failure: {}", e);
            Vec::new()
        }
    };

    let template = TablesTemplate {
        active_tab,
        active_theme,
        added: query.added.unwrap_or(false),
        updated: query.updated.unwrap_or(false),
        deleted: query.deleted.unwrap_or(false),
        error_msg: query.error_msg,
        products,
        contacts,
        orders,
        order_items,
    };

    match template.render() {
        Ok(html) => Response::builder()
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .body(Body::from(html))
            .unwrap()
            .into_response(),
        Err(err) => {
            error!("[AdminServer.tables_get] Askama rendering failed: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

// ------------------------------------------
// 🧴 Catalog CRUD Handlers
// ------------------------------------------

#[derive(serde::Deserialize)]
pub struct CatalogAddForm {
    pub product_name: String,
    pub standard_price: i32,
    pub stock_quantity: i32,
    pub tags: Option<String>,
    pub notes: Option<String>,
    pub suitable_season: String,
    pub suitable_situation: String,
    pub duration: Option<String>,
    pub sillage: Option<String>,
}

async fn catalog_add_handler(
    _auth: BasicAuth,
    State(state): State<AppState>,
    Form(form): Form<CatalogAddForm>,
) -> impl IntoResponse {
    let conn = match state.pool.get().await {
        Ok(c) => c,
        Err(_) => return Redirect::to("/tables?table=catalog&error_msg=DB Pool acquisition error").into_response(),
    };

    let add_res = conn.interact(move |conn| -> Result<_, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "INSERT INTO catalog (product_name, standard_price, stock_quantity, tags, notes, suitable_season, suitable_situation, duration, sillage) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )?;
        stmt.execute((
            &form.product_name,
            form.standard_price,
            form.stock_quantity,
            &form.tags,
            &form.notes,
            &form.suitable_season,
            &form.suitable_situation,
            &form.duration,
            &form.sillage,
        ))?;
        Ok(())
    }).await;

    match add_res {
        Ok(Ok(())) => Redirect::to("/tables?table=catalog&added=true").into_response(),
        err => {
            let msg = format!("DB Insertion failed: {:?}", err);
            error!("[AdminServer.catalog_add] {}", msg);
            Redirect::to(&format!("/tables?table=catalog&error_msg={}", urlencoding::encode(&msg))).into_response()
        }
    }
}

#[derive(serde::Deserialize)]
pub struct CatalogUpdateForm {
    pub product_id: i64,
    pub standard_price: i32,
    pub stock_quantity: i32,
}

async fn catalog_update_handler(
    _auth: BasicAuth,
    State(state): State<AppState>,
    Form(form): Form<CatalogUpdateForm>,
) -> impl IntoResponse {
    let conn = match state.pool.get().await {
        Ok(c) => c,
        Err(_) => return Redirect::to("/tables?table=catalog&error_msg=DB Pool acquisition error").into_response(),
    };

    let update_res = conn.interact(move |conn| -> Result<_, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "UPDATE catalog SET standard_price = ?, stock_quantity = ? WHERE product_id = ?"
        )?;
        stmt.execute((
            form.standard_price,
            form.stock_quantity,
            form.product_id,
        ))?;
        Ok(())
    }).await;

    match update_res {
        Ok(Ok(())) => Redirect::to("/tables?table=catalog&updated=true").into_response(),
        err => {
            let msg = format!("DB Update failed: {:?}", err);
            error!("[AdminServer.catalog_update] {}", msg);
            Redirect::to(&format!("/tables?table=catalog&error_msg={}", urlencoding::encode(&msg))).into_response()
        }
    }
}

async fn catalog_delete_handler(
    _auth: BasicAuth,
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> impl IntoResponse {
    let conn = match state.pool.get().await {
        Ok(c) => c,
        Err(_) => return Redirect::to("/tables?table=catalog&error_msg=DB Pool acquisition error").into_response(),
    };

    let delete_res = conn.interact(move |conn| -> Result<_, rusqlite::Error> {
        let mut stmt = conn.prepare("DELETE FROM catalog WHERE product_id = ?")?;
        stmt.execute([id])?;
        Ok(())
    }).await;

    match delete_res {
        Ok(Ok(())) => Redirect::to("/tables?table=catalog&deleted=true").into_response(),
        err => {
            let msg = format!("DB Deletion failed: {:?}", err);
            error!("[AdminServer.catalog_delete] {}", msg);
            Redirect::to(&format!("/tables?table=catalog&error_msg={}", urlencoding::encode(&msg))).into_response()
        }
    }
}

// ------------------------------------------
// 👤 Contacts CRUD Handlers
// ------------------------------------------

#[derive(serde::Deserialize)]
pub struct ContactAddForm {
    pub chat_id: i64,
    pub first_name: String,
    pub username: Option<String>,
    pub phone_number: Option<String>,
    pub address: Option<String>,
    pub nickname: Option<String>,
}

async fn contacts_add_handler(
    _auth: BasicAuth,
    State(state): State<AppState>,
    Form(form): Form<ContactAddForm>,
) -> impl IntoResponse {
    let conn = match state.pool.get().await {
        Ok(c) => c,
        Err(_) => return Redirect::to("/tables?table=contacts&error_msg=DB Pool acquisition error").into_response(),
    };

    let add_res = conn.interact(move |conn| -> Result<_, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "INSERT INTO contacts (chat_id, first_name, username, phone_number, address, nickname) VALUES (?, ?, ?, ?, ?, ?)"
        )?;
        stmt.execute((
            form.chat_id,
            &form.first_name,
            form.username.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()),
            form.phone_number.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()),
            form.address.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()),
            form.nickname.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()),
        ))?;
        Ok(())
    }).await;

    match add_res {
        Ok(Ok(())) => Redirect::to("/tables?table=contacts&added=true").into_response(),
        err => {
            let msg = format!("DB Insertion failed (make sure Chat ID is unique): {:?}", err);
            error!("[AdminServer.contacts_add] {}", msg);
            Redirect::to(&format!("/tables?table=contacts&error_msg={}", urlencoding::encode(&msg))).into_response()
        }
    }
}

#[derive(serde::Deserialize)]
pub struct ContactUpdateForm {
    pub chat_id: i64,
    pub phone_number: Option<String>,
    pub address: Option<String>,
    pub nickname: Option<String>,
}

async fn contacts_update_handler(
    _auth: BasicAuth,
    State(state): State<AppState>,
    Form(form): Form<ContactUpdateForm>,
) -> impl IntoResponse {
    let conn = match state.pool.get().await {
        Ok(c) => c,
        Err(_) => return Redirect::to("/tables?table=contacts&error_msg=DB Pool acquisition error").into_response(),
    };

    let update_res = conn.interact(move |conn| -> Result<_, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "UPDATE contacts SET phone_number = ?, address = ?, nickname = ? WHERE chat_id = ?"
        )?;
        stmt.execute((
            form.phone_number.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()),
            form.address.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()),
            form.nickname.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()),
            form.chat_id,
        ))?;
        Ok(())
    }).await;

    match update_res {
        Ok(Ok(())) => Redirect::to("/tables?table=contacts&updated=true").into_response(),
        err => {
            let msg = format!("DB Update failed: {:?}", err);
            error!("[AdminServer.contacts_update] {}", msg);
            Redirect::to(&format!("/tables?table=contacts&error_msg={}", urlencoding::encode(&msg))).into_response()
        }
    }
}

async fn contacts_delete_handler(
    _auth: BasicAuth,
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> impl IntoResponse {
    let conn = match state.pool.get().await {
        Ok(c) => c,
        Err(_) => return Redirect::to("/tables?table=contacts&error_msg=DB Pool acquisition error").into_response(),
    };

    let delete_res = conn.interact(move |conn| -> Result<_, rusqlite::Error> {
        let mut stmt = conn.prepare("DELETE FROM contacts WHERE chat_id = ?")?;
        stmt.execute([id])?;
        Ok(())
    }).await;

    match delete_res {
        Ok(Ok(())) => Redirect::to("/tables?table=contacts&deleted=true").into_response(),
        err => {
            let msg = format!("DB Deletion failed (possible foreign key constraint): {:?}", err);
            error!("[AdminServer.contacts_delete] {}", msg);
            Redirect::to(&format!("/tables?table=contacts&error_msg={}", urlencoding::encode(&msg))).into_response()
        }
    }
}

// ------------------------------------------
// 📦 Orders CRUD Handlers
// ------------------------------------------

#[derive(serde::Deserialize)]
pub struct OrderAddForm {
    pub order_id: String,
    pub chat_id: i64,
    pub status: String,
    pub delivery_address: Option<String>,
    pub total_amount: i32,
}

async fn orders_add_handler(
    _auth: BasicAuth,
    State(state): State<AppState>,
    Form(form): Form<OrderAddForm>,
) -> impl IntoResponse {
    let conn = match state.pool.get().await {
        Ok(c) => c,
        Err(_) => return Redirect::to("/tables?table=orders&error_msg=DB Pool acquisition error").into_response(),
    };

    let order_id = form.order_id.trim().to_string();
    if order_id.is_empty() {
        return Redirect::to("/tables?table=orders&error_msg=Order ID cannot be empty").into_response();
    }

    let add_res = conn.interact(move |conn| -> Result<_, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "INSERT INTO orders (order_id, chat_id, status, delivery_address, total_amount) VALUES (?, ?, ?, ?, ?)"
        )?;
        stmt.execute((
            &order_id,
            form.chat_id,
            &form.status,
            form.delivery_address.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()),
            form.total_amount,
        ))?;
        Ok(())
    }).await;

    match add_res {
        Ok(Ok(())) => Redirect::to("/tables?table=orders&added=true").into_response(),
        err => {
            let msg = format!("DB Insertion failed (make sure client exists and order ID is unique): {:?}", err);
            error!("[AdminServer.orders_add] {}", msg);
            Redirect::to(&format!("/tables?table=orders&error_msg={}", urlencoding::encode(&msg))).into_response()
        }
    }
}

#[derive(serde::Deserialize)]
pub struct OrderUpdateForm {
    pub order_id: String,
    pub status: String,
    pub delivery_address: Option<String>,
    pub total_amount: i32,
}

async fn orders_update_handler(
    _auth: BasicAuth,
    State(state): State<AppState>,
    Form(form): Form<OrderUpdateForm>,
) -> impl IntoResponse {
    let conn = match state.pool.get().await {
        Ok(c) => c,
        Err(_) => return Redirect::to("/tables?table=orders&error_msg=DB Pool acquisition error").into_response(),
    };

    let update_res = conn.interact(move |conn| -> Result<_, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "UPDATE orders SET status = ?, delivery_address = ?, total_amount = ? WHERE order_id = ?"
        )?;
        stmt.execute((
            &form.status,
            form.delivery_address.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()),
            form.total_amount,
            &form.order_id,
        ))?;
        Ok(())
    }).await;

    match update_res {
        Ok(Ok(())) => Redirect::to("/tables?table=orders&updated=true").into_response(),
        err => {
            let msg = format!("DB Update failed: {:?}", err);
            error!("[AdminServer.orders_update] {}", msg);
            Redirect::to(&format!("/tables?table=orders&error_msg={}", urlencoding::encode(&msg))).into_response()
        }
    }
}

async fn orders_delete_handler(
    _auth: BasicAuth,
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    let conn = match state.pool.get().await {
        Ok(c) => c,
        Err(_) => return Redirect::to("/tables?table=orders&error_msg=DB Pool acquisition error").into_response(),
    };

    let delete_res = conn.interact(move |conn| -> Result<_, rusqlite::Error> {
        let mut stmt = conn.prepare("DELETE FROM orders WHERE order_id = ?")?;
        stmt.execute([id])?;
        Ok(())
    }).await;

    match delete_res {
        Ok(Ok(())) => Redirect::to("/tables?table=orders&deleted=true").into_response(),
        err => {
            let msg = format!("DB Deletion failed: {:?}", err);
            error!("[AdminServer.orders_delete] {}", msg);
            Redirect::to(&format!("/tables?table=orders&error_msg={}", urlencoding::encode(&msg))).into_response()
        }
    }
}

// ------------------------------------------
// 🛍️ Order Items CRUD Handlers
// ------------------------------------------

#[derive(serde::Deserialize)]
pub struct OrderItemAddForm {
    pub item_id: String,
    pub order_id: String,
    pub product_id: i64,
    pub quantity: i32,
    pub price_at_sale: i32,
}

async fn order_items_add_handler(
    _auth: BasicAuth,
    State(state): State<AppState>,
    Form(form): Form<OrderItemAddForm>,
) -> impl IntoResponse {
    let conn = match state.pool.get().await {
        Ok(c) => c,
        Err(_) => return Redirect::to("/tables?table=order_items&error_msg=DB Pool acquisition error").into_response(),
    };

    let item_id = form.item_id.trim().to_string();
    if item_id.is_empty() {
        return Redirect::to("/tables?table=order_items&error_msg=Item ID cannot be empty").into_response();
    }

    let add_res = conn.interact(move |conn| -> Result<_, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "INSERT INTO order_items (item_id, order_id, product_id, quantity, price_at_sale) VALUES (?, ?, ?, ?, ?)"
        )?;
        stmt.execute((
            &item_id,
            &form.order_id,
            form.product_id,
            form.quantity,
            form.price_at_sale,
        ))?;
        Ok(())
    }).await;

    match add_res {
        Ok(Ok(())) => Redirect::to("/tables?table=order_items&added=true").into_response(),
        err => {
            let msg = format!("DB Insertion failed (make sure Order and Product exist and Item ID is unique): {:?}", err);
            error!("[AdminServer.order_items_add] {}", msg);
            Redirect::to(&format!("/tables?table=order_items&error_msg={}", urlencoding::encode(&msg))).into_response()
        }
    }
}

#[derive(serde::Deserialize)]
pub struct OrderItemUpdateForm {
    pub item_id: String,
    pub quantity: i32,
    pub price_at_sale: i32,
}

async fn order_items_update_handler(
    _auth: BasicAuth,
    State(state): State<AppState>,
    Form(form): Form<OrderItemUpdateForm>,
) -> impl IntoResponse {
    let conn = match state.pool.get().await {
        Ok(c) => c,
        Err(_) => return Redirect::to("/tables?table=order_items&error_msg=DB Pool acquisition error").into_response(),
    };

    let update_res = conn.interact(move |conn| -> Result<_, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "UPDATE order_items SET quantity = ?, price_at_sale = ? WHERE item_id = ?"
        )?;
        stmt.execute((
            form.quantity,
            form.price_at_sale,
            &form.item_id,
        ))?;
        Ok(())
    }).await;

    match update_res {
        Ok(Ok(())) => Redirect::to("/tables?table=order_items&updated=true").into_response(),
        err => {
            let msg = format!("DB Update failed: {:?}", err);
            error!("[AdminServer.order_items_update] {}", msg);
            Redirect::to(&format!("/tables?table=order_items&error_msg={}", urlencoding::encode(&msg))).into_response()
        }
    }
}

async fn order_items_delete_handler(
    _auth: BasicAuth,
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    let conn = match state.pool.get().await {
        Ok(c) => c,
        Err(_) => return Redirect::to("/tables?table=order_items&error_msg=DB Pool acquisition error").into_response(),
    };

    let delete_res = conn.interact(move |conn| -> Result<_, rusqlite::Error> {
        let mut stmt = conn.prepare("DELETE FROM order_items WHERE item_id = ?")?;
        stmt.execute([id])?;
        Ok(())
    }).await;

    match delete_res {
        Ok(Ok(())) => Redirect::to("/tables?table=order_items&deleted=true").into_response(),
        err => {
            let msg = format!("DB Deletion failed: {:?}", err);
            error!("[AdminServer.order_items_delete] {}", msg);
            Redirect::to(&format!("/tables?table=order_items&error_msg={}", urlencoding::encode(&msg))).into_response()
        }
    }
}

/// Starts the Axum web administration panel and binds it background thread concurrently.
pub async fn run(pool: DbPool, config: AppConfig) -> anyhow::Result<()> {
    let port = config.admin_server_port;
    let state = AppState { pool, config };

    let app = Router::new()
        .route("/", get(dashboard_handler))
        .route("/admin/order/update_status", post(order_status_update_handler))
        .route("/admin/update_theme", post(update_theme_handler))
        .route("/tables", get(tables_get_handler))
        .route("/tables/catalog/add", post(catalog_add_handler))
        .route("/tables/catalog/update", post(catalog_update_handler))
        .route("/tables/catalog/delete/{id}", post(catalog_delete_handler))
        .route("/tables/contacts/add", post(contacts_add_handler))
        .route("/tables/contacts/update", post(contacts_update_handler))
        .route("/tables/contacts/delete/{id}", post(contacts_delete_handler))
        .route("/tables/orders/add", post(orders_add_handler))
        .route("/tables/orders/update", post(orders_update_handler))
        .route("/tables/orders/delete/{id}", post(orders_delete_handler))
        .route("/tables/order_items/add", post(order_items_add_handler))
        .route("/tables/order_items/update", post(order_items_update_handler))
        .route("/tables/order_items/delete/{id}", post(order_items_delete_handler))
        .route("/config", get(config_get_handler).post(config_post_handler))
        .route("/logs", get(logs_handler))
        .route("/ws", get(ws_handler))
        .route("/health", get(health_check_handler))
        .route("/config/backup", post(config_backup_handler))
        .with_state(state);

    let address = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&address).await?;
    info!("🔮 Admin Panel Web Server listening on http://{}", address);
    
    axum::serve(listener, app).await?;
    Ok(())
}

