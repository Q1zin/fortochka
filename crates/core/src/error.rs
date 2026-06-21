use thiserror::Error;

/// Ошибки ядра. Один enum на весь крейт: FFI-граница потом
/// отобразит его в исключения Kotlin/Swift как есть.
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("некорректный URL сервера: {0}")]
    InvalidBaseUrl(String),

    #[error("сеть: {0}")]
    Network(#[from] reqwest::Error),

    /// Сервер ответил, но не 2xx.
    #[error("сервер вернул {status}: {message}")]
    Api { status: u16, message: String },

    #[error("ввод-вывод: {0}")]
    Io(#[from] std::io::Error),

    #[error("сериализация: {0}")]
    Serde(#[from] serde_json::Error),
}

impl CoreError {
    /// Стоит ли повторять запрос: сетевые сбои и 5xx — временные,
    /// 4xx — ошибка клиента, повторять бессмысленно.
    #[must_use]
    pub fn is_transient(&self) -> bool {
        match self {
            Self::Network(_) => true,
            Self::Api { status, .. } => *status >= 500,
            _ => false,
        }
    }
}
