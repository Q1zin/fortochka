//! Кадры лежат на диске (`{data_dir}/frames/{camera_id}.jpg`), в БД — только
//! метаданные. `camera_id` всегда берётся из БД (сгенерированный UUID),
//! а не из пользовательского ввода, поэтому path traversal здесь невозможен.

use std::path::{Path, PathBuf};

use tokio::fs;

fn frame_path(data_dir: &Path, camera_id: &str) -> PathBuf {
    data_dir.join("frames").join(format!("{camera_id}.jpg"))
}

/// Атомарная замена: пишем во временный файл и переименовываем, чтобы
/// зритель, читающий кадр параллельно, не получил половину файла.
pub async fn store_frame(data_dir: &Path, camera_id: &str, jpeg: &[u8]) -> std::io::Result<()> {
    let path = frame_path(data_dir, camera_id);
    let parent = path.parent().expect("frame_path всегда имеет родителя");
    fs::create_dir_all(parent).await?;
    let tmp = path.with_extension("jpg.tmp");
    fs::write(&tmp, jpeg).await?;
    fs::rename(&tmp, &path).await?;
    Ok(())
}

/// `None` — камера ещё не прислала ни одного кадра.
pub async fn load_frame(data_dir: &Path, camera_id: &str) -> std::io::Result<Option<Vec<u8>>> {
    match fs::read(frame_path(data_dir, camera_id)).await {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}
