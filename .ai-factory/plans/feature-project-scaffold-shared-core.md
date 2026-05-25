# Implementation Plan: Project Scaffold & Shared Core

Branch: `feature/project-scaffold-shared-core`
Created: 2026-05-26

## Settings
- Testing: yes
- Logging: verbose
- Docs: yes

## Commit Plan

- **Commit 1** (after tasks 1–3): `feat: initialize Cargo project with dependencies and module skeleton`
- **Commit 2** (after tasks 4–6): `feat: implement shared core — config, error types, newtype wrappers`
- **Commit 3** (after tasks 7–8): `feat: add SQLite database layer with connection pool and migrations`
- **Commit 4** (after tasks 9–10): `test: add unit tests for shared core and DB layer`

## Tasks

### Phase 1: Cargo Project Initialization

- [x] **Task 1: Initialize Cargo project and Cargo.toml**

  Create `Cargo.toml` at the project root with all required dependencies, project metadata, and release profile optimized for ARM64.

  **Deliverables:**
  - `Cargo.toml` with:
    ```toml
    [package]
    name = "telegram-bot-seller"
    version = "0.1.0"
    edition = "2024"
    rust-version = "1.85"
    description = "Autonomous Telegram Personal Bot & Seller (perfumery)"

    [dependencies]
    tokio = { version = "1.52", features = ["full"] }
    rusqlite = { version = "0.39", features = ["bundled"] }
    deadpool-sqlite = { version = "0.13", features = ["rt_tokio_1"] }
    axum = "0.8"
    askama = "0.16"
    reqwest = { version = "0.13", features = ["json"] }
    serde = { version = "1.0", features = ["derive"] }
    serde_json = "1.0"
    thiserror = "2.0"
    anyhow = "1.0"
    tracing = "0.1"
    tracing-subscriber = { version = "0.3", features = ["env-filter"] }
    tokio-tungstenite = "0.29"
    dotenvy = "0.15"

    [profile.release]
    opt-level = "z"
    lto = true
    strip = true
    codegen-units = 1
    panic = "abort"
    ```
  - `.gitignore` for Rust project (`/target`, `.env`, `*.db`)

  **LOGGING REQUIREMENTS:**
  - N/A — configuration file only

  **Files:** `Cargo.toml`, `.gitignore`

---

- [x] **Task 2: Create module skeleton (mod.rs tree)**

  Create the full directory structure and empty `mod.rs` files following the Modular Monolith architecture from `ARCHITECTURE.md`.

  **Deliverables:**
  - `src/main.rs` — composition root stub with `mod modules;` and `mod shared;`; basic `#[tokio::main]` entry point that initializes tracing and logs startup
  - `src/modules/mod.rs` — re-exports all modules: `pub mod telegram; pub mod ai; pub mod catalog; pub mod contacts; pub mod orders; pub mod media_manager; pub mod admin;`
  - `src/modules/telegram/mod.rs` — empty module stub with `// TODO: Milestone 4`
  - `src/modules/ai/mod.rs` — empty module stub with `// TODO: Milestone 5-6`
  - `src/modules/catalog/mod.rs` — empty module stub with `// TODO: Milestone 3`
  - `src/modules/contacts/mod.rs` — empty module stub with `// TODO: Milestone 3`
  - `src/modules/orders/mod.rs` — empty module stub with `// TODO: Milestone 3`
  - `src/modules/media_manager/mod.rs` — empty module stub with `// TODO: Milestone 3`
  - `src/modules/admin/mod.rs` — empty module stub with `// TODO: Milestone 9-10`
  - `src/shared/mod.rs` — re-exports: `pub mod config; pub mod db; pub mod error; pub mod types; pub mod alerting;`

  **LOGGING REQUIREMENTS:**
  - `main.rs`: Initialize `tracing_subscriber` with `EnvFilter` from `RUST_LOG` env var (default: `DEBUG`)
  - `main.rs`: Log `info!("🚀 telegram-bot-seller v{} starting...", env!("CARGO_PKG_VERSION"))`
  - `main.rs`: Log `debug!("Configuration loaded, modules initialized")`

  **Files:** `src/main.rs`, `src/modules/mod.rs`, `src/modules/*/mod.rs`, `src/shared/mod.rs`

---

- [x] **Task 3: Create .env.example and environment setup**

  Create an environment file template with all required configuration keys.

  **Deliverables:**
  - `.env.example` with:
    ```env
    # Telegram
    TELEGRAM_BOT_TOKEN=
    ADMIN_CHAT_ID=

    # AI APIs
    MINIMAX_API_KEY=
    MINIMAX_GROUP_ID=
    GEMINI_API_KEY=
    OPENROUTER_API_KEY=

    # Database
    DATABASE_PATH=./data/bot.db

    # Server
    ADMIN_SERVER_PORT=8080

    # Logging
    RUST_LOG=debug
    ```
  - `data/` directory (empty, for SQLite DB file)

  **LOGGING REQUIREMENTS:**
  - N/A — template file only

  **Files:** `.env.example`, `data/.gitkeep`

