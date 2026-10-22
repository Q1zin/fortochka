//! FFI-граница для мобильных платформ (`UniFFI`).
//!
//! Функции нарочно свободные и без состояния: Kotlin/Swift вызывают их
//! из фоновых потоков, ядро само создаёт клиент на каждый вызов.
//! Когда появится долгоживущее состояние — переедем на `uniffi::Object`.

// FFI-граница обязана принимать owned-типы — этого требует uniffi
#![allow(clippy::needless_pass_by_value)]

use std::path::Path;

use fortochka_core::{ApiClient, CoreError};

uniffi::setup_scaffolding!();

/// Копия `CoreError` в терминах, которые `UniFFI` умеет отдавать наружу:
/// Kotlin увидит `MobileException.*` с сообщением из `Display`.
#[derive(Debug, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum MobileError {
    #[error("сеть: {message}")]
    Network { message: String },
    #[error("сервер ({status}): {message}")]
    Api { status: u16, message: String },
    #[error("{message}")]
    Other { message: String },
}

impl From<CoreError> for MobileError {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::Api { status, message } => Self::Api { status, message },
            // Сам reqwest пишет общую фразу и URL (а в URL — токен зрителя);
            // причину — DNS, отказ соединения, TLS — несут вложенные source().
            CoreError::Network(err) => Self::Network {
                message: error_chain(&err.without_url()),
            },
            other => Self::Other {
                message: other.to_string(),
            },
        }
    }
}

/// «ошибка: её причина: причина причины» — вся цепочка `source()` в одну строку.
fn error_chain(err: &dyn std::error::Error) -> String {
    std::iter::successors(Some(err), |e| e.source())
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(": ")
}

#[derive(uniffi::Record)]
pub struct CameraRegistration {
    pub camera_id: String,
    pub upload_token: String,
    pub pairing_code: String,
}

#[derive(uniffi::Record)]
pub struct PairedCamera {
    pub view_token: String,
    pub camera_name: String,
}

#[derive(uniffi::Record)]
pub struct ViewStatus {
    pub camera_name: String,
    /// Unix-время последнего кадра; `None` — кадров ещё не было.
    pub last_frame_at: Option<i64>,
    /// Unix-время последнего видеоклипа; `None` — камера видео не снимает.
    pub last_clip_at: Option<i64>,
}

#[derive(uniffi::Enum)]
pub enum DeviceRole {
    Camera,
    Viewer,
}

/// Зеркало `fortochka_core::DeviceConfig` для FFI: uniffi-Record
/// не может быть тем же типом, что и serde-структура ядра.
#[derive(uniffi::Record)]
pub struct DeviceConfig {
    pub server_url: Option<String>,
    pub role: Option<DeviceRole>,
    pub camera_id: Option<String>,
    pub upload_token: Option<String>,
    pub pairing_code: Option<String>,
    pub capture_interval_secs: Option<u32>,
    pub view_token: Option<String>,
    pub camera_name: Option<String>,
}

impl From<fortochka_core::DeviceConfig> for DeviceConfig {
    fn from(c: fortochka_core::DeviceConfig) -> Self {
        Self {
            server_url: c.server_url,
            role: c.role.map(|r| match r {
                fortochka_core::DeviceRole::Camera => DeviceRole::Camera,
                fortochka_core::DeviceRole::Viewer => DeviceRole::Viewer,
            }),
            camera_id: c.camera_id,
            upload_token: c.upload_token,
            pairing_code: c.pairing_code,
            capture_interval_secs: c.capture_interval_secs,
            view_token: c.view_token,
            camera_name: c.camera_name,
        }
    }
}

impl From<DeviceConfig> for fortochka_core::DeviceConfig {
    fn from(c: DeviceConfig) -> Self {
        Self {
            server_url: c.server_url,
            role: c.role.map(|r| match r {
                DeviceRole::Camera => fortochka_core::DeviceRole::Camera,
                DeviceRole::Viewer => fortochka_core::DeviceRole::Viewer,
            }),
            camera_id: c.camera_id,
            upload_token: c.upload_token,
            pairing_code: c.pairing_code,
            capture_interval_secs: c.capture_interval_secs,
            view_token: c.view_token,
            camera_name: c.camera_name,
        }
    }
}

/// Читает конфиг из `dir` (платформа передаёт свой files-каталог).
/// Отсутствующий файл — не ошибка, вернётся пустой конфиг.
#[uniffi::export]
pub fn load_config(dir: String) -> Result<DeviceConfig, MobileError> {
    Ok(fortochka_core::DeviceConfig::load(Path::new(&dir))?.into())
}

