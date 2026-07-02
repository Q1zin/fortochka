#!/usr/bin/env bash
# Генерация Kotlin-биндингов из fortochka-mobile.
# Library-режим uniffi читает метаданные из собранной хостовой библиотеки.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build -p fortochka-mobile

case "$(uname)" in
    Darwin) LIB=target/debug/libfortochka_mobile.dylib ;;
    *)      LIB=target/debug/libfortochka_mobile.so ;;
esac

cargo run -p uniffi-bindgen -- generate \
    --library "$LIB" \
    --language kotlin \
    --out-dir android/app/src/main/kotlin

echo "Kotlin-биндинги обновлены: android/app/src/main/kotlin/uniffi/"
