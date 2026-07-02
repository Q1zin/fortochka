//! CLI-обёртка над uniffi-bindgen: генерирует Kotlin/Swift-биндинги
//! из собранной библиотеки. Запуск — через `scripts/gen-bindings.sh`.

fn main() {
    uniffi::uniffi_bindgen_main();
}