<!-- Commit checkpoint: tasks 1-3 → "feat: initialize Cargo project with dependencies and module skeleton" -->

---

### Phase 2: Shared Core Implementation

- [x] **Task 4: Implement shared::config — configuration loading**

  Create `AppConfig` struct that loads settings from environment variables (via `dotenvy`) with validation.

  **Deliverables:**
  - `src/shared/config.rs`:
    - `pub struct AppConfig` with fields: `telegram_token: String`, `admin_chat_id: i64`, `minimax_api_key: String`, `minimax_group_id: String`, `gemini_api_key: String`, `openrouter_api_key: Option<String>`, `database_path: String`, `admin_server_port: u16`, `rust_log: String`
    - `impl AppConfig { pub fn load() -> anyhow::Result<Self> }` — reads `.env` via `dotenvy::dotenv().ok()`, loads all keys via `std::env::var()`, validates required keys are non-empty
    - `impl Clone for AppConfig`

  **LOGGING REQUIREMENTS:**
  - Log `debug!("[Config.load] Loading configuration from environment")`
  - Log `debug!("[Config.load] DATABASE_PATH={}", self.database_path)` for each non-secret field
  - Log `info!("[Config.load] Configuration loaded successfully")` on success
  - Log `warn!("[Config.load] OPENROUTER_API_KEY not set, Gemini-only mode")` if optional key missing
  - **NEVER log API keys or tokens** — only log key names and whether they're set

  **Files:** `src/shared/config.rs`

---

- [x] **Task 5: Implement shared::error — unified error types**

  Create the application-wide error enum with `thiserror` and a convenience `AppResult<T>` type alias.

  **Deliverables:**
  - `src/shared/error.rs`:
    - `#[derive(Debug, thiserror::Error)] pub enum AppError`:
      - `#[error("Database error: {0}")] Database(#[from] rusqlite::Error)`
      - `#[error("Database pool error: {0}")] Pool(#[from] deadpool_sqlite::PoolError)`
      - `#[error("HTTP request error: {0}")] Http(#[from] reqwest::Error)`
      - `#[error("JSON error: {0}")] Json(#[from] serde_json::Error)`
      - `#[error("Configuration error: {0}")] Config(String)`
      - `#[error("Telegram API error: {0}")] Telegram(String)`
      - `#[error("AI API error: {0}")] AiApi(String)`
      - `#[error("Unknown tool: {0}")] UnknownTool(String)`
      - `#[error("Validation error: {0}")] Validation(String)`
      - `#[error("IO error: {0}")] Io(#[from] std::io::Error)`
    - `pub type AppResult<T> = Result<T, AppError>;`
    - `impl axum::response::IntoResponse for AppError` — maps errors to appropriate HTTP status codes for the admin panel

  **LOGGING REQUIREMENTS:**
  - `IntoResponse` impl: `error!("[AppError] HTTP {status}: {self}")` before returning response
  - Each variant should produce a meaningful error message via `Display`

  **Files:** `src/shared/error.rs`

---

- [x] **Task 6: Implement shared::types — newtype wrappers**

  Create strongly-typed ID wrappers for domain identifiers, preventing accidental misuse of raw integers/strings.

  **Deliverables:**
  - `src/shared/types.rs`:
    - `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)] pub struct ChatId(pub i64);`
    - `#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)] pub struct OrderId(pub String);`
    - `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)] pub struct ProductId(pub i64);`
    - `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)] pub struct ItemId(pub i64);`
    - `impl fmt::Display` for each type
    - `impl From<i64>` for numeric types, `impl From<String>` for `OrderId`
    - `impl OrderId { pub fn generate() -> Self }` — generates ID from current timestamp milliseconds (`SystemTime::now()`)

  **LOGGING REQUIREMENTS:**
  - `OrderId::generate()`: `debug!("[OrderId.generate] Generated new order ID: {id}")`

  **Files:** `src/shared/types.rs`

<!-- Commit checkpoint: tasks 4-6 → "feat: implement shared core — config, error types, newtype wrappers" -->

---

### Phase 3: Database Layer

- [x] **Task 7: Implement shared::db — SQLite connection pool and initialization**

  Set up the database connection pool using `deadpool-sqlite` and provide an `init` function.

  **Deliverables:**
  - `src/shared/db.rs`:
    - `pub type DbPool = deadpool_sqlite::Pool;`
    - `pub async fn init(db_path: &str) -> AppResult<DbPool>` — creates parent directories if needed, creates `deadpool_sqlite::Config` with `path` set, builds pool with `Runtime::Tokio1`, verifies connectivity with a test query
    - `pub async fn run_migrations(pool: &DbPool) -> AppResult<()>` — gets a connection from pool, reads and executes SQL files from embedded migrations (use `include_str!` macro for `migrations/001_init.sql`)
    - WAL mode enabled: `PRAGMA journal_mode=WAL;`
    - Foreign keys enabled: `PRAGMA foreign_keys=ON;`

  **LOGGING REQUIREMENTS:**
  - `init()`: `info!("[DB.init] Initializing SQLite at {db_path}")`
  - `init()`: `debug!("[DB.init] Pool created, verifying connectivity...")`
  - `init()`: `info!("[DB.init] Database connection pool ready")`
  - `run_migrations()`: `info!("[DB.run_migrations] Running database migrations...")`
  - `run_migrations()`: `debug!("[DB.run_migrations] Executing migration: 001_init.sql")`
  - `run_migrations()`: `info!("[DB.run_migrations] Migrations completed successfully")`
  - On error: `error!("[DB.init] Failed to initialize database: {err}")`

  **Files:** `src/shared/db.rs`

