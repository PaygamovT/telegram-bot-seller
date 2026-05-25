-- PRAGMA setup for database file
PRAGMA journal_mode=WAL;
PRAGMA foreign_keys=ON;

-- Таблица контактов/клиентов
CREATE TABLE IF NOT EXISTS contacts (
    chat_id INTEGER PRIMARY KEY,
    first_name TEXT,
    address TEXT,
    phone_number TEXT,
    username TEXT,
    nickname TEXT
);

-- Таблица товаров и парфюмерии
CREATE TABLE IF NOT EXISTS catalog (
    product_id INTEGER PRIMARY KEY,
    product_name TEXT NOT NULL,
    standard_price INTEGER NOT NULL,
    stock_quantity INTEGER NOT NULL,
    tags TEXT,
    notes TEXT,
    suitable_season TEXT,
    suitable_situation TEXT,
    duration TEXT,
    sillage TEXT
);

-- Таблица заказов
CREATE TABLE IF NOT EXISTS orders (
    order_id TEXT PRIMARY KEY,
    chat_id INTEGER NOT NULL REFERENCES contacts(chat_id),
    status TEXT NOT NULL,
    delivery_address TEXT,
    total_amount INTEGER NOT NULL
);

-- Позиции в заказе
CREATE TABLE IF NOT EXISTS order_items (
    item_id TEXT PRIMARY KEY,
    order_id TEXT NOT NULL REFERENCES orders(order_id) ON DELETE CASCADE,
    product_id INTEGER NOT NULL REFERENCES catalog(product_id),
    quantity INTEGER NOT NULL CHECK(quantity > 0),
    price_at_sale INTEGER NOT NULL
);

-- Системные настройки и токены
CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Медиа-файлы агента (имиджи, доступные ИИ)
CREATE TABLE IF NOT EXISTS agent_media (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_path TEXT NOT NULL,
    telegram_file_id TEXT,
    title TEXT NOT NULL,
    purpose TEXT NOT NULL,
    is_allowed_for_ai BOOLEAN DEFAULT 1
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_orders_chat_id ON orders(chat_id);
CREATE INDEX IF NOT EXISTS idx_order_items_order_id ON order_items(order_id);
