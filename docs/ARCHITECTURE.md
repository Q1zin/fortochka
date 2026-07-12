# Архитектура

Продуктовая концепция — в [README](../README.md). Здесь — как устроен код.

## Слои вместо ОС

Общая логика делится **по слоям**, а не по операционным системам. Всё, что можно
написать один раз — протокол, сетевой клиент, пейринг, конфиг — живёт в Rust.
Платформенные оболочки (Kotlin сейчас, Swift в будущем) содержат только то,
к чему нет Rust-API: камеру, установку обоев, фоновый планировщик.

```
crates/proto ──── DTO API (serde), общие для всех. Ноль рантайма.
      │
      ├── crates/server ── axum + tokio + SQLite; кадры на диске; кроп (image)
      │
      └── crates/core ─── клиент API (blocking reqwest), ретраи, DeviceConfig
              │
              └── crates/mobile ── UniFFI-граница → libfortochka.so
                      │
                      ├── android/ ── Kotlin: CameraX, WallpaperManager, WorkManager
                      └── ios/ (будущее) ── Swift: то же ядро, другая оболочка
```

Ядро сознательно **синхронное**: на мобильных платформах его вызывают из
фонового потока (WorkManager / GCD), async-рантайм там не нужен.
Асинхронность живёт только на сервере.

## Поток данных

```
[камера] ──POST /api/v1/cameras/{id}/frame (Bearer upload_token, JPEG)──▶ [сервер]
                                                       │
                                     data/frames/{camera_id}.jpg (атомарный rename)
                                     SQLite: cameras, view_tokens
                                                       │
[зритель] ◀──GET /cam/{view_token}/wallpaper.jpg?w=1080&h=2400── (центр-кроп + ресайз)
```

- **Пейринг**: камера при регистрации получает 6-символьный код (без 0/O/1/I);
  зритель вводит код → `POST /api/v1/pair` → персональный `view_token`.
- **Токен в пути URL** (не в заголовке) — iOS Shortcuts умеет только простой GET.
- `Cache-Control: no-store` — каждый запрос обоев должен приносить свежий кадр.
- Хранится только **последний** кадр камеры; замена атомарна (tmp + rename),
  поэтому читатель никогда не получит полфайла.

## Сервер

- `state.rs` — конфиг из env (`BIND_ADDR`, `DATA_DIR`), инициализация SQLite (WAL).
- `handlers.rs` — регистрация, приём кадра, пейринг, выдача обоев.
- `images.rs` — центр-кроп до соотношения экрана + resize, JPEG q85.
  Тяжёлая CPU-работа уводится в `spawn_blocking`.
- `error.rs` — `ApiError` → HTTP-статус + JSON-тело `ApiErrorBody`;
  всё неожиданное → 500 без деталей (детали в логах).
- Схема БД — `schema.sql`, применяется при старте (`CREATE IF NOT EXISTS`);
  на sqlx-миграции переедем, когда схема начнёт меняться.

Интеграционные тесты (`crates/server/tests/api.rs`) поднимают настоящий сервер
на случайном порту и гоняют его **настоящим клиентом из `fortochka-core`** —
проверяется совместимость клиента и сервера на уровне провода.

## CI/CD (GitHub Actions, `.github/workflows/ci.yml`)

1. **rust** — `fmt --check`, `clippy -D warnings` (с pedantic), `cargo test`.
2. **android** — cargo-ndk собирает `libfortochka.so`, Gradle — debug-APK (артефакт).
3. **deploy** (только push в master) — Docker-образ → GHCR →
   scp compose-файлов → `docker compose pull && up -d` по SSH.

Подробности деплоя и разовая настройка сервера — в [DEPLOY.md](DEPLOY.md).

## Команды разработки

`justfile` в корне: `just check` (как CI), `just server`, `just android`
(собрать ядро + APK и поставить на телефон по USB).

## Журнал решений

| Дата | Решение | Почему |
|------|---------|--------|
| 2026-07-12 | Kotlin-оболочка + Rust-ядро (UniFFI), а не полный Rust-UI | к камере/обоям/WorkManager всё равно нет Rust-API; ядро переиспользуется на iOS |
| 2026-07-12 | Одно Android-приложение с двумя ролями | проще сборка и пейринг; разделение возможно позже |
| 2026-07-12 | Деплой: GHCR + docker compose + Caddy | воспроизводимость, откат на предыдущий образ, авто-HTTPS |
| 2026-07-12 | Ядро синхронное (blocking reqwest) | вызывается из фоновых потоков платформы; async не нужен |
| 2026-07-13 | SQLite + кадры на диске | один узел, нулевое обслуживание; последний кадр — просто файл |
