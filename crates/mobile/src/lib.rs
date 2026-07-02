//! FFI-граница для мобильных платформ (`UniFFI`).
//!
//! Функции нарочно свободные и без состояния: Kotlin/Swift вызывают их
//! из фоновых потоков, ядро само создаёт клиент на каждый вызов.
//! Когда появится долгоживущее состояние — переедем на `uniffi::Object`.

// FFI-граница обязана принимать owned-типы — этого требует uniffi
#![allow(clippy::needless_pass_by_value)]

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
