-- Исходная схема. IF NOT EXISTS — потому что базы, созданные до перехода
-- на миграции, уже содержат эти таблицы: миграция для них ничего не меняет.

CREATE TABLE IF NOT EXISTS cameras (
    id           TEXT PRIMARY KEY,
    name         TEXT NOT NULL,
    upload_token TEXT NOT NULL UNIQUE,
    pairing_code TEXT NOT NULL UNIQUE,
    created_at   INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS view_tokens (
    token      TEXT PRIMARY KEY,
    camera_id  TEXT NOT NULL REFERENCES cameras(id),
    created_at INTEGER NOT NULL
);
