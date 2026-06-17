# Команды разработки Форточки. Установка just: brew install just

default:
    @just --list

# Все проверки как в CI
check:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace

fmt:
    cargo fmt --all

test:
    cargo test --workspace

# Локальный запуск сервера (данные в ./data)
server:
    cargo run -p fortochka-server
