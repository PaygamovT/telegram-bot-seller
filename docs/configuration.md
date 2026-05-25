[← Architecture Overview](architecture.md) · [Back to README](../README.md) · [Database Reference →](database.md)

# Configuration Guide

`telegram-bot-seller` manages its configuration via environment variables. In development, these are loaded from a `.env` file in the root of the project.

The application core strictly parses and validates these configurations on startup via the `shared::config::AppConfig` model.

---

## Supported Environment Variables

| Variable Name | Required | Default Value | Description |
| :--- | :--- | :--- | :--- |
| **`TELEGRAM_BOT_TOKEN`** | **Yes** | *None* | The authentication API token provided by Telegram's BotFather. |
| **`ADMIN_CHAT_ID`** | **Yes** | *None* | The Telegram Chat ID of the main administrator. Used for alerting, logs, and panic reports. |
| **`MINIMAX_API_KEY`** | No | *None* | API Key for MiniMax AI integrations. |
| **`MINIMAX_GROUP_ID`** | No | *None* | Group/Organization ID for MiniMax AI. |
| **`GEMINI_API_KEY`** | No | *None* | API Key for Google Gemini integrations. |
| **`OPENROUTER_API_KEY`**| No | *None* | API Key for OpenRouter LLM gateway integrations. |
| **`DATABASE_PATH`** | No | `./data/bot.db` | The path to the SQLite local database file. |
| **`ADMIN_SERVER_PORT`** | No | `8080` | Port for the Axum Web-based Admin Dashboard API. |
| **`RUST_LOG`** | No | `info` | Logging verbosity filter (supported values: `error`, `warn`, `info`, `debug`, `trace`). |

---

## Setup & Validation Flow

When you run `AppConfig::load()`, the following workflow executes:

1.  **Skip on Unit Tests**: If `TEST_ENV=true` is set, configuration loading will skip the `.env` loading and verification checks to prevent pollution of environment states during parallel unit tests.
2.  **dotenvy Loading**: In normal modes, the system attempts to load the local `.env` file from the workspace.
3.  **Strict Variable Validation**: The system parses crucial variables. If a required variable (like `TELEGRAM_BOT_TOKEN` or `ADMIN_CHAT_ID`) is missing, compilation will proceed but the application will panic gracefully on boot with an informative error listing the missing fields.
4.  **Admin Chat Parsing**: The `ADMIN_CHAT_ID` is strictly parsed into an `i64`. If it is invalid, an error is generated.

---

## Troubleshooting

*   **Missing `.env` File**: Ensure you copy `.env.example` to `.env` in the root folder before starting.
*   **Failed to parse Admin ID**: Ensure the `ADMIN_CHAT_ID` is a valid signed integer (no decimals or non-numeric characters). You can find your Telegram ID using bots like `@userinfobot`.

---

## See Also

*   [Getting Started](getting-started.md) — How to configure and start the system.
*   [Database Reference](database.md) — Details on where SQLite persists this data.
