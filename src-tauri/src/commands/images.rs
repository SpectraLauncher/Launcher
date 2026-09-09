use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::UNIX_EPOCH;

use sha1::{Digest, Sha1};

use crate::paths;
use crate::error::AppResult;

const CACHE_LIMIT: u64 = 128 * 1024 * 1024;
static GENERATED: AtomicUsize = AtomicUsize::new(0);

pub fn cache_icon(bytes: &[u8], ext: &str) -> Option<String> {
    if bytes.is_empty() {
        return None;
    }
    let mut hash = Sha1::new();
    hash.update(bytes);
    let dir = paths::cache_dir().join("icons");
    let target = dir.join(format!("{:x}.{ext}", hash.finalize()));

    if !target.is_file() {
        std::fs::create_dir_all(&dir).ok()?;
        write_atomic(&target, bytes).ok()?;
    }
    Some(target.to_string_lossy().into_owned())
}

#[tauri::command]
pub async fn get_image_thumbnail(path: String, size: u32) -> AppResult<String> {
    crate::blocking(move || {
        let thumbnail = build_thumbnail(Path::new(&path), size.clamp(32, 512))?;
        Ok(thumbnail.to_string_lossy().into_owned())
    })
    .await
}

fn build_thumbnail(path: &Path, size: u32) -> AppResult<PathBuf> {
    let path = std::fs::canonicalize(path).map_err(|e| format!("thumbnail source: {e}"))?;
    let root = std::fs::canonicalize(paths::data_root())
        .map_err(|e| format!("data directory: {e}"))?;
    if !path.starts_with(&root) {
        return Err("image is outside the launcher directory".into());
    }

    let metadata = std::fs::metadata(&path).map_err(|e| format!("thumbnail source: {e}"))?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    let mut hash = Sha1::new();
    hash.update(path.to_string_lossy().as_bytes());
    hash.update(metadata.len().to_le_bytes());
    hash.update(modified.to_le_bytes());
    hash.update(size.to_le_bytes());

    let dir = paths::cache_dir().join("thumbnails");
    let target = dir.join(format!("{:x}.png", hash.finalize()));
    if target.is_file() {
        return Ok(target);
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("thumbnail cache: {e}"))?;

    let mut reader = image::ImageReader::open(&path)
        .map_err(|e| format!("open image: {e}"))?
        .with_guessed_format()
        .map_err(|e| format!("read image: {e}"))?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(256 * 1024 * 1024);
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    reader.limits(limits);

    let decoded = reader.decode().map_err(|e| format!("decode image: {e}"))?;
    let thumbnail = decoded.thumbnail(size, size);
    drop(decoded);

    let mut encoded = Vec::new();
    thumbnail
        .write_to(&mut std::io::Cursor::new(&mut encoded), image::ImageFormat::Png)
        .map_err(|e| format!("encode thumbnail: {e}"))?;
    write_atomic(&target, &encoded).map_err(|e| format!("write thumbnail: {e}"))?;

    if GENERATED.fetch_add(1, Ordering::Relaxed) % 32 == 0 {
        if let Err(e) = prune(&dir, &target) {
            log::warn!("could not prune the thumbnail cache: {e}");
        }
    }
    Ok(target)
}

fn prune(dir: &Path, keep: &Path) -> std::io::Result<()> {
    let mut entries = Vec::new();
    let mut total = 0u64;
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if !metadata.is_file() {
            continue;
        }
        total += metadata.len();
        entries.push((
            metadata.modified().unwrap_or(UNIX_EPOCH),
            metadata.len(),
            entry.path(),
        ));
    }
    entries.sort_unstable_by_key(|(modified, _, _)| *modified);

    for (_, size, path) in entries {
        if total <= CACHE_LIMIT {
            break;
        }
        if path != keep && std::fs::remove_file(&path).is_ok() {
            total -= size;
        }
    }
    Ok(())
}

fn write_atomic(target: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = target.with_extension(format!("{}.tmp", std::process::id()));
    std::fs::write(&tmp, bytes)?;
    if let Err(e) = std::fs::rename(&tmp, target) {
        let _ = std::fs::remove_file(&tmp);
        if !target.is_file() {
            return Err(e);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{build_thumbnail, cache_icon};

    fn png_bytes() -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(64, 48, image::Rgba([10, 20, 30, 255]));
        let mut out = Vec::new();
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        out
    }

    #[test]
    fn icons_are_deduplicated_and_thumbnails_stay_inside_the_data_root() {
        let _guard = crate::paths::lock_data_dir();
        let root = std::env::temp_dir().join(format!("spectra-img-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        std::env::set_var("SPECTRA_DATA_DIR", &root);

        let bytes = png_bytes();
        let first = cache_icon(&bytes, "png").unwrap();
        let second = cache_icon(&bytes, "png").unwrap();
        assert_eq!(first, second, "identical bytes must land on one cache entry");
        assert!(std::path::Path::new(&first).is_file());
        assert_eq!(cache_icon(&[], "png"), None);

        let source = root.join("shot.png");
        std::fs::write(&source, &bytes).unwrap();
        let thumb = build_thumbnail(&source, 32).unwrap();
        let decoded = image::ImageReader::open(&thumb).unwrap().decode().unwrap();
        assert!(decoded.width() <= 32 && decoded.height() <= 32);
        assert_eq!(
            build_thumbnail(&source, 32).unwrap(),
            thumb,
            "a second call must reuse the cached file"
        );

        let outside = std::env::temp_dir().join(format!("spectra-out-{}.png", uuid::Uuid::new_v4()));
        std::fs::write(&outside, &bytes).unwrap();
        assert!(build_thumbnail(&outside, 32).is_err());

        std::fs::remove_file(&outside).unwrap();
        std::fs::remove_dir_all(&root).unwrap();
        std::env::remove_var("SPECTRA_DATA_DIR");
    }
}