/// Атомарно сохраняет конфиг в `dir`.
#[uniffi::export]
pub fn save_config(dir: String, config: DeviceConfig) -> Result<(), MobileError> {
    fortochka_core::DeviceConfig::from(config).save(Path::new(&dir))?;
    Ok(())
}

/// Версия ядра — первый вызов из Kotlin для проверки моста Kotlin → JNI → Rust.
#[uniffi::export]
pub fn core_version() -> String {
    fortochka_core::VERSION.to_owned()
}

/// `GET /healthz` — проверка, что сервер жив и URL правильный.
#[uniffi::export]
pub fn check_server(base_url: String) -> Result<(), MobileError> {
    Ok(ApiClient::new(&base_url)?.health()?)
}

#[uniffi::export]
pub fn register_camera(base_url: String, name: String) -> Result<CameraRegistration, MobileError> {
    let resp = ApiClient::new(&base_url)?.register_camera(&name)?;
    Ok(CameraRegistration {
        camera_id: resp.camera_id,
        upload_token: resp.upload_token,
        pairing_code: resp.pairing_code,
    })
}

#[uniffi::export]
pub fn pair_with_code(base_url: String, pairing_code: String) -> Result<PairedCamera, MobileError> {
    let resp = ApiClient::new(&base_url)?.pair(&pairing_code)?;
    Ok(PairedCamera {
        view_token: resp.view_token,
        camera_name: resp.camera_name,
    })
}

#[uniffi::export]
pub fn upload_frame(
    base_url: String,
    camera_id: String,
    upload_token: String,
    jpeg: Vec<u8>,
) -> Result<(), MobileError> {
    ApiClient::new(&base_url)?.upload_frame(&camera_id, &upload_token, &jpeg)?;
    Ok(())
}

/// Сброс телефона-камеры: камера, её кадр и зрители удаляются с сервера.
#[uniffi::export]
pub fn delete_camera(
    base_url: String,
    camera_id: String,
    upload_token: String,
) -> Result<(), MobileError> {
    Ok(ApiClient::new(&base_url)?.delete_camera(&camera_id, &upload_token)?)
}

/// Жива ли камера и есть ли новый кадр — без скачивания самого кадра.
#[uniffi::export]
pub fn view_status(base_url: String, view_token: String) -> Result<ViewStatus, MobileError> {
    let status = ApiClient::new(&base_url)?.view_status(&view_token)?;
    Ok(ViewStatus {
        camera_name: status.camera_name,
        last_frame_at: status.last_frame_at,
        last_clip_at: status.last_clip_at,
    })
}

/// Видеоклип камеры из файла `path` — Kotlin отдаёт путь, а не мегабайты байтов.
#[uniffi::export]
pub fn upload_clip(
    base_url: String,
    camera_id: String,
    upload_token: String,
    path: String,
) -> Result<(), MobileError> {
    ApiClient::new(&base_url)?.upload_clip(&camera_id, &upload_token, Path::new(&path))?;
    Ok(())
}

/// Последний клип камеры — в файл `dest` (атомарно, через временный файл).
#[uniffi::export]
pub fn fetch_clip(base_url: String, view_token: String, dest: String) -> Result<(), MobileError> {
    Ok(ApiClient::new(&base_url)?.fetch_clip(&view_token, Path::new(&dest))?)
}

/// Свежий кадр, кропнутый сервером под экран зрителя.
#[uniffi::export]
pub fn fetch_wallpaper(
    base_url: String,
    view_token: String,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, MobileError> {
    Ok(ApiClient::new(&base_url)?.fetch_wallpaper(&view_token, width, height)?)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use fortochka_core::retry::Backoff;

    use super::*;

    #[test]
    fn network_error_shows_cause_without_url() {
        // Порт освобождается сразу после bind — соединение получит отказ.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let err = ApiClient::new(&format!("http://127.0.0.1:{port}"))
            .unwrap()
            .with_backoff(Backoff {
                attempts: 1,
                base_delay: Duration::ZERO,
                max_delay: Duration::ZERO,
            })
            .fetch_wallpaper("secret-view-token", 1, 1)
            .unwrap_err();

        let message = MobileError::from(err).to_string();
        assert!(message.contains("tcp connect error"), "{message}");
        assert!(!message.contains("secret-view-token"), "{message}");
    }
}
