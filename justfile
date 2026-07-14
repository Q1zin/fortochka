# Команды разработки Форточки. Установка just: brew install just

# NDK: берём последнюю установленную версию из SDK
ndk_home := `ls -d "$HOME/Library/Android/sdk/ndk"/* 2>/dev/null | sort -V | tail -1`
# Gradle 8.x требует JVM ≤ 23 — на маке закрепляемся на 21
java_home := `/usr/libexec/java_home -v 21 2>/dev/null || echo "$JAVA_HOME"`

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

# Сквозной smoke-тест: register → upload → pair → wallpaper (по умолчанию прод)
smoke url="https://fortochka.fun":
    bash scripts/smoke.sh {{url}}

# Разовая настройка Android-тулчейна (rustup-таргеты + cargo-ndk)
setup-android:
    rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
    cargo install cargo-ndk --locked

# Kotlin-биндинги из Rust-ядра (uniffi)
bindings:
    bash scripts/gen-bindings.sh

# Сборка Rust-ядра под Android + биндинги
android-libs: bindings
    ANDROID_NDK_HOME="{{ndk_home}}" cargo ndk -t arm64-v8a -t armeabi-v7a \
        -o android/app/src/main/jniLibs build -p fortochka-mobile --release

# Собрать APK и поставить на подключённый телефон
android: android-libs
    cd android && JAVA_HOME="{{java_home}}" ./gradlew installDebug

# Только собрать APK (без установки)
apk: android-libs
    cd android && JAVA_HOME="{{java_home}}" ./gradlew assembleDebug
