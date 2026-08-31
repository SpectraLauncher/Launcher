use std::io::{Cursor, Read, Write};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::commands::instances;
use crate::models::{Instance, Loader};
use crate::{paths, store};

#[derive(Serialize, Clone)]
pub struct ExternalInstance {
    pub launcher: String,
    pub name: String,
    pub path: String,
    pub game_dir: String,
    pub mc_version: Option<String>,
    pub loader: Option<String>,
    pub loader_version: Option<String>,
}

#[tauri::command]
pub fn detect_external_instances() -> Vec<ExternalInstance> {
    let mut out = Vec::new();
    out.extend(scan_prism());
    out.extend(scan_curseforge());
    out.extend(scan_modrinth());
    out
}

#[tauri::command]
pub fn import_external_instance(
    name: String,
    game_dir: String,
    mc_version: String,
    loader: Option<String>,
    loader_version: Option<String>,
) -> Result<Instance, String> {
    if mc_version.trim().is_empty() {
        return Err("could not determine the Minecraft version of this instance".into());
    }
    let src = PathBuf::from(&game_dir);
    if !src.is_dir() {
        return Err("source game directory not found".into());
    }

    let lver = loader_version.filter(|s| !s.trim().is_empty());
    let loader_enum = build_loader(loader.as_deref(), lver);
    let instance = instances::create_instance(name, mc_version, loader_enum, None, None)?;

    let dst = paths::instance_game_dir(&instance.id);
    if let Err(e) = copy_game_dir(&src, &dst) {
        let _ = std::fs::remove_dir_all(paths::instance_dir(&instance.id));
        return Err(e);
    }
    Ok(instance)
}

#[derive(Serialize, Deserialize)]
struct BackupManifest {
    format: String,
    version: u32,
    instance: Instance,
}

const BACKUP_FORMAT: &str = "spectra-instance-backup";
pub const BACKUP_MANIFEST: &str = "spectra-instance.json";

#[derive(Serialize)]
pub struct DirChild {
    name: String,
    is_dir: bool,
    size: u64,
}

const NEVER: &[&str] = &[
    "versions", "libraries", "assets", "bin", "natives", "logs", ".cache",
    ".fabric", ".quilt", ".mixin.out", "asm",
];

#[tauri::command]
pub fn list_dir(id: String, rel: String) -> Result<Vec<DirChild>, String> {
    let base = paths::instance_game_dir(&id);
    let dir = join_safe(&base, &rel)?;
    let mut out = Vec::new();
    for e in std::fs::read_dir(&dir).map_err(|e| format!("read dir: {e}"))?.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if rel.is_empty() && NEVER.contains(&name.to_lowercase().as_str()) {
            continue;
        }
        let path = e.path();
        let is_dir = path.is_dir();
        let size = if is_dir { 0 } else { e.metadata().map(|m| m.len()).unwrap_or(0) };
        out.push(DirChild { name, is_dir, size });
    }
    out.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(out)
}

#[derive(Serialize, Default)]
pub struct DropResult {
    instances: Vec<Instance>,
    added: usize,
    skipped: usize,
}

#[tauri::command]
pub async fn import_dropped(
    app: AppHandle,
    instance_id: Option<String>,
    paths: Vec<String>,
) -> Result<DropResult, String> {
    let mut result = DropResult::default();
    for p in paths {
        let path = PathBuf::from(&p);
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();

        if ext == "mrpack" || (ext == "zip" && is_instance_archive(&path)) {
            match crate::commands::modrinth::import_file(app.clone(), p.clone(), None).await {
                Ok(inst) => result.instances.push(inst),
                Err(_) => result.skipped += 1,
            }
            continue;
        }

        if (ext == "jar" || ext == "zip") && instance_id.is_some() {
            let id = instance_id.as_deref().unwrap();
            if copy_dropped_content(id, &path).is_ok() {
                result.added += 1;
            } else {
                result.skipped += 1;
            }
            continue;
        }
        result.skipped += 1;
    }
    Ok(result)
}

