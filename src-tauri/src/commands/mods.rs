use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use base64::Engine;
use serde::Serialize;

use crate::commands::modrinth::{self, InstalledItem};
use crate::paths;

type Jar = zip::ZipArchive<std::fs::File>;

#[derive(Default)]
struct JarMeta {
    name: Option<String>,
    version: Option<String>,
    icon: Option<String>,
}

fn read_local_mod_meta(jar_path: &Path) -> JarMeta {
    let Ok(file) = std::fs::File::open(jar_path) else { return JarMeta::default() };
    let Ok(mut zip) = zip::ZipArchive::new(file) else { return JarMeta::default() };

    let (name, version, icon_path) = fabric_meta(&mut zip)
        .or_else(|| quilt_meta(&mut zip))
        .or_else(|| toml_meta(&mut zip, "META-INF/neoforge.mods.toml"))
        .or_else(|| toml_meta(&mut zip, "META-INF/mods.toml"))
        .or_else(|| mcmod_meta(&mut zip))
        .unwrap_or((None, None, None));

    JarMeta {
        icon: icon_path.and_then(|p| read_zip_image(&mut zip, &p)),
        name,
        version,
    }
}

fn read_entry_string(zip: &mut Jar, name: &str) -> Option<String> {
    let mut f = zip.by_name(name).ok()?;
    let mut s = String::new();
    f.read_to_string(&mut s).ok()?;
    Some(s)
}

fn icon_from_json(v: Option<&serde_json::Value>) -> Option<String> {
    match v {
        Some(serde_json::Value::String(s)) => Some(s.clone()),
        Some(serde_json::Value::Object(map)) => map
            .iter()
            .max_by_key(|(k, _)| k.parse::<u32>().unwrap_or(0))
            .and_then(|(_, val)| val.as_str())
            .map(String::from),
        _ => None,
    }
}

fn clean_version(v: Option<&str>) -> Option<String> {
    v.filter(|s| !s.contains("${") && !s.is_empty()).map(String::from)
}

type MetaTuple = (Option<String>, Option<String>, Option<String>);

fn fabric_meta(zip: &mut Jar) -> Option<MetaTuple> {
    let s = read_entry_string(zip, "fabric.mod.json")?;
    let v: serde_json::Value = serde_json::from_str(&s).ok()?;
    let name = v.get("name").and_then(|x| x.as_str()).map(String::from);
    let version = clean_version(v.get("version").and_then(|x| x.as_str()));
    Some((name, version, icon_from_json(v.get("icon"))))
}

fn quilt_meta(zip: &mut Jar) -> Option<MetaTuple> {
    let s = read_entry_string(zip, "quilt.mod.json")?;
    let v: serde_json::Value = serde_json::from_str(&s).ok()?;
    let ql = v.get("quilt_loader");
    let meta = ql.and_then(|q| q.get("metadata"));
    let name = meta.and_then(|m| m.get("name")).and_then(|x| x.as_str()).map(String::from);
    let version = clean_version(ql.and_then(|q| q.get("version")).and_then(|x| x.as_str()));
    Some((name, version, icon_from_json(meta.and_then(|m| m.get("icon")))))
}

fn toml_meta(zip: &mut Jar, path: &str) -> Option<MetaTuple> {
    let s = read_entry_string(zip, path)?;
    let doc: toml::Value = toml::from_str(&s).ok()?;
    let first = doc.get("mods").and_then(|m| m.as_array()).and_then(|a| a.first());
    let name = first.and_then(|m| m.get("displayName")).and_then(|v| v.as_str()).map(String::from);
    let version = clean_version(first.and_then(|m| m.get("version")).and_then(|v| v.as_str()));
    let logo = first
        .and_then(|m| m.get("logoFile"))
        .and_then(|v| v.as_str())
        .or_else(|| doc.get("logoFile").and_then(|v| v.as_str()))
        .map(String::from);
    Some((name, version, logo))
}

fn mcmod_meta(zip: &mut Jar) -> Option<MetaTuple> {
    let s = read_entry_string(zip, "mcmod.info")?;
    let v: serde_json::Value = serde_json::from_str(&s).ok()?;
    let entry = v
        .as_array()
        .and_then(|a| a.first())
        .or_else(|| v.get("modList").and_then(|m| m.as_array()).and_then(|a| a.first()))?;
    let name = entry.get("name").and_then(|x| x.as_str()).map(String::from);
    let version = clean_version(entry.get("version").and_then(|x| x.as_str()));
    let logo = entry.get("logoFile").and_then(|x| x.as_str()).map(String::from);
    Some((name, version, logo))
}

