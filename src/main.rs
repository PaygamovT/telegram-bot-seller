// Composition Root for Telegram Bot Seller application

use tracing::{debug, error, info};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};
use telegram_bot_seller::shared::config::AppConfig;
use telegram_bot_seller::shared::{alerting, db, seed};
use telegram_bot_seller::modules;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Initialize tracing log broadcaster and custom Layer
    let _ = modules::admin::server::START_TIME.set(std::time::Instant::now());
    let (log_tx, _) = tokio::sync::broadcast::channel::<String>(1024);
    let _ = modules::admin::server::LOG_BROADCASTER.set(log_tx.clone());
    let ws_log_layer = modules::admin::server::WsLogLayer::new(log_tx);

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(ws_log_layer)
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug")))
        .init();

    info!(
        "🚀 telegram-bot-seller v{} starting...",
        env!("CARGO_PKG_VERSION")
    );

    // 2. Load Configuration
    let config = AppConfig::load()?;

    // 3. Initialize pooled SQLite Database
    let db_pool = db::init(&config.database_path).await?;
    db::run_migrations(&db_pool).await?;

    // 4. Run database seeding
    seed::seed_database(&db_pool).await?;

    // 5. Initialize Panic Hook Alerting
    alerting::install_panic_hook(config.admin_chat_id, &config.telegram_token);

    // 6. Spawn Telegram Bot background task
    info!("🤖 Starting Telegram Bot polling engine...");
    let bot_handle = tokio::spawn(modules::telegram::run(db_pool.clone(), config.clone()));

    // 6b. Spawn Admin Panel Web Server background task
    info!("🔮 Starting Admin Panel Web Server...");
    let admin_handle = tokio::spawn(modules::admin::run(db_pool.clone(), config.clone()));

    info!("🚀 Application started successfully! Press Ctrl+C to terminate.");

    // 7. Await termination signal or background task failures
    tokio::select! {
        res = bot_handle => {
            match res {
                Ok(Ok(())) => info!("Telegram bot engine stopped successfully."),
                Ok(Err(err)) => error!("Telegram bot engine stopped with error: {:?}", err),
                Err(err) => error!("Telegram bot task aborted: {:?}", err),
            }
        }
        res = admin_handle => {
            match res {
                Ok(Ok(())) => info!("Admin panel stopped successfully."),
                Ok(Err(err)) => error!("Admin panel stopped with error: {:?}", err),
                Err(err) => error!("Admin panel task aborted: {:?}", err),
            }
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Ctrl+C received. Terminating application gracefully...");
        }
    }

    info!("👋 Shutdown complete. Bye!");
    Ok(())
}
