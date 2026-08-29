use serde::de::DeserializeOwned;
use serde::Serialize;
use std::path::Path;

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let value = serde_json::from_slice(&bytes)
        .map_err(|e| format!("parse {}: {e}", path.display()))?;
    Ok(Some(value))
}

#[cfg(unix)]
fn restrict(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn restrict(_path: &Path) {}

pub fn write_json_private<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    write_json(path, value)?;
    restrict(path);
    Ok(())
}

pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;

    let json = serde_json::to_vec_pretty(value).map_err(|e| format!("serialize: {e}"))?;

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let file_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = parent.join(format!(".{file_name}.{stamp}.{:x}.tmp", std::process::id()));

    std::fs::write(&tmp, &json).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    restrict(&tmp);
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("rename {} -> {}: {e}", tmp.display(), path.display()));
    }
    Ok(())
}
