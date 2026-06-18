//! Типы API-контракта «Форточки», общие для сервера и всех клиентов.
//!
//! Крейт намеренно минимальный: только DTO (serde) и пути маршрутов.
//! Никакого рантайма — его тянут и сервер (axum), и мобильное ядро (reqwest).

use serde::{Deserialize, Serialize};

/// Пути API. Единственный источник правды и для роутера сервера,
/// и для клиента — чтобы они физически не могли разъехаться.
pub mod routes {
    pub const HEALTHZ: &str = "/healthz";
    pub const REGISTER_CAMERA: &str = "/api/v1/cameras/register";
    pub const PAIR: &str = "/api/v1/pair";

    /// Приём кадра от камеры.
    #[must_use]
    pub fn camera_frame(camera_id: &str) -> String {
        format!("/api/v1/cameras/{camera_id}/frame")
    }

    /// Выдача обоев зрителю. Токен в пути, а не в заголовке,
    /// потому что iOS Shortcuts умеет только простой GET по URL.
    #[must_use]
    pub fn wallpaper(view_token: &str) -> String {
        format!("/cam/{view_token}/wallpaper.jpg")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterCameraRequest {
    /// Человекочитаемое имя камеры («Дача», «Аквариум»).
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterCameraResponse {
    pub camera_id: String,
    /// Секрет камеры для загрузки кадров (Bearer).
    pub upload_token: String,
    /// Короткий код, который вводит зритель для подключения.
    pub pairing_code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairRequest {
    pub pairing_code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairResponse {
    /// Токен зрителя — часть персонального URL обоев.
    pub view_token: String,
    pub camera_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameUploadResponse {
    /// Unix-время (секунды), когда сервер принял кадр.
    pub received_at: i64,
}

/// Тело ошибки, которое сервер отдаёт вместе с не-2xx статусом.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorBody {
    /// Машиночитаемый код («invalid_pairing_code», «camera_not_found»).
    pub code: String,
    /// Человекочитаемое описание для логов и UI.
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dto_round_trip() {
        let resp = RegisterCameraResponse {
            camera_id: "cam-1".into(),
            upload_token: "secret".into(),
            pairing_code: "ABC123".into(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let back: RegisterCameraResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back.camera_id, resp.camera_id);
        assert_eq!(back.pairing_code, resp.pairing_code);
    }

    #[test]
    fn route_builders() {
        assert_eq!(routes::camera_frame("id-1"), "/api/v1/cameras/id-1/frame");
        assert_eq!(routes::wallpaper("tok"), "/cam/tok/wallpaper.jpg");
    }
}