---

- [x] **Task 8: Create SQL migration 001_init.sql**

  Create the initial migration with all 6 tables from the concept_summary schema.

  **Deliverables:**
  - `src/migrations/001_init.sql`:
    - `PRAGMA journal_mode=WAL;`
    - `PRAGMA foreign_keys=ON;`
    - `CREATE TABLE IF NOT EXISTS contacts` — exact schema from concept_summary
    - `CREATE TABLE IF NOT EXISTS catalog` — exact schema from concept_summary
    - `CREATE TABLE IF NOT EXISTS orders` — exact schema from concept_summary
    - `CREATE TABLE IF NOT EXISTS order_items` — exact schema with `ON DELETE CASCADE`
    - `CREATE TABLE IF NOT EXISTS settings` — key/value store
    - `CREATE TABLE IF NOT EXISTS agent_media` — media files with `is_allowed_for_ai`
    - Indexes: `CREATE INDEX idx_orders_chat_id ON orders(chat_id);`, `CREATE INDEX idx_order_items_order_id ON order_items(order_id);`

  **LOGGING REQUIREMENTS:**
  - N/A — SQL file only; logging is handled by `shared::db::run_migrations()`

  **Files:** `src/migrations/001_init.sql`

<!-- Commit checkpoint: tasks 7-8 → "feat: add SQLite database layer with connection pool and migrations" -->

---

### Phase 4: Alerting & Tests

- [ ] **Task 9: Implement shared::alerting — panic hook and admin notifications**

  Create a minimal alerting stub that captures panics and logs them. The actual Telegram notification will be wired when the Telegram module is implemented (Milestone 4).

  **Deliverables:**
  - `src/shared/alerting.rs`:
    - `pub fn install_panic_hook(admin_chat_id: i64, telegram_token: &str)` — sets `std::panic::set_hook` that logs the panic message with full backtrace via `error!`; stores `admin_chat_id` and `telegram_token` in static `OnceLock<AlertConfig>` for future Telegram integration
    - `pub async fn send_alert(message: &str) -> AppResult<()>` — currently logs `warn!("[Alerting] ALERT: {message}")` with a TODO comment for Telegram integration in Milestone 4
    - `struct AlertConfig { admin_chat_id: i64, telegram_token: String }`

  **LOGGING REQUIREMENTS:**
  - `install_panic_hook()`: `info!("[Alerting.install_panic_hook] Panic hook installed for chat_id={admin_chat_id}")`
  - Panic hook closure: `error!("[PANIC] {panic_info} at {location}")` — include file, line, column
  - `send_alert()`: `warn!("[Alerting.send_alert] {message}")`

  **Files:** `src/shared/alerting.rs`

---

- [ ] **Task 10: Write unit tests for shared core**

  Create comprehensive tests for config, types, error, and DB modules.

  **Deliverables:**
  - `tests/shared_config_test.rs`:
    - Test `AppConfig::load()` with all env vars set → succeeds
    - Test `AppConfig::load()` with missing required var → fails with meaningful error
    - Test `AppConfig::load()` with optional `OPENROUTER_API_KEY` missing → succeeds with `None`
  - `tests/shared_types_test.rs`:
    - Test `ChatId`, `ProductId` creation and display
    - Test `OrderId::generate()` returns unique IDs
    - Test `OrderId::generate()` format is numeric string (timestamp millis)
    - Test serialization/deserialization roundtrip with serde_json
  - `tests/shared_db_test.rs`:
    - Test `db::init()` creates DB file at specified path
    - Test `db::run_migrations()` creates all 6 tables
    - Test basic INSERT + SELECT on `contacts` table
    - Test foreign key constraint on `order_items.order_id`
    - Use temporary directory (`std::env::temp_dir()`) for test DB files
  - `tests/shared_error_test.rs`:
    - Test `AppError::Display` for each variant
    - Test `From` conversions (rusqlite::Error → AppError, etc.)

  **LOGGING REQUIREMENTS:**
  - Use `tracing_subscriber` in test setup with `RUST_LOG=debug` filter for debug output
  - Each test: `debug!("[Test] Running: {test_name}")`

  **Files:** `tests/shared_config_test.rs`, `tests/shared_types_test.rs`, `tests/shared_db_test.rs`, `tests/shared_error_test.rs`

<!-- Commit checkpoint: tasks 9-10 → "test: add unit tests for shared core and DB layer" -->