fn is_instance_archive(path: &Path) -> bool {
    let Ok(bytes) = std::fs::read(path) else { return false };
    let Ok(mut z) = zip::ZipArchive::new(Cursor::new(bytes)) else { return false };
    z.by_name("modrinth.index.json").is_ok()
        || z.by_name("manifest.json").is_ok()
        || z.by_name(BACKUP_MANIFEST).is_ok()
}

fn copy_dropped_content(instance_id: &str, path: &Path) -> Result<(), String> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).ok_or("bad filename")?;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let folder = if ext == "jar" { "mods" } else { sniff_zip_folder(path) };
    let dir = paths::instance_game_dir(instance_id).join(folder);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::copy(path, dir.join(&name)).map_err(|e| e.to_string())?;
    Ok(())
}

fn sniff_zip_folder(path: &Path) -> &'static str {
    let Ok(bytes) = std::fs::read(path) else { return "resourcepacks" };
    let Ok(mut z) = zip::ZipArchive::new(Cursor::new(bytes)) else { return "resourcepacks" };
    let mut has_data = false;
    let mut has_assets = false;
    for i in 0..z.len() {
        if let Ok(f) = z.by_index(i) {
            let n = f.name();
            if n.starts_with("shaders/") {
                return "shaderpacks";
            }
            if n.starts_with("data/") {
                has_data = true;
            }
            if n.starts_with("assets/") {
                has_assets = true;
            }
        }
    }
    if has_data && !has_assets {
        "datapacks"
    } else {
        "resourcepacks"
    }
}

pub fn is_never_top(name: &str) -> bool {
    NEVER.contains(&name.to_lowercase().as_str())
}

#[tauri::command]
pub fn write_text_file(path: String, content: String) -> Result<(), String> {
    std::fs::write(&path, content).map_err(|e| format!("write {path}: {e}"))
}

pub struct ExportFilter {
    pub included: std::collections::HashSet<String>,
    pub excluded: std::collections::HashSet<String>,
}

impl ExportFilter {
    pub fn new(include: Vec<String>, exclude: Vec<String>) -> Self {
        Self { included: include.into_iter().collect(), excluded: exclude.into_iter().collect() }
    }

    pub fn includes(&self, rel: &str) -> bool {
        let parts: Vec<&str> = rel.split('/').collect();
        for i in (1..=parts.len()).rev() {
            let p = parts[..i].join("/");
            if self.included.contains(&p) {
                return true;
            }
            if self.excluded.contains(&p) {
                return false;
            }
        }
        true
    }

    pub fn should_descend(&self, dir_rel: &str) -> bool {
        if self.includes(dir_rel) {
            return true;
        }
        let prefix = format!("{dir_rel}/");
        self.included.iter().any(|p| p.starts_with(&prefix))
    }
}

#[tauri::command]
pub fn export_instance(id: String, dest: String, exclude: Vec<String>, include: Vec<String>) -> Result<(), String> {
    let instance: Instance =
        store::read_json(&paths::instance_config_file(&id))?.ok_or("instance not found")?;
    let game_dir = paths::instance_game_dir(&id);
    let filter = ExportFilter::new(include, exclude);

    let file = std::fs::File::create(&dest).map_err(|e| format!("create {dest}: {e}"))?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let manifest = BackupManifest { format: BACKUP_FORMAT.into(), version: 1, instance };
    let manifest_json = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
    zip.start_file(BACKUP_MANIFEST, opts).map_err(|e| e.to_string())?;
    zip.write_all(&manifest_json).map_err(|e| e.to_string())?;

    let icon = paths::instance_icon_file(&id);
    if let Ok(bytes) = std::fs::read(&icon) {
        zip.start_file("icon.png", opts).map_err(|e| e.to_string())?;
        zip.write_all(&bytes).map_err(|e| e.to_string())?;
    }

    zip_game_files(&mut zip, &game_dir, "", &filter, opts)?;
    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}

