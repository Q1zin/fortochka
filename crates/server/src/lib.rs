//! Сервер «Форточки»: приём кадров с камер, пейринг и выдача обоев.
//!
//! Библиотечный крейт + тонкий `main.rs`, чтобы интеграционные тесты
//! могли поднять точно такое же приложение на случайном порту.

pub mod error;
pub mod handlers;
pub mod images;
pub mod limits;
pub mod state;
pub mod storage;

use std::time::Duration;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::{delete, get, post};
use fortochka_proto::routes;
use tower_http::trace::TraceLayer;

pub use state::{AppState, ServerConfig};

/// Максимальный размер кадра: 12-Мп JPEG обычно 3–6 МБ, берём с запасом.
const MAX_FRAME_BYTES: usize = 20 * 1024 * 1024;

/// Роутер приложения. Вызывать внутри tokio-рантайма (см. [`limits::per_ip`])
/// и обслуживать с `into_make_service_with_connect_info::<SocketAddr>()`.
pub fn app(state: AppState) -> Router {
    Router::new()
        .route(routes::HEALTHZ, get(handlers::healthz))
        .route(
            routes::REGISTER_CAMERA,
            post(handlers::register_camera).layer(limits::per_ip(5, Duration::from_mins(1))),
        )
        .route(
            "/api/v1/cameras/{camera_id}/frame",
            post(handlers::upload_frame).layer(DefaultBodyLimit::max(MAX_FRAME_BYTES)),
        )
        .route(
            "/api/v1/cameras/{camera_id}",
            delete(handlers::delete_camera),
        )
        .route(
            routes::PAIR,
            post(handlers::pair).layer(limits::per_ip(10, Duration::from_secs(6))),
        )
        .route("/cam/{view_token}/wallpaper.jpg", get(handlers::wallpaper))
        .route("/api/v1/view/{view_token}", get(handlers::view_status))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
