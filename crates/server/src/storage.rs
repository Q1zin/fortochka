//! Кадры и клипы лежат на диске (`{data_dir}/frames/{camera_id}.jpg`,
//! `{data_dir}/clips/{camera_id}.mp4`), в БД — только метаданные. `camera_id`
//! всегда берётся из БД (сгенерированный UUID), а не из пользовательского
//! ввода, поэтому path traversal здесь невозможен.

use std::path::{Path, PathBuf};

use tokio::fs;

/// Что камера хранит на сервере: последний кадр и последний видеоклип.
#[derive(Debug, Clone, Copy)]
pub enum Media {
    Frame,
    Clip,
}

fn media_path(data_dir: &Path, camera_id: &str, media: Media) -> PathBuf {
    match media {
        Media::Frame => data_dir.join("frames").join(format!("{camera_id}.jpg")),
        Media::Clip => data_dir.join("clips").join(format!("{camera_id}.mp4")),
    }
}

/// Атомарная замена: пишем во временный файл и переименовываем, чтобы
/// зритель, читающий файл параллельно, не получил половину.
pub async fn store(
    data_dir: &Path,
    camera_id: &str,
    media: Media,
    bytes: &[u8],
) -> std::io::Result<()> {
    let path = media_path(data_dir, camera_id, media);
    let parent = path.parent().expect("media_path всегда имеет родителя");
    fs::create_dir_all(parent).await?;
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes).await?;
    fs::rename(&tmp, &path).await?;
    Ok(())
}

/// Кадр и клип удалённой камеры; чего не было — тоже не ошибка.
pub async fn delete_all(data_dir: &Path, camera_id: &str) -> std::io::Result<()> {
    for media in [Media::Frame, Media::Clip] {
        match fs::remove_file(media_path(data_dir, camera_id, media)).await {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
    }
    Ok(())
}

/// `None` — камера ещё не присылала такого.
pub async fn load(
    data_dir: &Path,
    camera_id: &str,
    media: Media,
) -> std::io::Result<Option<Vec<u8>>> {
    match fs::read(media_path(data_dir, camera_id, media)).await {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}