fn zip_game_files(
    zip: &mut zip::ZipWriter<std::fs::File>,
    base: &Path,
    rel: &str,
    filter: &ExportFilter,
    opts: zip::write::SimpleFileOptions,
) -> Result<(), String> {
    let dir = if rel.is_empty() { base.to_path_buf() } else { base.join(rel) };
    let Ok(entries) = std::fs::read_dir(&dir) else { return Ok(()) };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if rel.is_empty() && NEVER.contains(&name.to_lowercase().as_str()) {
            continue;
        }
        let child_rel = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
        let path = e.path();
        if path.is_dir() {
            if filter.should_descend(&child_rel) {
                zip_game_files(zip, base, &child_rel, filter, opts)?;
            }
        } else if filter.includes(&child_rel) {
            let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
            zip.start_file(format!("minecraft/{child_rel}"), opts).map_err(|e| e.to_string())?;
            zip.write_all(&bytes).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

pub fn is_backup_zip(bytes: &[u8]) -> bool {
    let Ok(mut archive) = zip::ZipArchive::new(Cursor::new(bytes)) else { return false };
    let found = archive.by_name(BACKUP_MANIFEST).is_ok();
    found
}

pub fn restore_backup_from_bytes(bytes: &[u8]) -> Result<Instance, String> {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("open backup: {e}"))?;

    let manifest: BackupManifest = {
        let mut f = archive
            .by_name(BACKUP_MANIFEST)
            .map_err(|_| "not a Spectra backup".to_string())?;
        let mut s = String::new();
        f.read_to_string(&mut s).map_err(|e| e.to_string())?;
        serde_json::from_str(&s).map_err(|e| format!("parse manifest: {e}"))?
    };
    let src = manifest.instance;

    let created = instances::create_instance(
        src.name.clone(),
        src.mc_version.clone(),
        src.loader.clone(),
        None,
        None,
    )?;
    let new_id = created.id.clone();

    let mut instance = Instance {
        id: new_id.clone(),
        created_at: created.created_at,
        last_played: None,
        playtime_seconds: 0,
        icon: None,
        ..src
    };

    let game_dir = paths::instance_game_dir(&new_id);
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        if name == "icon.png" {
            let mut buf = Vec::new();
            if entry.read_to_end(&mut buf).is_ok()
                && std::fs::write(paths::instance_icon_file(&new_id), &buf).is_ok()
            {
                instance.icon = Some("icon.png".into());
            }
            continue;
        }
        if let Some(rel) = name.strip_prefix("minecraft/") {
            let dest = join_safe(&game_dir, rel)?;
            if let Some(p) = dest.parent() {
                std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
            }
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf).map_err(|e| e.to_string())?;
            std::fs::write(&dest, &buf).map_err(|e| e.to_string())?;
        }
    }

    store::write_json(&paths::instance_config_file(&new_id), &instance)?;
    Ok(instance)
}

pub fn join_safe(base: &Path, rel: &str) -> Result<PathBuf, String> {
    let mut out = base.to_path_buf();
    for comp in Path::new(rel).components() {
        match comp {
            Component::Normal(c) => out.push(c),
            Component::CurDir => {}
            _ => return Err(format!("unsafe path in backup: {rel}")),
        }
    }
    Ok(out)
}

fn build_loader(loader: Option<&str>, version: Option<String>) -> Loader {
    match loader.unwrap_or("vanilla") {
        "fabric" => Loader::Fabric(version.unwrap_or_default()),
        "quilt" => Loader::Quilt(version.unwrap_or_default()),
        "forge" => Loader::Forge(version.unwrap_or_default()),
        "neoforge" => Loader::NeoForge(version.unwrap_or_default()),
        _ => Loader::Vanilla,
    }
}

const SKIP: &[&str] = &[
    "versions", "libraries", "assets", "bin", "natives", "logs", ".cache",
    ".fabric", ".quilt", ".mixin.out", "asm", "patchouli_books",
];