fn read_zip_image(zip: &mut Jar, path: &str) -> Option<String> {
    let p = path.trim_start_matches('/');
    if p.is_empty() {
        return None;
    }
    let mut buf = Vec::new();
    {
        let mut f = zip.by_name(p).ok()?;
        f.read_to_end(&mut buf).ok()?;
    }
    let lower = p.to_lowercase();
    let mime = if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else {
        "image/png"
    };
    Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(&buf)))
}

#[derive(Serialize)]
pub struct ModEntry {
    filename: String,
    enabled: bool,
    name: Option<String>,
    version: Option<String>,
    version_id: Option<String>,
    icon_url: Option<String>,
    project_id: Option<String>,
    provider: String,
    modified: u64,
}

#[tauri::command]
pub fn list_mods(instance_id: String) -> Result<Vec<ModEntry>, String> {
    list_content(instance_id, "mod".into())
}

fn kind_layout(kind: &str) -> (&'static str, &'static str) {
    match kind {
        "resourcepack" => ("resourcepacks", ".zip"),
        "shader" => ("shaderpacks", ".zip"),
        "datapack" => ("datapacks", ".zip"),
        _ => ("mods", ".jar"),
    }
}

#[tauri::command]
pub fn list_content(instance_id: String, kind: String) -> Result<Vec<ModEntry>, String> {
    let (folder, ext) = kind_layout(&kind);
    let dir = paths::instance_game_dir(&instance_id).join(folder);
    let index = modrinth::installed_items(&instance_id);
    let by_file: HashMap<&str, &InstalledItem> = index.iter().map(|i| (i.filename.as_str(), i)).collect();

    let mut out = Vec::new();
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Ok(out),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_dir = path.is_dir();
        if !is_dir && !path.is_file() {
            continue;
        }
        let raw = entry.file_name().to_string_lossy().into_owned();
        let (filename, enabled) = match raw.strip_suffix(".disabled") {
            Some(base) => (base.to_string(), false),
            None => (raw.clone(), true),
        };
        if is_dir {
            if ext == ".jar" {
                continue;
            }
        } else if !filename.ends_with(ext) {
            continue;
        }
        let meta = by_file.get(filename.as_str());
        let modified = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let local = if meta.is_none() && !is_dir && ext == ".jar" {
            read_local_mod_meta(&path)
        } else {
            JarMeta::default()
        };

        out.push(ModEntry {
            name: meta.map(|m| m.name.clone()).or(local.name),
            version: meta.map(|m| m.version_number.clone()).or(local.version),
            version_id: meta.map(|m| m.version_id.clone()),
            icon_url: meta.and_then(|m| m.icon_url.clone()).or(local.icon),
            provider: meta.map(|m| m.provider.clone()).unwrap_or_else(|| "local".into()),
            project_id: meta.map(|m| m.project_id.clone()),
            enabled,
            filename,
            modified,
        });
    }
    out.sort_by(|a, b| {
        let an = a.name.as_deref().unwrap_or(&a.filename).to_lowercase();
        let bn = b.name.as_deref().unwrap_or(&b.filename).to_lowercase();
        an.cmp(&bn)
    });
    Ok(out)
}

#[tauri::command]
pub fn set_mod_enabled(instance_id: String, filename: String, enabled: bool) -> Result<(), String> {
    let dir = paths::instance_game_dir(&instance_id).join("mods");
    let on = dir.join(&filename);
    let off = dir.join(format!("{filename}.disabled"));
    if enabled {
        if off.exists() {
            std::fs::rename(&off, &on).map_err(|e| e.to_string())?;
        }
    } else if on.exists() {
        std::fs::rename(&on, &off).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn delete_mod(instance_id: String, filename: String) -> Result<(), String> {
    let dir = paths::instance_game_dir(&instance_id).join("mods");
    for p in [dir.join(&filename), dir.join(format!("{filename}.disabled"))] {
        if p.exists() {
            std::fs::remove_file(&p).map_err(|e| e.to_string())?;
        }
    }
    modrinth::remove_index_entry(&instance_id, &filename);
    Ok(())
}
