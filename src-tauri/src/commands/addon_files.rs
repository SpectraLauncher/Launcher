use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use serde::Serialize;

use crate::error::{AppError, AppResult};

pub const LIMIT: usize = 10 * 1024 * 1024;
const MAX_ENTRIES: usize = 5000;
const NO_WRITE_FOLDERS: &[&str] = &["mods", "kubejs", "scripts", "coremods"];
const CODE_EXTENSIONS: &[&str] = &[
    "jar", "class", "dll", "so", "dylib", "exe", "msi", "com", "scr", "bat", "cmd", "ps1", "psm1", "vbs", "sh",
    "command", "app", "lnk",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Read,
    Write,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct DirEntry {
    pub name: String,
    pub dir: bool,
    pub size: u64,
    pub modified: u64,
}

pub fn valid_folder(folder: &str) -> bool {
    !folder.is_empty()
        && folder.len() <= 64
        && !folder.starts_with('.')
        && !folder.ends_with('.')
        && folder.bytes().all(|c| c.is_ascii_alphanumeric() || b"_-.".contains(&c))
}

pub fn writable_folder(folder: &str) -> bool {
    valid_folder(folder) && !NO_WRITE_FOLDERS.contains(&folder.to_ascii_lowercase().as_str())
}

pub fn valid_permission(permission: &str) -> bool {
    match (permission.strip_prefix("files:read:"), permission.strip_prefix("files:write:")) {
        (Some(folder), _) => valid_folder(folder),
        (_, Some(folder)) => writable_folder(folder),
        _ => false,
    }
}

fn denied(message: impl Into<String>) -> AppError {
    AppError::new("permission_denied", message)
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

pub fn resolve(game: &Path, path: &str, granted: &[String], access: Access) -> AppResult<PathBuf> {
    let clean = crate::commands::addons::safe_name(path) && path.split('/').all(|p| !p.ends_with('.') && !p.ends_with(' '));
    if !clean {
        return Err(AppError::invalid("path has to be a relative path inside the instance, like config/example.json"));
    }
    let folder = path.split('/').next().unwrap_or_default();
    let has = |kind: &str| granted.iter().any(|p| p == &format!("files:{kind}:{folder}"));
    let allowed = match access {
        Access::Read => has("read") || has("write"),
        Access::Write => has("write") && writable_folder(folder),
    };
    if !allowed {
        let kind = if access == Access::Read { "read" } else { "write" };
        return Err(denied(format!("{path} needs the files:{kind}:{folder} permission")));
    }
    if access == Access::Write {
        let name = path.rsplit('/').next().unwrap_or_default().to_ascii_lowercase();
        if name.rsplit_once('.').is_some_and(|(_, ext)| CODE_EXTENSIONS.contains(&ext)) {
            return Err(denied(format!("addons cannot write program files like {name}")));
        }
    }

    let full = game.join(path);
    let base = game.join(folder);
    if base.exists() {
        let base_real = base.canonicalize().map_err(|e| e.to_string())?;
        let mut anchor = full.as_path();
        while !anchor.exists() {
            anchor = anchor.parent().ok_or_else(|| AppError::invalid("path is outside the instance"))?;
        }
        let anchor_real = anchor.canonicalize().map_err(|e| e.to_string())?;
        if !anchor_real.starts_with(&base_real) {
            return Err(denied(format!("{path} leads outside {folder}")));
        }
    }
    Ok(full)
}

pub fn list(dir: &Path) -> AppResult<Vec<DirEntry>> {
    let mut out = Vec::new();
    let entries = std::fs::read_dir(dir).map_err(|_| AppError::not_found("no such folder"))?;
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        let modified = meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis() as u64);
        out.push(DirEntry { name: entry.file_name().to_string_lossy().into_owned(), dir: meta.is_dir(), size: if meta.is_dir() { 0 } else { meta.len() }, modified });
        if out.len() >= MAX_ENTRIES {
            break;
        }
    }
    out.sort_by(|a, b| b.dir.cmp(&a.dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(out)
}

pub fn read(file: &Path, base64: bool) -> AppResult<String> {
    if !file.is_file() {
        return Err(AppError::not_found("no such file"));
    }
    let mut bytes = Vec::new();
    std::fs::File::open(file).map_err(|e| e.to_string())?.take(LIMIT as u64 + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() > LIMIT {
        return Err(AppError::invalid(format!("the file is larger than {} MB", LIMIT / 1024 / 1024)));
    }
    if base64 {
        return Ok(base64::engine::general_purpose::STANDARD.encode(bytes));
    }
    String::from_utf8(bytes).map_err(|_| AppError::invalid("the file is not text; read it as base64"))
}

fn trash_target(trash: &Path, rel: &str) -> PathBuf {
    let unique = uuid::Uuid::new_v4().simple().to_string();
    trash.join(format!("{}-{}", now_ms(), &unique[..8])).join(rel)
}

fn move_to_trash(file: &Path, trash: &Path, rel: &str) -> AppResult<()> {
    let target = trash_target(trash, rel);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::rename(file, &target).map_err(|e| format!("move {rel} aside: {e}"))?;
    Ok(())
}

pub fn decode(data: &str, base64: bool) -> AppResult<Vec<u8>> {
    let bytes = if base64 {
        base64::engine::general_purpose::STANDARD.decode(data.trim()).map_err(|_| AppError::invalid("data is not base64"))?
    } else {
        data.as_bytes().to_vec()
    };
    if bytes.len() > LIMIT {
        return Err(AppError::invalid(format!("data is larger than {} MB", LIMIT / 1024 / 1024)));
    }
    Ok(bytes)
}

pub fn write(file: &Path, bytes: &[u8], trash: &Path, rel: &str) -> AppResult<()> {
    if file.is_dir() {
        return Err(AppError::invalid(format!("{rel} is a folder")));
    }
    if file.is_file() {
        if std::fs::read(file).is_ok_and(|old| old == bytes) {
            return Ok(());
        }
        let target = trash_target(trash, rel);
        std::fs::create_dir_all(target.parent().unwrap_or(trash)).map_err(|e| e.to_string())?;
        std::fs::copy(file, &target).map_err(|e| format!("keep the old {rel}: {e}"))?;
    }
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(file, bytes).map_err(|e| format!("write {rel}: {e}"))?;
    Ok(())
}

pub fn remove(file: &Path, trash: &Path, rel: &str) -> AppResult<()> {
    if !file.exists() {
        return Err(AppError::not_found("no such file"));
    }
    move_to_trash(file, trash, rel)
}

pub fn mkdir(dir: &Path) -> AppResult<()> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> PathBuf {
        let root = std::env::temp_dir().join(format!("spectra-files-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("minecraft/config")).unwrap();
        std::fs::create_dir_all(root.join("minecraft/mods")).unwrap();
        std::fs::write(root.join("minecraft/options.txt"), "fov:0.0\n").unwrap();
        root
    }

    fn granted(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn permissions_name_one_top_level_folder_and_never_mods() {
        for ok in ["files:read:config", "files:write:config", "files:read:mods", "files:write:options.txt", "files:write:XaeroWaypoints"] {
            assert!(valid_permission(ok), "{ok}");
        }
        for bad in ["files:read", "files:read:", "files:write:mods", "files:write:MODS", "files:write:kubejs", "files:read:../x", "files:read:config/sub", "files:read:.fabric", "files:write:mods.", "files:read:a b", "files:delete:config"] {
            assert!(!valid_permission(bad), "{bad}");
        }
    }

    #[test]
    fn paths_stay_inside_the_granted_folder() {
        let root = scratch();
        let game = root.join("minecraft");
        let read = granted(&["files:read:config"]);
        assert!(resolve(&game, "config/a.json", &read, Access::Read).is_ok());
        assert_eq!(resolve(&game, "config/a.json", &read, Access::Write).unwrap_err().code, "permission_denied");
        assert_eq!(resolve(&game, "options.txt", &read, Access::Read).unwrap_err().code, "permission_denied");
        for bad in ["../x", "config/../../x", "/etc/passwd", "C:/Windows", "config\\a", "config/a.json.", "config/a ", ""] {
            assert!(resolve(&game, bad, &read, Access::Read).is_err(), "{bad}");
        }

        let write = granted(&["files:write:config", "files:write:mods"]);
        assert!(resolve(&game, "config/new/b.json", &write, Access::Write).is_ok());
        assert!(resolve(&game, "config/b.json", &write, Access::Read).is_ok());
        assert_eq!(resolve(&game, "mods/x.txt", &write, Access::Write).unwrap_err().code, "permission_denied");
        for code in ["config/evil.jar", "config/run.BAT", "config/x.dll"] {
            assert_eq!(resolve(&game, code, &write, Access::Write).unwrap_err().code, "permission_denied", "{code}");
        }
    }

    #[test]
    fn writing_and_removing_keep_the_old_file_aside() {
        let root = scratch();
        let game = root.join("minecraft");
        let trash = root.join("addon-trash/test");
        let file = game.join("config/a.json");

        write(&file, b"one", &trash, "config/a.json").unwrap();
        assert_eq!(read(&file, false).unwrap(), "one");
        assert!(!trash.exists());

        write(&file, b"two", &trash, "config/a.json").unwrap();
        write(&file, b"two", &trash, "config/a.json").unwrap();
        let kept: Vec<_> = walkdir(&trash);
        assert_eq!(kept.len(), 1);
        assert_eq!(std::fs::read_to_string(&kept[0]).unwrap(), "one");

        remove(&file, &trash, "config/a.json").unwrap();
        assert!(!file.exists());
        assert_eq!(walkdir(&trash).len(), 2);
        assert_eq!(remove(&file, &trash, "config/a.json").unwrap_err().code, "not_found");

        std::fs::write(game.join("config/bin.dat"), [0xff, 0xfe, 0x00]).unwrap();
        assert_eq!(read(&game.join("config/bin.dat"), false).unwrap_err().code, "invalid");
        assert_eq!(read(&game.join("config/bin.dat"), true).unwrap(), "//4A");
        assert_eq!(decode("//4A", true).unwrap(), vec![0xff, 0xfe, 0x00]);

        mkdir(&game.join("config/deep/er")).unwrap();
        let entries = list(&game.join("config")).unwrap();
        assert_eq!(entries[0].name, "deep");
        assert!(entries[0].dir);
        assert!(entries.iter().any(|e| e.name == "bin.dat" && e.size == 3));
    }

    fn walkdir(dir: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            if entry.path().is_dir() {
                out.extend(walkdir(&entry.path()));
            } else {
                out.push(entry.path());
            }
        }
        out
    }
}