fn copy_game_dir(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| format!("create dest: {e}"))?;
    for entry in std::fs::read_dir(src).map_err(|e| format!("read source: {e}"))?.flatten() {
        let name = entry.file_name();
        let lower = name.to_string_lossy().to_lowercase();
        if SKIP.contains(&lower.as_str()) {
            continue;
        }
        let from = entry.path();
        let to = dst.join(&name);
        if from.is_dir() {
            copy_dir_all(&from, &to)?;
        } else {
            std::fs::copy(&from, &to).map_err(|e| format!("copy {}: {e}", from.display()))?;
        }
    }
    Ok(())
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), String> {
    if instances::clone_dir(src, dst) {
        return Ok(());
    }
    std::fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(src).map_err(|e| e.to_string())?.flatten() {
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir_all(&from, &to)?;
        } else {
            std::fs::copy(&from, &to).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn scan_prism() -> Vec<ExternalInstance> {
    let mut out = Vec::new();

    let mut roots: Vec<PathBuf> = Vec::new();

    if let Some(data) = dirs::data_dir() {
        for app in ["PrismLauncher", "PolyMC", "MultiMC"] {
            roots.push(data.join(app).join("instances"));
        }
    }

    #[cfg(target_os = "macos")]
    if let Some(home) = dirs::home_dir() {
        for app in ["PrismLauncher", "PolyMC", "MultiMC"] {
            roots.push(home.join(".local").join("share").join(app).join("instances"));
        }
    }

    for root in roots {
        let Ok(entries) = std::fs::read_dir(&root) else { continue };
        for e in entries.flatten() {
            let dir = e.path();
            let pack = dir.join("mmc-pack.json");
            if !dir.is_dir() || !pack.exists() {
                continue;
            }
            let game = if dir.join(".minecraft").is_dir() {
                dir.join(".minecraft")
            } else if dir.join("minecraft").is_dir() {
                dir.join("minecraft")
            } else {
                continue;
            };
            let name = read_cfg_value(&dir.join("instance.cfg"), "name")
                .unwrap_or_else(|| dir_name(&dir));
            let (mc, loader, lver) = parse_mmc_pack(&pack);
            if out.iter().any(|x: &ExternalInstance| x.path == dir.to_string_lossy()) {
                continue;
            }
            out.push(ExternalInstance {
                launcher: "prism".into(),
                name,
                path: dir.to_string_lossy().into_owned(),
                game_dir: game.to_string_lossy().into_owned(),
                mc_version: mc,
                loader,
                loader_version: lver,
            });
        }
    }
    out
}

fn parse_mmc_pack(path: &Path) -> (Option<String>, Option<String>, Option<String>) {
    let Ok(raw) = std::fs::read_to_string(path) else { return (None, None, None) };
    let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw) else { return (None, None, None) };
    let (mut mc, mut loader, mut lver) = (None, None, None);
    if let Some(comps) = val.get("components").and_then(|c| c.as_array()) {
        for c in comps {
            let uid = c.get("uid").and_then(|u| u.as_str()).unwrap_or("");
            let ver = c.get("version").and_then(|v| v.as_str()).map(String::from);
            match uid {
                "net.minecraft" => mc = ver,
                "net.fabricmc.fabric-loader" => { loader = Some("fabric".into()); lver = ver; }
                "org.quiltmc.quilt-loader" => { loader = Some("quilt".into()); lver = ver; }
                "net.minecraftforge" => { loader = Some("forge".into()); lver = ver; }
                "net.neoforged" => { loader = Some("neoforge".into()); lver = ver; }
                _ => {}
            }
        }
    }
    (mc, loader, lver)
}

fn read_cfg_value(path: &Path, key: &str) -> Option<String> {
    let raw = std::fs::read_to_string(path).ok()?;
    for line in raw.lines() {
        if let Some((k, v)) = line.split_once('=') {
            if k.trim() == key {
                let v = v.trim();
                if !v.is_empty() {
                    return Some(v.to_string());
                }
            }
        }
    }
    None
}

fn scan_curseforge() -> Vec<ExternalInstance> {
    let mut out = Vec::new();
    let mut roots = Vec::new();
    if let Some(d) = dirs::document_dir() {
        roots.push(d.join("Curseforge").join("Minecraft").join("Instances"));
    }
    if let Some(h) = dirs::home_dir() {
        roots.push(h.join("curseforge").join("minecraft").join("Instances"));
    }
    for root in roots {
        let Ok(entries) = std::fs::read_dir(&root) else { continue };
        for e in entries.flatten() {
            let dir = e.path();
            let meta = dir.join("minecraftinstance.json");
            if !dir.is_dir() || !meta.exists() {
                continue;
            }
            let (name, mc, loader, lver) = parse_cf(&meta);
            out.push(ExternalInstance {
                launcher: "curseforge".into(),
                name: name.unwrap_or_else(|| dir_name(&dir)),
                game_dir: dir.to_string_lossy().into_owned(),
                path: dir.to_string_lossy().into_owned(),
                mc_version: mc,
                loader,
                loader_version: lver,
            });
        }
    }
    out
}

fn parse_cf(path: &Path) -> (Option<String>, Option<String>, Option<String>, Option<String>) {
    let Ok(raw) = std::fs::read_to_string(path) else { return (None, None, None, None) };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else { return (None, None, None, None) };
    let name = v.get("name").and_then(|x| x.as_str()).map(String::from);
    let mc = v
        .get("gameVersion")
        .and_then(|x| x.as_str())
        .map(String::from)
        .or_else(|| {
            v.get("baseModLoader")
                .and_then(|b| b.get("minecraftVersion"))
                .and_then(|x| x.as_str())
                .map(String::from)
        });
    let (loader, lver) = v
        .get("baseModLoader")
        .and_then(|b| b.get("name"))
        .and_then(|x| x.as_str())
        .map(parse_cf_loader)
        .unwrap_or((None, None));
    (name, mc, loader, lver)
}

fn parse_cf_loader(name: &str) -> (Option<String>, Option<String>) {
    let (kind, ver) = name.split_once('-').unwrap_or((name, ""));
    let loader = match kind.to_lowercase().as_str() {
        "forge" => "forge",
        "fabric" => "fabric",
        "neoforge" => "neoforge",
        "quilt" => "quilt",
        _ => return (None, None),
    };
    (Some(loader.into()), (!ver.is_empty()).then(|| ver.to_string()))
}

fn scan_modrinth() -> Vec<ExternalInstance> {
    let mut out = Vec::new();
    let Some(data) = dirs::data_dir() else { return out };
    for app in ["ModrinthApp", "com.modrinth.theseus"] {
        let root = data.join(app).join("profiles");
        let Ok(entries) = std::fs::read_dir(&root) else { continue };
        for e in entries.flatten() {
            let dir = e.path();
            let meta = dir.join("profile.json");
            if !dir.is_dir() || !meta.exists() {
                continue;
            }
            let (name, mc, loader, lver) = parse_modrinth_profile(&meta, &dir_name(&dir));
            out.push(ExternalInstance {
                launcher: "modrinth".into(),
                name,
                game_dir: dir.to_string_lossy().into_owned(),
                path: dir.to_string_lossy().into_owned(),
                mc_version: mc,
                loader,
                loader_version: lver,
            });
        }
    }
    out
}

fn parse_modrinth_profile(
    path: &Path,
    fallback: &str,
) -> (String, Option<String>, Option<String>, Option<String>) {
    let Ok(raw) = std::fs::read_to_string(path) else { return (fallback.into(), None, None, None) };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else { return (fallback.into(), None, None, None) };
    let meta = v.get("metadata").unwrap_or(&v);
    let name = meta.get("name").and_then(|x| x.as_str()).map(String::from).unwrap_or_else(|| fallback.into());
    let mc = meta.get("game_version").and_then(|x| x.as_str()).map(String::from);
    let loader = meta
        .get("loader")
        .and_then(|x| x.as_str())
        .map(|s| s.to_lowercase())
        .filter(|s| s != "vanilla");
    let lver = meta.get("loader_version").and_then(|lv| {
        lv.as_str()
            .map(String::from)
            .or_else(|| lv.get("id").and_then(|i| i.as_str()).map(String::from))
    });
    (name, mc, loader, lver)
}

fn dir_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}
