use std::time::{SystemTime, UNIX_EPOCH};

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, header};
use axum::response::{IntoResponse, Response};
use fortochka_proto::{
    FrameUploadResponse, PairRequest, PairResponse, RegisterCameraRequest, RegisterCameraResponse,
};
use rand::Rng;
use serde::Deserialize;
use sqlx::Row;
use uuid::Uuid;

use crate::error::ApiError;
use crate::state::AppState;
use crate::{images, storage};

/// Пейринг-код без похожих символов (0/O, 1/I) — его вводят руками.
const CODE_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
const CODE_LEN: usize = 6;
const TOKEN_LEN: usize = 40;
/// Кроп больше 4К бессмысленен и открывает дорогу к отказу в обслуживании
/// через огромные аллокации.
const MAX_DIMENSION: u32 = 4096;

pub async fn healthz() -> &'static str {
    "ok"
}

pub async fn register_camera(
    State(state): State<AppState>,
    Json(req): Json<RegisterCameraRequest>,
) -> Result<Json<RegisterCameraResponse>, ApiError> {
    let name = req.name.trim();
    if name.is_empty() || name.len() > 100 {
        return Err(ApiError::bad_request(
            "invalid_name",
            "имя камеры должно быть от 1 до 100 символов",
        ));
    }

    // Пейринг-код короткий, коллизии реальны — пробуем несколько раз,
    // уникальность гарантирует UNIQUE-констрейнт в БД.
    for _ in 0..5 {
        let camera_id = Uuid::new_v4().to_string();
        let upload_token = random_token(TOKEN_LEN);
        let pairing_code = random_pairing_code();

        let inserted = sqlx::query(
            "INSERT INTO cameras (id, name, upload_token, pairing_code, created_at) \
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&camera_id)
        .bind(name)
        .bind(&upload_token)
        .bind(&pairing_code)
        .bind(unix_now())
        .execute(&state.db)
        .await;

        match inserted {
            Ok(_) => {
                tracing::info!(camera_id, "зарегистрирована камера");
                return Ok(Json(RegisterCameraResponse {
                    camera_id,
                    upload_token,
                    pairing_code,
                }));
            }
            Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {}
            Err(e) => return Err(e.into()),
        }
    }
    Err(ApiError::Internal(anyhow::anyhow!(
        "не удалось подобрать уникальный пейринг-код за 5 попыток"
    )))
}

pub async fn upload_frame(
    State(state): State<AppState>,
    Path(camera_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<FrameUploadResponse>, ApiError> {
    let token = bearer_token(&headers)?;

    let row = sqlx::query("SELECT upload_token FROM cameras WHERE id = ?")
        .bind(&camera_id)
        .fetch_optional(&state.db)
        .await?;
    // Несуществующая камера и неверный токен неразличимы для клиента —
    // не подсказываем перебором, какие id существуют.
    let valid = row.is_some_and(|r| r.get::<String, _>("upload_token") == token);
    if !valid {
        return Err(ApiError::unauthorized(
            "invalid_upload_token",
            "неизвестная камера или неверный токен",
        ));
    }

    // Полный декод на каждый upload — лишний CPU; магических байт JPEG
    // достаточно, чтобы отсечь мусор. Декод случится при выдаче обоев.
    if !body.starts_with(&[0xFF, 0xD8]) {
        return Err(ApiError::bad_request(
            "not_a_jpeg",
            "тело запроса не похоже на JPEG",
        ));
    }

    storage::store_frame(&state.data_dir, &camera_id, &body).await?;
    tracing::debug!(camera_id, bytes = body.len(), "принят кадр");
    Ok(Json(FrameUploadResponse {
        received_at: unix_now(),
    }))
}

pub async fn pair(
    State(state): State<AppState>,
    Json(req): Json<PairRequest>,
) -> Result<Json<PairResponse>, ApiError> {
    let code = req.pairing_code.trim().to_uppercase();

    let row = sqlx::query("SELECT id, name FROM cameras WHERE pairing_code = ?")
        .bind(&code)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| {
            ApiError::not_found("invalid_pairing_code", "камера с таким кодом не найдена")
        })?;

    let camera_id: String = row.get("id");
    let camera_name: String = row.get("name");
    let view_token = random_token(TOKEN_LEN);

    sqlx::query("INSERT INTO view_tokens (token, camera_id, created_at) VALUES (?, ?, ?)")
        .bind(&view_token)
        .bind(&camera_id)
        .bind(unix_now())
        .execute(&state.db)
        .await?;

    tracing::info!(camera_id, "подключён зритель");
    Ok(Json(PairResponse {
        view_token,
        camera_name,
    }))
}

#[derive(Debug, Deserialize)]
pub struct WallpaperParams {
    w: Option<u32>,
    h: Option<u32>,
}

pub async fn wallpaper(
    State(state): State<AppState>,
    Path(view_token): Path<String>,
    Query(params): Query<WallpaperParams>,
) -> Result<Response, ApiError> {
    let row = sqlx::query("SELECT camera_id FROM view_tokens WHERE token = ?")
        .bind(&view_token)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| ApiError::not_found("invalid_view_token", "токен не найден"))?;
    let camera_id: String = row.get("camera_id");

    let jpeg = storage::load_frame(&state.data_dir, &camera_id)
        .await?
        .ok_or_else(|| {
            ApiError::not_found("no_frame_yet", "камера ещё не прислала ни одного кадра")
        })?;

    let jpeg = match (params.w, params.h) {
        (Some(w), Some(h)) => {
            if !(1..=MAX_DIMENSION).contains(&w) || !(1..=MAX_DIMENSION).contains(&h) {
                return Err(ApiError::bad_request(
                    "invalid_dimensions",
                    format!("w и h должны быть в диапазоне 1..={MAX_DIMENSION}"),
                ));
            }
            // Декод и ресайз — тяжёлая CPU-работа, уводим её с async-потоков.
            tokio::task::spawn_blocking(move || images::crop_to_fit(&jpeg, w, h))
                .await?
                .map_err(|e| match e {
                    images::CropError::Image(_) => {
                        ApiError::Internal(anyhow::anyhow!("битый кадр на диске: {e}"))
                    }
                    images::CropError::ZeroDimension => {
                        ApiError::bad_request("invalid_dimensions", e.to_string())
                    }
                })?
        }
        // Без параметров отдаём оригинал как есть.
        (None, None) => jpeg,
        _ => {
            return Err(ApiError::bad_request(
                "invalid_dimensions",
                "w и h задаются только вместе",
            ));
        }
    };

    Ok((
        [
            (header::CONTENT_TYPE, "image/jpeg"),
            // Каждый запрос — свежий кадр; кэширование сломало бы смысл обоев.
            (header::CACHE_CONTROL, "no-store"),
        ],
        jpeg,
    )
        .into_response())
}

fn bearer_token(headers: &HeaderMap) -> Result<&str, ApiError> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| {
            ApiError::unauthorized("missing_token", "нужен заголовок Authorization: Bearer …")
        })
}

fn random_token(len: usize) -> String {
    rand::rng()
        .sample_iter(rand::distr::Alphanumeric)
        .take(len)
        .map(char::from)
        .collect()
}

fn random_pairing_code() -> String {
    let mut rng = rand::rng();
    (0..CODE_LEN)
        .map(|_| CODE_ALPHABET[rng.random_range(0..CODE_ALPHABET.len())] as char)
        .collect()
}

#[allow(clippy::cast_possible_wrap)] // unix-время переполнит i64 через ~300 млрд лет
fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("время до 1970 — сломанные часы")
        .as_secs() as i64
}
