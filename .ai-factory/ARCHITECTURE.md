# Architecture: Modular Monolith

## Overview

Telegram Personal Bot & Seller — нативное Rust-приложение, работающее на смартфоне (ARM64/Debian Chroot). Приложение объединяет Telegram Business API, двухэтапный мультимодальный ИИ-конвейер (Gemini → MiniMax), базу данных SQLite и веб-панель управления (Axum + Askama) в единую автономную систему.

Выбрана архитектура **Modular Monolith**, так как:
- Проект выполняется небольшой командой (1–3 человека).
- Все компоненты работают на одном устройстве — независимое масштабирование или деплой не нужны.
- Домены (каталог, заказы, ИИ, Telegram) чётко разграничены, но тесно связаны через единую SQLite-базу.
- Модульная структура даёт чистые границы между доменами при минимальных накладных расходах, а в будущем позволит извлечь модули в отдельные crate-ы или сервисы.

## Decision Rationale

- **Project type:** Автономный Telegram-бот + продавец с ИИ-ассистентом, работающий на мобильном устройстве.
- **Tech stack:** Rust, SQLite, Axum, Askama, tokio, reqwest; API: Telegram Bot, MiniMax, Google Gemini/OpenRouter.
- **Key factor:** Единый бинарник для ARM64, простота деплоя, чёткое разделение доменов при сохранении общей БД.

## Folder Structure

```
src/
├── main.rs                        # Composition root: инициализация, graceful shutdown
│
├── modules/
│   ├── telegram/                  # Модуль Telegram
│   │   ├── mod.rs                 # Публичный API модуля (re-exports)
│   │   ├── bot.rs                 # Long-polling / webhook, обработка Update
│   │   ├── handlers.rs            # Маршрутизация входящих сообщений
│   │   ├── business.rs            # Telegram Business API: реакции, кастомные эмодзи
│   │   └── media.rs               # Скачивание файлов (фото, голос .ogg)
│   │
│   ├── ai/                        # Модуль ИИ-конвейера
│   │   ├── mod.rs                 # Публичный API модуля
│   │   ├── pipeline.rs            # Двухэтапный конвейер: распознавание → диалог
│   │   ├── gemini.rs              # Клиент Google Gemini (voice STT, image OCR)
│   │   ├── minimax.rs             # Клиент MiniMax (chat, tool_call)
│   │   └── tools.rs               # Определения инструментов (функций) для MiniMax
│   │
│   ├── catalog/                   # Модуль каталога товаров
│   │   ├── mod.rs                 # Публичный API: get_catalog, update_catalog
│   │   ├── domain.rs              # Структуры: Product, Tag, Season, Sillage
│   │   └── repo.rs                # SQL-запросы к таблице catalog
│   │
│   ├── contacts/                  # Модуль контактов / клиентов
│   │   ├── mod.rs                 # Публичный API: get_contact, update_contact
│   │   ├── domain.rs              # Структуры: Contact
│   │   └── repo.rs                # SQL-запросы к таблице contacts
│   │
│   ├── orders/                    # Модуль заказов
│   │   ├── mod.rs                 # Публичный API: insert_order, get_orders, update_order
│   │   ├── domain.rs              # Структуры: Order, OrderItem, OrderStatus
│   │   └── repo.rs                # SQL-запросы к таблицам orders, order_items
│   │
│   ├── media_manager/             # Модуль управления медиа-файлами агента
│   │   ├── mod.rs                 # Публичный API: get_media, upload_media
│   │   ├── domain.rs              # Структуры: AgentMedia
│   │   └── repo.rs                # SQL-запросы к таблице agent_media
│   │
│   └── admin/                     # Модуль веб-панели (Axum + Askama)
│       ├── mod.rs                 # Публичный API: роутер Axum
│       ├── routes.rs              # HTTP-эндпоинты: dashboard, config, logs, media
│       ├── templates/             # Askama HTML-шаблоны
│       │   ├── base.html
│       │   ├── dashboard.html
│       │   ├── config.html
│       │   ├── logs.html
│       │   └── media.html
│       └── ws.rs                  # WebSocket: лог-стрим в реальном времени
│
├── shared/                        # Общие утилиты и ядро
│   ├── mod.rs
│   ├── db.rs                      # Инициализация SQLite (rusqlite), пул соединений
│   ├── config.rs                  # Загрузка настроек (из таблицы settings + env)
│   ├── error.rs                   # Единый тип ошибки (thiserror), маппинг в HTTP/Telegram
│   ├── alerting.rs                # Отправка алёртов администратору (panic hook, ошибки API)
│   └── types.rs                   # Общие типы: ChatId, OrderId, ProductId (newtype wrappers)
│
├── migrations/                    # SQL-миграции для SQLite
│   ├── 001_init.sql
│   └── ...
│
└── Cargo.toml                     # Единый crate, features для включения/отключения модулей
```

