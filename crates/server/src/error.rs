use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use fortochka_proto::ApiErrorBody;

/// Ошибка уровня API: знает свой HTTP-статус и машиночитаемый код.
/// Всё неожиданное сворачивается в `Internal` — клиент видит 500
/// без деталей, детали остаются в логах.
#[derive(Debug)]
pub enum ApiError {
    BadRequest { code: &'static str, message: String },
    Unauthorized { code: &'static str, message: String },
    NotFound { code: &'static str, message: String },
    Internal(anyhow::Error),
}

impl ApiError {
    pub fn bad_request(code: &'static str, message: impl Into<String>) -> Self {
        Self::BadRequest {
            code,
            message: message.into(),
        }
    }

    pub fn unauthorized(code: &'static str, message: impl Into<String>) -> Self {
        Self::Unauthorized {
            code,
            message: message.into(),
        }
    }

    pub fn not_found(code: &'static str, message: impl Into<String>) -> Self {
        Self::NotFound {
            code,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::BadRequest { code, message } => (StatusCode::BAD_REQUEST, code, message),
            Self::Unauthorized { code, message } => (StatusCode::UNAUTHORIZED, code, message),
            Self::NotFound { code, message } => (StatusCode::NOT_FOUND, code, message),
            Self::Internal(err) => {
                tracing::error!("внутренняя ошибка: {err:#}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal",
                    "внутренняя ошибка сервера".to_owned(),
                )
            }
        };
        (
            status,
            Json(ApiErrorBody {
                code: code.to_owned(),
                message,
            }),
        )
            .into_response()
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        Self::Internal(err.into())
    }
}

impl From<std::io::Error> for ApiError {
    fn from(err: std::io::Error) -> Self {
        Self::Internal(err.into())
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err)
    }
}

impl From<tokio::task::JoinError> for ApiError {
    fn from(err: tokio::task::JoinError) -> Self {
        Self::Internal(err.into())
    }
}
