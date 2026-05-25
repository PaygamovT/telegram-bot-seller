# Telegram Bot Seller

> Autonomous Telegram Personal Bot & Seller (perfumery) built on a modular monolith architecture in Rust.

`telegram-bot-seller` is a high-performance, autonomous agentic sales bot tailored for automated catalog browsing, ordering, and customer communication, alongside a web-based Admin Panel. It is fully built using Rust, Axum, SQLite, and cutting-edge LLMs (Gemini, OpenRouter, MiniMax).

---

## Quick Start

### 1. Clone & Setup Environment
```bash
git clone https://github.com/PaygamovT/telegram-bot-seller.git
cd telegram-bot-seller
cp .env.example .env
```

### 2. Run the Bot
```bash
cargo run
```

---

## Key Features

*   **Modular Monolith Architecture**: Separated core services and domain-specific modules (`telegram`, `ai`, `catalog`, `orders`, etc.).
*   **Asynchronous Core**: Powered by Tokio runtime and Axum web framework.
*   **Built-in SQLite Pool**: SQLite connection pooling using `deadpool-sqlite` with automatic WAL and foreign-key configurations.
*   **Unified Error Handling**: Comprehensive domain-specific errors generated using `thiserror`.
*   **Autonomous Agentic Sales**: Deep integrations with LLMs (Gemini, OpenRouter, MiniMax) for natural language product recommendations and sales flow.

---

## Code Example

A clean, unified approach to starting up the shared database pool and server:

```rust
use telegram_bot_seller::shared::{config::AppConfig, db, alerting};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize Panic Hook Alerting
    alerting::setup_panic_hook();

    // 2. Load Configuration
    let config = AppConfig::load()?;

    // 3. Setuppooled SQLite Database & Run Migrations
    let pool = db::create_pool(&config).await?;
    db::run_migrations(&pool).await?;

    println!("Database initialized and migrated successfully!");
    Ok(())
}
```

---

## Documentation

| Guide | Description |
|-------|-------------|
| [Getting Started](docs/getting-started.md) | Prerequisites, step-by-step installation, and testing. |
| [Architecture Overview](docs/architecture.md) | Modular monolith layout, boundaries, and shared core. |
| [Configuration](docs/configuration.md) | Complete environment variable and configuration reference. |
| [Database Reference](docs/database.md) | Database schema details, SQLite configurations, and migrations. |

---

## License

This project is licensed under the MIT License.