## Dependency Rules

Модули взаимодействуют через свои публичные API (`mod.rs`), а не через прямой доступ к внутренним файлам.

- ✅ `modules::telegram` → `modules::ai` (передаёт входящие сообщения в конвейер)
- ✅ `modules::ai` → `modules::catalog`, `modules::contacts`, `modules::orders`, `modules::media_manager` (вызов инструментов по запросу MiniMax)
- ✅ `modules::admin` → все `modules::*` (чтение данных для дашборда и управления)
- ✅ Любой модуль → `shared::*` (БД, конфиг, типы, ошибки)
- ❌ `modules::catalog` → `modules::orders` (доменные модули НЕ зависят друг от друга напрямую)
- ❌ `modules::contacts` → `modules::telegram` (доменные модули НЕ зависят от инфраструктурных)
- ❌ `shared::*` → любой `modules::*` (ядро не знает о модулях)
- ❌ Прямой `use modules::orders::repo::*` из другого модуля (только через `mod.rs`)

```
┌─────────────────────────────────────────────────────────────────┐
│                        main.rs (Composition Root)               │
│   Инициализирует все модули, передаёт зависимости              │
└──────────┬──────────┬──────────┬──────────┬──────────┬──────────┘
           │          │          │          │          │
     ┌─────▼───┐ ┌───▼────┐ ┌──▼───┐ ┌───▼───┐ ┌───▼────┐
     │telegram │ │  ai    │ │admin │ │orders │ │catalog │ ...
     │         │ │pipeline│ │(axum)│ │       │ │        │
     └────┬────┘ └───┬────┘ └──┬───┘ └───┬───┘ └───┬────┘
          │          │         │         │         │
          └──────────┴─────────┴─────────┴─────────┘
                              │
                       ┌──────▼──────┐
                       │   shared/   │
                       │ db, config, │
                       │ error, types│
                       └─────────────┘
```

## Layer/Module Communication

- **Telegram → AI**: Модуль `telegram` получает `Update`, скачивает медиа при необходимости, и передаёт `IncomingMessage` в модуль `ai` через публичную функцию `ai::pipeline::process_message()`.
- **AI → Доменные модули**: Когда MiniMax вызывает `tool_call`, модуль `ai::tools` маршрутизирует вызов к соответствующему доменному модулю (`catalog::get_catalog()`, `orders::insert_order()` и т.д.) через их публичные API.
- **Admin → Все модули**: Веб-панель читает данные через публичные API модулей. Для WebSocket-логов используется `tokio::sync::broadcast` канал из `shared`.
- **Алёрты**: При критических ошибках любой модуль вызывает `shared::alerting::send_alert()`, который отправляет сообщение администратору через Telegram API.
- **Межмодульные события (при необходимости)**: Если в будущем потребуется реактивная связь между доменными модулями (например, «заказ создан → обновить остатки»), использовать `tokio::sync::broadcast` или `tokio::sync::mpsc` каналы, а не прямые вызовы.

## Key Principles

1. **Единый бинарник, модульное внутреннее устройство.** Приложение компилируется в один `aarch64` бинарник. Модули — это Rust-модули (`mod`), а не отдельные crate-ы (пока проект не вырастет).

2. **Публичный API через `mod.rs`.** Каждый модуль экспортирует только то, что нужно другим модулям. Внутренние `repo.rs`, `domain.rs` остаются приватными (`pub(crate)` при необходимости, но предпочитается `pub(super)`).

3. **Shared — только инфраструктурное ядро.** В `shared/` — только то, что используется двумя и более модулями: подключение к БД, конфиг, общие типы, обработка ошибок. Бизнес-логика в `shared` запрещена.

4. **Доменные модули изолированы.** `catalog`, `contacts`, `orders`, `media_manager` не знают друг о друге. Оркестрация — задача модулей `ai` и `admin`.

