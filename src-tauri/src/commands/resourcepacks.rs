use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{AppError, AppResult};

pub const EDIT_FILE: &str = "spectra-edit.json";
const READ_LIMIT: u64 = 4 * 1024 * 1024;
const MAX_FILES: usize = 30_000;
const SUFFIX: &str = " (edited)";

#[derive(Debug, Serialize)]
pub struct PackFile {
    pub path: String,
    pub size: u64,
}

#[derive(Debug, Serialize)]
pub struct PackListing {
    pub files: Vec<PackFile>,
    pub mcmeta: Value,
    pub edit: Option<EditInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EditInfo {
    pub source: String,
    pub excluded: Vec<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Saved {
    pub filename: String,
}

fn plain_name(name: &str) -> AppResult<&str> {
    let bad = name.is_empty()
        || name == "."
        || name == ".."
        || name.contains(['/', '\\', ':'])
        || name.ends_with(".disabled");
    if bad {
        return Err(AppError::invalid("filename has to be the name of a resource pack"));
    }
    Ok(name)
}

fn inner_path(path: &str) -> AppResult<&str> {
    if !crate::commands::addons::safe_name(path) {
        return Err(AppError::invalid("path has to be a relative path inside the pack"));
    }
    Ok(path)
}

fn locate(folder: &Path, filename: &str) -> AppResult<PathBuf> {
    let name = plain_name(filename)?;
    [folder.join(name), folder.join(format!("{name}.disabled"))]
        .into_iter()
        .find(|p| p.exists())
        .ok_or_else(|| AppError::not_found(format!("no resource pack named {name}")))
}

fn open_zip(path: &Path) -> AppResult<zip::ZipArchive<std::fs::File>> {
    let file = std::fs::File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    zip::ZipArchive::new(file).map_err(|e| AppError::invalid(format!("the pack is not a readable zip: {e}")))
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) -> AppResult<()> {
    for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())?.flatten() {
        let Ok(kind) = entry.file_type() else { continue };
        let path = entry.path();
        if kind.is_dir() {
            walk(root, &path, out)?;
        } else if kind.is_file() {
            let Ok(rel) = path.strip_prefix(root) else { continue };
            let rel = rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
            out.push((rel, path));
        }
        if out.len() > MAX_FILES {
            return Err(AppError::invalid(format!("the pack has more than {MAX_FILES} files")));
        }
    }
    Ok(())
}

fn read_entry(pack: &Path, path: &str, limit: u64) -> AppResult<Option<Vec<u8>>> {
    let mut bytes = Vec::new();
    if pack.is_dir() {
        let file = pack.join(path);
        if !file.is_file() {
            return Ok(None);
        }
        std::fs::File::open(&file).map_err(|e| e.to_string())?.take(limit + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    } else {
        let mut archive = open_zip(pack)?;
        let Ok(entry) = archive.by_name(path) else { return Ok(None) };
        if entry.is_dir() {
            return Ok(None);
        }
        if entry.size() > limit {
            return Err(AppError::invalid(format!("{path} is larger than {} MB", limit / 1024 / 1024)));
        }
        entry.take(limit + 1).read_to_end(&mut bytes).map_err(|e| format!("read {path}: {e}"))?;
    }
    if bytes.len() as u64 > limit {
        return Err(AppError::invalid(format!("{path} is larger than {} MB", limit / 1024 / 1024)));
    }
    Ok(Some(bytes))
}

fn json_entry(pack: &Path, path: &str) -> Option<Value> {
    let bytes = read_entry(pack, path, 1024 * 1024).ok()??;
    serde_json::from_slice(bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes)).ok()
}

pub fn list(folder: &Path, filename: &str) -> AppResult<PackListing> {
    let pack = locate(folder, filename)?;
    let mut files = Vec::new();
    if pack.is_dir() {
        let mut found = Vec::new();
        walk(&pack, &pack, &mut found)?;
        for (path, file) in found {
            let size = file.metadata().map(|m| m.len()).unwrap_or(0);
            files.push(PackFile { path, size });
        }
    } else {
        let mut archive = open_zip(&pack)?;
        if archive.len() > MAX_FILES {
            return Err(AppError::invalid(format!("the pack has more than {MAX_FILES} files")));
        }
        for i in 0..archive.len() {
            let entry = archive.by_index_raw(i).map_err(|e| e.to_string())?;
            if !entry.is_dir() {
                files.push(PackFile { path: entry.name().to_string(), size: entry.size() });
            }
        }
    }
    files.retain(|f| f.path != EDIT_FILE);
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let edit = json_entry(&pack, EDIT_FILE).and_then(|v| serde_json::from_value(v).ok());
    Ok(PackListing { files, mcmeta: json_entry(&pack, "pack.mcmeta").unwrap_or(Value::Null), edit })
}

pub fn read(folder: &Path, filename: &str, path: &str) -> AppResult<String> {
    let pack = locate(folder, filename)?;
    let bytes = read_entry(&pack, inner_path(path)?, READ_LIMIT)?.ok_or_else(|| AppError::not_found(format!("{path} is not in the pack")))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

fn copy_name(filename: &str) -> String {
    let stem = filename.strip_suffix(".zip").unwrap_or(filename);
    let stem = stem.strip_suffix(SUFFIX).unwrap_or(stem);
    format!("{stem}{SUFFIX}.zip")
}

fn write_copy(pack: &Path, target: &Path, excluded: &BTreeSet<String>, info: &EditInfo) -> AppResult<()> {
    let file = std::fs::File::create(target).map_err(|e| format!("create {}: {e}", target.display()))?;
    let mut writer = zip::ZipWriter::new(file);
    let keep = |name: &str| name != EDIT_FILE && !excluded.contains(name);
    if pack.is_dir() {
        let mut found = Vec::new();
        walk(pack, pack, &mut found)?;
        let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (rel, path) in found.into_iter().filter(|(rel, _)| keep(rel)) {
            writer.start_file(rel.as_str(), options).map_err(|e| e.to_string())?;
            let mut source = std::fs::File::open(&path).map_err(|e| e.to_string())?;
            std::io::copy(&mut source, &mut writer).map_err(|e| e.to_string())?;
        }
    } else {
        let mut archive = open_zip(pack)?;
        for i in 0..archive.len() {
            let entry = archive.by_index_raw(i).map_err(|e| e.to_string())?;
            if keep(entry.name()) {
                writer.raw_copy_file(entry).map_err(|e| e.to_string())?;
            }
        }
    }
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    writer.start_file(EDIT_FILE, options).map_err(|e| e.to_string())?;
    writer.write_all(&serde_json::to_vec_pretty(info)?).map_err(|e| e.to_string())?;
    writer.finish().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn swap_in_options(options: &str, from: &str, to: &str) -> Option<String> {
    let (from, to) = (format!("file/{from}"), format!("file/{to}"));
    let mut changed = false;
    let lines: Vec<String> = options
        .split('\n')
        .map(|line| {
            let body = line.strip_suffix('\r').unwrap_or(line);
            let Some((key, value)) = body.split_once(':') else { return line.to_string() };
            if key != "resourcePacks" && key != "incompatibleResourcePacks" {
                return line.to_string();
            }
            let Ok(packs) = serde_json::from_str::<Vec<String>>(value) else { return line.to_string() };
            if !packs.contains(&from) {
                return line.to_string();
            }
            let mut next: Vec<String> = Vec::new();
            for pack in packs {
                let pack = if pack == from { to.clone() } else { pack };
                if !next.contains(&pack) {
                    next.push(pack);
                }
            }
            changed = true;
            let tail = if line.ends_with('\r') { "\r" } else { "" };
            format!("{key}:{}{tail}", serde_json::to_string(&next).unwrap_or_default())
        })
        .collect();
    changed.then(|| lines.join("\n"))
}

pub fn save(game: &Path, filename: &str, excluded: &[String]) -> AppResult<Saved> {
    let folder = game.join("resourcepacks");
    let pack = locate(&folder, filename)?;
    for path in excluded {
        inner_path(path)?;
    }
    let previous: Option<EditInfo> = json_entry(&pack, EDIT_FILE).and_then(|v| serde_json::from_value(v).ok());
    let mut all: BTreeSet<String> = excluded.iter().filter(|p| p.as_str() != "pack.mcmeta").cloned().collect();
    if let Some(previous) = &previous {
        all.extend(previous.excluded.iter().cloned());
    }
    let info = EditInfo {
        source: previous.as_ref().map_or_else(|| filename.to_string(), |p| p.source.clone()),
        excluded: all.iter().cloned().collect(),
    };

    let name = copy_name(filename);
    let target = folder.join(&name);
    let existing = [target.clone(), folder.join(format!("{name}.disabled"))].into_iter().find(|p| p.exists());
    if let Some(existing) = &existing {
        let ours = json_entry(existing, EDIT_FILE)
            .and_then(|v| serde_json::from_value::<EditInfo>(v).ok())
            .is_some_and(|e| e.source == info.source);
        if !ours {
            return Err(AppError::invalid(format!("{name} already exists and was not made by the pack editor")));
        }
    }

    let temp = folder.join(format!(".{name}.tmp"));
    if let Err(e) = write_copy(&pack, &temp, &all, &info) {
        let _ = std::fs::remove_file(&temp);
        return Err(e);
    }
    if let Some(existing) = existing {
        std::fs::remove_file(&existing).map_err(|e| format!("replace {name}: {e}"))?;
    }
    std::fs::rename(&temp, &target).map_err(|e| format!("save {name}: {e}"))?;

    if name != filename {
        if pack == folder.join(filename) {
            std::fs::rename(&pack, folder.join(format!("{filename}.disabled"))).map_err(|e| format!("turn off {filename}: {e}"))?;
        }
        let options = game.join("options.txt");
        if let Ok(text) = std::fs::read_to_string(&options) {
            if let Some(next) = swap_in_options(&text, filename, &name) {
                std::fs::write(&options, next).map_err(|e| format!("options.txt: {e}"))?;
            }
        }
    }
    Ok(Saved { filename: name })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack_zip(path: &Path, files: &[(&str, &str)]) {
        let mut writer = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        for (name, body) in files {
            writer.start_file(*name, options).unwrap();
            writer.write_all(body.as_bytes()).unwrap();
        }
        writer.finish().unwrap();
    }

    fn scratch() -> PathBuf {
        let root = std::env::temp_dir().join(format!("spectra-packs-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn names(listing: &PackListing) -> Vec<&str> {
        listing.files.iter().map(|f| f.path.as_str()).collect()
    }

    const FILES: &[(&str, &str)] = &[
        ("pack.mcmeta", "{\"pack\":{\"pack_format\":34,\"description\":\"Test\"}}"),
        ("pack.png", "png"),
        ("assets/minecraft/textures/block/stone.png", "stone"),
        ("assets/minecraft/textures/block/dirt.png", "dirt"),
        ("assets/minecraft/models/block/dirt.json", "{}"),
    ];

    #[test]
    fn a_zip_pack_lists_and_reads_its_files() {
        let dir = scratch();
        pack_zip(&dir.as_path().join("Test.zip"), FILES);
        let listing = list(dir.as_path(), "Test.zip").unwrap();
        assert_eq!(names(&listing).len(), 5);
        assert_eq!(listing.mcmeta["pack"]["pack_format"], 34);
        assert!(listing.edit.is_none());
        assert_eq!(read(dir.as_path(), "Test.zip", "assets/minecraft/textures/block/dirt.png").unwrap(), "ZGlydA==");
        assert_eq!(read(dir.as_path(), "Test.zip", "nope.png").unwrap_err().code, "not_found");
        assert_eq!(read(dir.as_path(), "Test.zip", "../secret").unwrap_err().code, "invalid");
        assert_eq!(list(dir.as_path(), "../Test.zip").unwrap_err().code, "invalid");
    }

    #[test]
    fn a_folder_pack_works_the_same() {
        let dir = scratch();
        let root = dir.as_path().join("Folder pack");
        for (name, body) in FILES {
            let path = root.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, body).unwrap();
        }
        let listing = list(dir.as_path(), "Folder pack").unwrap();
        assert!(names(&listing).contains(&"assets/minecraft/models/block/dirt.json"));
        assert_eq!(read(dir.as_path(), "Folder pack", "pack.png").unwrap(), "cG5n");
    }

    #[test]
    fn saving_makes_a_copy_turns_the_original_off_and_takes_its_place_in_the_game() {
        let dir = scratch();
        let folder = dir.as_path().join("resourcepacks");
        std::fs::create_dir_all(&folder).unwrap();
        pack_zip(&folder.join("Test.zip"), FILES);
        std::fs::write(dir.as_path().join("options.txt"), "fov:0.0\nresourcePacks:[\"vanilla\",\"file/Test.zip\",\"file/Other.zip\"]\nlang:pl_pl\n").unwrap();

        let dirt = vec!["assets/minecraft/textures/block/dirt.png".to_string(), "assets/minecraft/models/block/dirt.json".to_string()];
        let saved = save(dir.as_path(), "Test.zip", &dirt).unwrap();
        assert_eq!(saved.filename, "Test (edited).zip");
        assert!(folder.join("Test.zip.disabled").exists());
        assert!(!folder.join("Test.zip").exists());

        let copy = list(&folder, "Test (edited).zip").unwrap();
        assert_eq!(names(&copy), vec!["assets/minecraft/textures/block/stone.png", "pack.mcmeta", "pack.png"]);
        let edit = copy.edit.unwrap();
        assert_eq!(edit.source, "Test.zip");
        assert_eq!(edit.excluded.len(), 2);
        let options = std::fs::read_to_string(dir.as_path().join("options.txt")).unwrap();
        assert!(options.contains("resourcePacks:[\"vanilla\",\"file/Test (edited).zip\",\"file/Other.zip\"]\n"));

        let again = save(dir.as_path(), "Test.zip", &["assets/minecraft/textures/block/stone.png".to_string()]).unwrap();
        assert_eq!(again.filename, "Test (edited).zip");
        let copy = list(&folder, "Test (edited).zip").unwrap();
        assert_eq!(names(&copy), vec!["assets/minecraft/models/block/dirt.json", "assets/minecraft/textures/block/dirt.png", "pack.mcmeta", "pack.png"]);
        assert_eq!(copy.edit.unwrap().excluded, vec!["assets/minecraft/textures/block/stone.png"]);
        assert!(!folder.read_dir().unwrap().flatten().any(|e| e.file_name().to_string_lossy().ends_with(".tmp")));
    }

    #[test]
    fn a_copy_without_its_original_keeps_what_it_already_lost() {
        let dir = scratch();
        let folder = dir.as_path().join("resourcepacks");
        std::fs::create_dir_all(&folder).unwrap();
        pack_zip(&folder.join("Test.zip"), FILES);
        save(dir.as_path(), "Test.zip", &["assets/minecraft/textures/block/dirt.png".to_string()]).unwrap();
        std::fs::remove_file(folder.join("Test.zip.disabled")).unwrap();

        save(dir.as_path(), "Test (edited).zip", &["pack.mcmeta".to_string(), "pack.png".to_string()]).unwrap();
        let copy = list(&folder, "Test (edited).zip").unwrap();
        assert!(names(&copy).contains(&"pack.mcmeta"));
        assert!(!names(&copy).contains(&"pack.png"));
        let edit = copy.edit.unwrap();
        assert_eq!(edit.source, "Test.zip");
        assert_eq!(edit.excluded, vec!["assets/minecraft/textures/block/dirt.png", "pack.png"]);
    }

    #[test]
    fn a_pack_with_the_copy_name_that_is_not_ours_is_left_alone() {
        let dir = scratch();
        let folder = dir.as_path().join("resourcepacks");
        std::fs::create_dir_all(&folder).unwrap();
        pack_zip(&folder.join("Test.zip"), FILES);
        pack_zip(&folder.join("Test (edited).zip"), FILES);
        assert_eq!(save(dir.as_path(), "Test.zip", &[]).unwrap_err().code, "invalid");
        assert!(folder.join("Test.zip").exists());
    }

    #[test]
    fn options_keep_order_and_line_endings() {
        let text = "a:1\r\nresourcePacks:[\"file/X.zip\",\"file/X (edited).zip\"]\r\nincompatibleResourcePacks:[]\r\n";
        assert_eq!(swap_in_options(text, "X.zip", "X (edited).zip").unwrap(), "a:1\r\nresourcePacks:[\"file/X (edited).zip\"]\r\nincompatibleResourcePacks:[]\r\n");
        assert_eq!(swap_in_options("resourcePacks:[\"vanilla\"]\n", "X.zip", "Y.zip"), None);
    }
}
