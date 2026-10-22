//! Лимит запросов по IP для открытых ручек. Пейринг-код короткий
//! (32⁶ ≈ 10⁹ вариантов) — без лимита чужую камеру можно подобрать перебором.
//!
//! За Caddy адрес клиента берём из `X-Forwarded-For`: Caddy не доверяет
//! пришедшему заголовку и подставляет настоящий IP. Без прокси (локально,
//! в тестах) — адрес соединения, поэтому сервер поднимают с `ConnectInfo`.

use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::body::Body;
use axum::http::{Response, StatusCode};
use axum::response::IntoResponse;
use fortochka_proto::ApiErrorBody;
use governor::clock::QuantaInstant;
use governor::middleware::NoOpMiddleware;
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::key_extractor::SmartIpKeyExtractor;
use tower_governor::{GovernorError, GovernorLayer};

use crate::error::ApiError;

pub type IpLimit = GovernorLayer<SmartIpKeyExtractor, NoOpMiddleware<QuantaInstant>, Body>;

/// `burst` запросов сразу, дальше по одному раз в `every`.
/// Вызывать внутри tokio-рантайма: память лимитера чистит фоновая задача.
pub fn per_ip(burst: u32, every: Duration) -> IpLimit {
    let config = Arc::new(
        GovernorConfigBuilder::default()
            .key_extractor(SmartIpKeyExtractor)
            .period(every)
            .burst_size(burst)
            .finish()
            .expect("лимит с ненулевыми burst и периодом"),
    );
    // Лимитер помнит каждый IP — раз в минуту забываем тех, кто давно не заходил.
    let limiter = config.limiter().clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_mins(1));
        loop {
            tick.tick().await;
            limiter.retain_recent();
        }
    });
    GovernorLayer::new(config).error_handler(too_many_requests)
}

/// 429 в том же JSON-формате, что и остальные ошибки API, плюс `Retry-After`.
fn too_many_requests(err: GovernorError) -> Response<Body> {
    match err {
        GovernorError::TooManyRequests { wait_time, headers } => {
            let mut resp = (
                StatusCode::TOO_MANY_REQUESTS,
                Json(ApiErrorBody {
                    code: "rate_limited".to_owned(),
                    message: format!("слишком много запросов, повторите через {wait_time} с"),
                }),
            )
                .into_response();
            if let Some(headers) = headers {
                resp.headers_mut().extend(headers);
            }
            resp
        }
        // не нашли ни X-Forwarded-For, ни адреса соединения — ошибка настройки сервера
        other => ApiError::Internal(anyhow::anyhow!("лимитер: {other}")).into_response(),
    }
}
