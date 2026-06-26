use std::path::{Path, PathBuf};

use anyhow::Context;
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};

/// Конфигурация из окружения — единственный источник настроек,
/// чтобы Docker-деплой конфигурировался без файлов.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub bind_addr: String,
    pub data_dir: PathBuf,
}

impl ServerConfig {
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            bind_addr: std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into()),
            data_dir: std::env::var("DATA_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("./data")),
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub data_dir: PathBuf,
}

impl AppState {
    /// Создаёт директорию данных, открывает SQLite (WAL — чтобы читатели
    /// обоев не блокировались загрузкой кадров) и накатывает схему.
    pub async fn init(data_dir: &Path) -> anyhow::Result<Self> {
        tokio::fs::create_dir_all(data_dir)
            .await
            .with_context(|| format!("не удалось создать {}", data_dir.display()))?;

        let options = SqliteConnectOptions::new()
            .filename(data_dir.join("fortochka.db"))
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal);
        let db = SqlitePool::connect_with(options)
            .await
            .context("не удалось открыть SQLite")?;

        sqlx::raw_sql(include_str!("schema.sql"))
            .execute(&db)
            .await
            .context("не удалось применить схему БД")?;

        Ok(Self {
            db,
            data_dir: data_dir.to_owned(),
        })
    }
}
