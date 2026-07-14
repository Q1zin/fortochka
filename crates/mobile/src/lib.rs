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
            CoreError::Network(err) => Self::Network {
                message: err.to_string(),
            },
            other => Self::Other {
                message: other.to_string(),
            },
        }
    }
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
