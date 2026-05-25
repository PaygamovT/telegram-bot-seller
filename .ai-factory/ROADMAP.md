# Project Roadmap

> Автономный ИИ-ассистент и продавец парфюмерии в Telegram, работающий на смартфоне (Rust, ARM64, SQLite).

## Milestones

- [x] **Project Scaffold & Shared Core** — инициализация Cargo-проекта, модульная структура `src/modules/` + `src/shared/`, настройка `Cargo.toml` с зависимостями (tokio, rusqlite, axum, reqwest, serde, thiserror, askama), `shared::config`, `shared::error`, `shared::types` (newtype wrappers)
- [x] **SQLite Database & Migrations** — инициализация БД (`shared::db`), система миграций, таблицы `contacts`, `catalog`, `orders`, `order_items`, `settings`, `agent_media`; seed-данные из CSV; пул соединений через `deadpool-sqlite` или `spawn_blocking`
- [x] **Domain Modules (catalog, contacts, orders, media_manager)** — `domain.rs` + `repo.rs` + `mod.rs` для каждого доменного модуля; полные CRUD-операции; все 10 инструментов из concept_summary (get_contacts, update_contacts, get_catalog, update_catalog, insert_order, insert_order_items, get_orders, get_order_items, update_order, update_order_items)
- [x] **Telegram Bot Module (long-polling & Business API)** — подключение к Telegram Bot API, long-polling цикл, парсинг `Update` / `BusinessMessage`, маршрутизация по типу контента (текст / фото / голос), отправка ответов, реакции (лайки), кастомные эмодзи; скачивание медиа-файлов (фото, .ogg)
- [x] **AI Pipeline — Stage 1: Gemini Recognition** — HTTP-клиент Google Gemini (через OpenRouter или напрямую); транскрипция голосовых сообщений (.ogg → текст); распознавание изображений (OCR скриншотов оплат, описание фото товаров)
- [x] **AI Pipeline — Stage 2: MiniMax Dialog & Tool Calling** — HTTP-клиент MiniMax API; формирование контекста диалога; обработка `tool_call` ответов; маршрутизация tool_call → доменные модули через `ai::tools::execute_tool()`; правило «2 цифр» (валидация числовых кодов); управление реакциями на сообщения
- [x] **End-to-End Message Flow** — связка Telegram → AI Pipeline → Domain Modules → Telegram; полный цикл: получение сообщения → распознавание (если нужно) → диалог с MiniMax → вызов инструментов → формирование и отправка ответа; обработка ошибок на каждом этапе
- [x] **Alerting & Error Handling** — `shared::alerting`: panic hook с отправкой сообщения администратору; алёрты при ошибках API (MiniMax, Gemini, Telegram); retry-логика для HTTP-запросов; graceful degradation (если ИИ недоступен — уведомление владельцу)
- [x] **Admin Web Panel — Core** — Axum HTTP-сервер на `localhost`; Askama-шаблоны (`base.html`, `dashboard.html`, `config.html`); Dashboard: статус бота, счётчики заказов/выручки; Конфигурация: формы для API-ключей, Chat-ID; базовая аутентификация
- [x] **Admin Web Panel — Logs & Media Manager** — WebSocket-консоль для логов в реальном времени (`admin::ws`); Медиа-менеджер: загрузка изображений, управление тегами и доступом `is_allowed_for_ai`; интеграция с `media_manager` модулем
- [x] **ARM64 Cross-Compilation & Deployment** — настройка cross-compilation для `aarch64-unknown-linux-gnu`; оптимизация бинарника (LTO, strip, `opt-level = "z"`); скрипт деплоя на Samsung Galaxy Flip 3 (Chroot Debian); systemd unit / запуск в фоне; проверка потребления RAM < 100 МБ
- [x] **Production Hardening** — rate limiting для Telegram-обработки; таймауты на все HTTP-вызовы; ограничение размера скачиваемых файлов; логирование (`tracing` crate); мониторинг здоровья (health-check эндпоинт); бэкап SQLite; тесты (unit для доменных модулей, integration для AI pipeline)

## Completed

| Milestone | Date |
|-----------|------|
| Project Scaffold & Shared Core | 2026-05-26 |
| SQLite Database & Migrations | 2026-05-26 |
| Domain Modules | 2026-05-26 |
| Telegram Bot Module | 2026-05-26 |
| AI Pipeline — Stage 1: Gemini Recognition | 2026-05-26 |
| AI Pipeline — Stage 2: MiniMax Dialog & Tool Calling | 2026-05-26 |
| End-to-End Message Flow | 2026-05-26 |
| Alerting & Error Handling | 2026-05-26 |
| Admin Web Panel — Core | 2026-05-26 |
| Admin Web Panel — Logs & Media Manager | 2026-05-26 |
| ARM64 Cross-Compilation & Deployment | 2026-05-26 |
| Production Hardening | 2026-05-26 |
