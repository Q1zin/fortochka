use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::CoreError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceRole {
    Camera,
    Viewer,
}

/// Состояние устройства. Платформа передаёт директорию для хранения
/// (`filesDir` на Android, Application Support на iOS) — ядро не знает,
/// где живут файлы конкретной ОС.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceConfig {
    pub server_url: Option<String>,
    pub role: Option<DeviceRole>,
    // роль «камера»
    pub camera_id: Option<String>,
    pub upload_token: Option<String>,
    pub pairing_code: Option<String>,
    // роль «зритель»
    pub view_token: Option<String>,
}

impl DeviceConfig {
    const FILE_NAME: &'static str = "config.json";

    /// Отсутствующий файл — это не ошибка, а первый запуск.
    pub fn load(dir: &Path) -> Result<Self, CoreError> {
        match fs::read(dir.join(Self::FILE_NAME)) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }

    /// Атомарная запись: сначала во временный файл, затем rename —
    /// процесс, убитый посреди записи, не оставит битый конфиг.
    pub fn save(&self, dir: &Path) -> Result<(), CoreError> {
        fs::create_dir_all(dir)?;
        let tmp = dir.join(format!("{}.tmp", Self::FILE_NAME));
        fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        fs::rename(&tmp, dir.join(Self::FILE_NAME))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_missing_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let config = DeviceConfig::load(dir.path()).unwrap();
        assert_eq!(config, DeviceConfig::default());
    }

    #[test]
    fn save_then_load_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let config = DeviceConfig {
            server_url: Some("https://example.com".into()),
            role: Some(DeviceRole::Viewer),
            view_token: Some("tok".into()),
            ..DeviceConfig::default()
        };
        config.save(dir.path()).unwrap();
        assert_eq!(DeviceConfig::load(dir.path()).unwrap(), config);
    }
}
