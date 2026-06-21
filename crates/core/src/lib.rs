//! Клиентское ядро «Форточки» — общая логика для Android и iOS.
//!
//! Ядро сознательно синхронное (blocking reqwest): на мобильных платформах
//! его вызывают из фонового потока (WorkManager / GCD), и тащить туда
//! async-рантайм незачем. Асинхронность живёт только на сервере.

pub mod error;
pub mod retry;

pub use error::CoreError;

/// Версия ядра — её дергаем из Kotlin/Swift для проверки FFI-моста.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
