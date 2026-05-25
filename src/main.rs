// Composition Root for Telegram Bot Seller application

use tracing::{debug, info};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug")))
        .init();

    info!(
        "🚀 telegram-bot-seller v{} starting...",
        env!("CARGO_PKG_VERSION")
    );
    debug!("Configuration loaded, modules initialized");

    Ok(())
}
