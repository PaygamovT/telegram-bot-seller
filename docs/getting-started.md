[Back to README](../README.md) · [Architecture Overview →](architecture.md)

# Getting Started

This guide walks you through setting up, running, and testing `telegram-bot-seller` locally on your system.

---

## Prerequisites

Before starting, ensure you have the following installed on your machine:

1.  **Rust Toolchain**: Rust 1.85+ (Stable).
    *   Install via [rustup](https://rustup.rs/): `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
2.  **SQLite**: The project uses SQLite for local data persistence.
    *   *(Note: The Rust engine compiles SQLite automatically via the `bundled` feature flag).*
3.  **Git**: For cloning the repository.

---

## Installation & Setup

Follow these steps to set up the project locally:

### 1. Clone the Repository
```bash
git clone https://github.com/PaygamovT/telegram-bot-seller.git
cd telegram-bot-seller
```

### 2. Configure the Environment
Copy the example environment file and fill in your custom tokens:
```bash
cp .env.example .env
```
Open the `.env` file and enter your configurations (e.g., your `TELEGRAM_BOT_TOKEN`, `ADMIN_CHAT_ID`, and LLM API keys). Refer to the [Configuration Guide](configuration.md) for full parameter details.

### 3. Run the Server & Bot
Compile and run the binary in development mode:
```bash
cargo run
```
The database file `bot.db` will be initialized automatically under `./data/` and all database migrations will run on startup.

---

## Running Verification

Ensure your development environment is fully working by executing the test suite:

```bash
cargo test
```

This runs the integration and unit tests for:
*   Configuration parsers and environment fallback logic.
*   Database pool creation, connection robustness, and schema migrations.
*   Unified domain error conversion.
*   Domain-specific strong typing and newtype wrappers.

---

## See Also

*   [Architecture Overview](architecture.md) — Learn how the project is structured.
*   [Configuration Reference](configuration.md) — View all supported environment variables.
*   [Database Reference](database.md) — Understand the database schema.