5. **Ресурсосбережение.** На Samsung Galaxy Flip 3 с 4 ГБ RAM каждый байт на счету. Избегать аллокаций, использовать `&str` вместо `String` где возможно, не держать открытыми лишние SQLite-соединения, ограничить количество tokio-задач.

## Code Examples

### Публичный API модуля (catalog/mod.rs)

```rust
// src/modules/catalog/mod.rs

mod domain;
mod repo;

// Реэкспортируем только публичный API
pub use domain::Product;

use crate::shared::db::DbPool;
use crate::shared::error::AppResult;

/// Получить полный каталог товаров
pub async fn get_catalog(db: &DbPool) -> AppResult<Vec<Product>> {
    repo::fetch_all(db).await
}

/// Обновить остаток товара на складе
pub async fn update_stock(db: &DbPool, product_id: i64, new_qty: i32) -> AppResult<()> {
    repo::update_quantity(db, product_id, new_qty).await
}
```

### Маршрутизация tool_call в AI-модуле (ai/tools.rs)

```rust
// src/modules/ai/tools.rs

use serde_json::Value;
use crate::shared::db::DbPool;
use crate::shared::error::AppResult;
use crate::modules::{catalog, contacts, orders};

/// Диспетчер инструментов MiniMax.
/// Вызывается, когда MiniMax возвращает tool_call в ответе.
pub async fn execute_tool(
    db: &DbPool,
    tool_name: &str,
    args: &Value,
    chat_id: i64,
) -> AppResult<Value> {
    match tool_name {
        "get_catalog" => {
            let products = catalog::get_catalog(db).await?;
            Ok(serde_json::to_value(products)?)
        }
        "get_contacts" => {
            let contact = contacts::get_contact(db, chat_id).await?;
            Ok(serde_json::to_value(contact)?)
        }
        "insert_order" => {
            let address = args["delivery_address"].as_str().unwrap_or_default();
            let total = args["total_amount"].as_i64().unwrap_or(0) as i32;
            let order = orders::insert_order(db, chat_id, address, total).await?;
            Ok(serde_json::to_value(order)?)
        }
        // ... остальные инструменты
        _ => Err(crate::shared::error::AppError::UnknownTool(tool_name.into())),
    }
}
```

### Правило границ модуля — Composition Root (main.rs)

```rust
// src/main.rs

mod modules;
mod shared;

use shared::config::AppConfig;
use shared::db;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Загрузка конфигурации
    let config = AppConfig::load()?;

    // 2. Инициализация БД и миграции
    let db_pool = db::init(&config.database_path).await?;
    db::run_migrations(&db_pool).await?;

    // 3. Установка panic hook для алёртов
    shared::alerting::install_panic_hook(config.admin_chat_id, &config.telegram_token);

    // 4. Запуск веб-панели (Axum) в фоне
    let admin_handle = tokio::spawn(
        modules::admin::start_server(db_pool.clone(), config.clone())
    );

    // 5. Запуск Telegram long-polling
    let bot_handle = tokio::spawn(
        modules::telegram::run(db_pool.clone(), config.clone())
    );

    // Graceful shutdown
    tokio::select! {
        res = admin_handle => res??,
        res = bot_handle => res??,
    }

    Ok(())
}
```

## Anti-Patterns

- ❌ **Прямой доступ к внутренностям модуля.** Никогда: `use modules::orders::repo::insert_raw`. Всегда через `modules::orders::insert_order()`.
- ❌ **Бизнес-логика в `shared/`.** Shared — инфраструктура (БД, конфиг, ошибки). Логика расчёта цен, валидация данных клиента — в доменных модулях.
- ❌ **Циклические зависимости между модулями.** Если `catalog` нужен `orders` и `orders` нужен `catalog` — вынести общую логику в `shared::types` или использовать event-канал.
- ❌ **God-module `ai`.** Модуль `ai` — оркестратор, но не должен содержать доменную логику. Валидация заказа — в `orders`, поиск товара — в `catalog`. AI только маршрутизирует.
- ❌ **Блокирующие вызовы в async-контексте.** SQLite через `rusqlite` — синхронный. Все вызовы оборачивать в `tokio::task::spawn_blocking()` или использовать `deadpool-sqlite`.
- ❌ **Хранение секретов в коде.** API-ключи (MiniMax, Gemini, Telegram) — только через таблицу `settings` или переменные окружения, но никогда не хардкод.
