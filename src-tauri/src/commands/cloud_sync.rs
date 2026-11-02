use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use tauri::State;

use crate::commands::{curseforge, modrinth, spectra, sync};
use crate::commands::modrinth::{ContentIndex, InstalledItem};
use crate::error::{AppError, AppResult};
use crate::models::{Instance, Loader};
use crate::{paths, store, AppState};

const API: &str = concat!(crate::spectra_site!(), "/api/me/launcher-sync");
const MAX_ARCHIVE: u64 = 512 * 1024 * 1024;
const MAX_FILE: u64 = 256 * 1024 * 1024;
const MAX_MOD: u64 = 20 * 1024 * 1024;

fn entry_limit(name: &str) -> u64 {
    if name.contains("/resourcepacks/") { MAX_FILE }
    else if name.ends_with("icon.png") { 5 * 1024 * 1024 }
    else if name.ends_with(".json") { 2 * 1024 * 1024 }
    else { 20 * 1024 * 1024 }
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct LocalState {
    account_id: String,
    enabled: bool,
    revision: u64,
    fingerprint: String,
    known_ids: Vec<String>,
    updated: Option<i64>,
    conflict: bool,
    conflict_revision: Option<u64>,
    /// Instances whose synced mods could not all be downloaded yet.
    pending_content: Vec<String>,
    /// Hand-added mod files this computer received from the cloud, per instance. Only
    /// these are removed when the cloud copy drops them, never the player's own files.
    received_mods: BTreeMap<String, Vec<String>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudView {
    account_id: String,
    enabled: bool,
    revision: u64,
    updated: Option<i64>,
    status: &'static str,
    changed: bool,
}

impl LocalState {
    fn view(&self, status: &'static str, changed: bool) -> CloudView {
        CloudView { account_id: self.account_id.clone(), enabled: self.enabled, revision: self.revision, updated: self.updated, status, changed }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PortableInstance {
    id: String,
    name: String,
    mc_version: String,
    loader: Loader,
    group: Option<String>,
    created_at: String,
    modpack_project_id: Option<String>,
    modpack_version_id: Option<String>,
}

impl From<&Instance> for PortableInstance {
    fn from(i: &Instance) -> Self {
        Self {
            id: i.id.clone(), name: i.name.clone(), mc_version: i.mc_version.clone(),
            loader: i.loader.clone(), group: i.group.clone(), created_at: i.created_at.clone(),
            modpack_project_id: i.modpack_project_id.clone(),
            modpack_version_id: i.modpack_version_id.clone(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct Manifest {
    format: u32,
    instances: Vec<PortableInstance>,
}

#[derive(Clone)]
enum Source { File(PathBuf), Bytes(Vec<u8>) }
#[derive(Clone)]
struct Entry { name: String, source: Source }
#[derive(Clone)]
struct Snapshot { manifest: Manifest, entries: Vec<Entry>, fingerprint: String }

fn state_path() -> PathBuf { paths::data_root().join("cloud-sync.json") }
fn load() -> AppResult<LocalState> { Ok(store::read_json(&state_path())?.unwrap_or_default()) }
fn save(state: &LocalState) -> AppResult<()> { store::write_json(&state_path(), state) }

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && safe_name(id)
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn add_file(entries: &mut Vec<Entry>, name: String, path: PathBuf) {
    if std::fs::symlink_metadata(&path).is_ok_and(|meta| meta.file_type().is_file()) {
        entries.push(Entry { name, source: Source::File(path) });
    }
}

fn is_zip(name: &str) -> bool { name.to_ascii_lowercase().ends_with(".zip") }

fn is_mod_file(name: &str) -> bool {
    name.len() <= 140 && safe_name(name) && (name.ends_with(".jar") || name.ends_with(".jar.disabled"))
}

fn cloud_mods_file(id: &str) -> PathBuf { paths::instance_dir(id).join("cloud-mods.json") }

/// The hand-added mods marked for cloud sync in an instance, by `mods::local_mod_key`.
pub fn cloud_mod_keys(id: &str) -> Vec<String> {
    store::read_json(&cloud_mods_file(id)).ok().flatten().unwrap_or_default()
}

/// The files in an instance's mods folder that match a marked key, as (file name, path).
fn marked_mod_files(id: &str, keys: &[String]) -> Vec<(String, PathBuf)> {
    let indexed: HashSet<String> = modrinth::read_content_index(id).items.into_iter().map(|i| i.filename).collect();
    let Ok(files) = std::fs::read_dir(paths::instance_game_dir(id).join("mods")) else { return Vec::new() };
    let mut out = Vec::new();
    for file in files.flatten() {
        let raw = file.file_name().to_string_lossy().into_owned();
        let base = raw.strip_suffix(".disabled").unwrap_or(&raw);
        let fits = file.metadata().is_ok_and(|meta| meta.is_file() && meta.len() <= MAX_MOD);
        if !fits || !is_mod_file(&raw) || indexed.contains(base) { continue; }
        if keys.contains(&super::mods::local_mod_key(&file.path(), base)) {
            out.push((raw, file.path()));
        }
    }
    out
}

fn add_packs(entries: &mut Vec<Entry>, prefix: &str, dir: PathBuf) -> AppResult<()> {
    if !std::fs::symlink_metadata(&dir).is_ok_and(|meta| meta.file_type().is_dir()) { return Ok(()); }
    if let Ok(files) = std::fs::read_dir(dir) {
        for file in files.flatten() {
            let name = file.file_name().to_string_lossy().into_owned();
            if !is_zip(&name) { continue; }
            if name.len() > 128 || !safe_name(&name) {
                return Err(AppError::invalid(format!("resource pack filename cannot sync: {name}")));
            }
            add_file(entries, format!("{prefix}/{name}"), file.path());
        }
    }
    Ok(())
}

fn safe_name(name: &str) -> bool {
    if name.is_empty() || name == "." || name == ".." || name.ends_with('.') || name.ends_with(' ')
        || name.chars().any(|c| c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')) {
        return false;
    }
    let stem = name.split('.').next().unwrap_or("").to_ascii_lowercase();
    !matches!(stem.as_str(), "con" | "prn" | "aux" | "nul" | "com1" | "com2" | "com3" | "com4" | "com5" | "com6" | "com7" | "com8" | "com9" | "lpt1" | "lpt2" | "lpt3" | "lpt4" | "lpt5" | "lpt6" | "lpt7" | "lpt8" | "lpt9")
}

fn build_snapshot() -> AppResult<Snapshot> {
    let mut instances = Vec::new();
    let mut entries = Vec::new();
    if let Ok(dirs) = std::fs::read_dir(paths::instances_dir()) {
        for dir in dirs.flatten() {
            let id = dir.file_name().to_string_lossy().into_owned();
            if !valid_id(&id) || !dir.file_type().is_ok_and(|kind| kind.is_dir()) { continue; }
            let Some(instance) = store::read_json::<Instance>(&paths::instance_config_file(&id))? else { continue };
            if instance.id != id { continue; }
            if instance.name.len() > 200 || instance.mc_version.len() > 40 || instance.created_at.len() > 80 {
                return Err(AppError::invalid(format!("instance metadata cannot sync: {id}")));
            }
            let options = if std::fs::symlink_metadata(paths::instance_game_dir(&id).join("options.txt"))
                .is_ok_and(|meta| meta.file_type().is_symlink()) {
                BTreeMap::new()
            } else {
                sync::export_cloud_options(&id, &instance.mc_version)
            };
            if !options.is_empty() {
                entries.push(Entry { name: format!("instances/{id}/options.json"),
                    source: Source::Bytes(serde_json::to_vec(&options).map_err(|e| e.to_string())?) });
            }
            let game = paths::instance_game_dir(&id);
            for (name, file) in [
                ("servers.dat", "servers.dat"), ("command_history.txt", "command_history.txt"),
                ("hotbar.nbt", "hotbar.nbt"),
            ] {
                add_file(&mut entries, format!("instances/{id}/{name}"), game.join(file));
            }
            add_file(&mut entries, format!("instances/{id}/icon.png"), paths::instance_icon_file(&id));
            // The list of what was installed from Modrinth and CurseForge, not the files:
            // the other computer downloads them again from the same place.
            add_file(&mut entries, format!("instances/{id}/content.json"), paths::instance_content_index(&id));
            // Hand-added mods go as files, but only the ones the player marked.
            if cloud_mods_file(&id).is_file() {
                add_file(&mut entries, format!("instances/{id}/cloud-mods.json"), cloud_mods_file(&id));
                for (name, path) in marked_mod_files(&id, &cloud_mod_keys(&id)) {
                    add_file(&mut entries, format!("instances/{id}/mods/{name}"), path);
                }
            }
            add_packs(&mut entries, &format!("instances/{id}/resourcepacks"), game.join("resourcepacks"))?;
            instances.push(PortableInstance::from(&instance));
        }
    }
    instances.sort_by(|a, b| a.id.cmp(&b.id));
    if instances.len() > 500 { return Err(AppError::invalid("too many instances for cloud sync")); }
    let shared = paths::shared_sync_dir();
    for file in ["sync.json", "options.json", "servers.json", "command_history.txt", "hotbar.nbt", "resourcepacks.json"] {
        add_file(&mut entries, format!("shared/{file}"), shared.join(file));
    }
    add_packs(&mut entries, "shared/resourcepacks", paths::sync_packs_dir())?;
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    if entries.len() > 9_999 { return Err(AppError::invalid("too many files for cloud sync")); }
    let manifest = Manifest { format: 1, instances };
    let mut hash = Sha1::new();
    hash.update(serde_json::to_vec(&manifest).map_err(|e| e.to_string())?);
    let mut total = 0u64;
    for entry in &entries {
        hash.update(entry.name.as_bytes());
        match &entry.source {
            Source::Bytes(bytes) => {
                if bytes.len() as u64 > entry_limit(&entry.name) { return Err(AppError::invalid(format!("sync file too large: {}", entry.name))); }
                total += bytes.len() as u64; hash.update(bytes);
            }
            Source::File(path) => {
                let size = std::fs::metadata(path)?.len();
                if size > entry_limit(&entry.name) { return Err(AppError::invalid(format!("sync file too large: {}", path.display()))); }
                total += size;
                let mut file = File::open(path)?;
                let mut buffer = [0u8; 65536];
                loop {
                    let count = file.read(&mut buffer)?;
                    if count == 0 { break; }
                    hash.update(&buffer[..count]);
                }
            }
        }
        if total > MAX_ARCHIVE { return Err(AppError::invalid("cloud sync exceeds 512 MB")); }
    }
    Ok(Snapshot { manifest, entries, fingerprint: format!("{:x}", hash.finalize()) })
}

fn write_archive(snapshot: &Snapshot, path: &Path) -> AppResult<u64> {
    let file = File::create(path)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("manifest.json", options).map_err(|e| e.to_string())?;
    zip.write_all(&serde_json::to_vec(&snapshot.manifest).map_err(|e| e.to_string())?)?;
    for entry in &snapshot.entries {
        let method = if entry.name.contains("/resourcepacks/") {
            zip::CompressionMethod::Stored
        } else {
            zip::CompressionMethod::Deflated
        };
        zip.start_file(&entry.name, options.compression_method(method)).map_err(|e| e.to_string())?;
        match &entry.source {
            Source::Bytes(bytes) => zip.write_all(bytes)?,
            Source::File(path) => { std::io::copy(&mut File::open(path)?, &mut zip)?; }
        }
    }
    let file = zip.finish().map_err(|e| e.to_string())?;
    let size = file.metadata()?.len();
    if size > MAX_ARCHIVE { return Err(AppError::invalid("cloud sync archive exceeds 512 MB")); }
    Ok(size)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Remote { revision: u64, updated: Option<i64>, download_url: Option<String> }

async fn api(method: reqwest::Method, path: &str, body: Option<serde_json::Value>, token: &str) -> AppResult<serde_json::Value> {
    let mut req = crate::http().request(method, format!("{API}{path}"))
        .header("origin", spectra::ORIGIN).bearer_auth(token);
    if let Some(body) = body { req = req.json(&body); }
    let response = req.send().await.map_err(|e| AppError::network(e.to_string()))?;
    let status = response.status();
    if !status.is_success() {
        let raw = response.text().await.unwrap_or_default();
        let message = serde_json::from_str::<serde_json::Value>(&raw).ok()
            .and_then(|value| value.get("statusMessage").or_else(|| value.get("message"))
                .and_then(|text| text.as_str()).map(str::to_owned))
            .unwrap_or_else(|| format!("request failed ({status})"));
        return Err(AppError::new(if status == reqwest::StatusCode::CONFLICT { "conflict" } else { "network" },
            message));
    }
    response.json().await.map_err(|e| AppError::network(e.to_string()))
}

async fn remote(token: &str) -> AppResult<Remote> {
    serde_json::from_value(api(reqwest::Method::GET, "", None, token).await?)
        .map_err(|e| AppError::network(e.to_string()))
}

async fn account_id() -> AppResult<Option<String>> {
    Ok(spectra::spectra_session().await?.and_then(|user| user.get("id")?.as_str().map(str::to_owned)))
}

async fn upload(snapshot: &Snapshot, revision: u64, token: &str) -> AppResult<(u64, i64)> {
    let path = paths::data_root().join(format!("cloud-sync-{}.zip", uuid::Uuid::new_v4()));
    let archive_snapshot = snapshot.clone();
    let archive_path = path.clone();
    let size = match crate::blocking(move || write_archive(&archive_snapshot, &archive_path)).await {
        Ok(size) => size,
        Err(error) => { let _ = std::fs::remove_file(&path); return Err(error); }
    };
    let result = async {
        let ticket = api(reqwest::Method::POST, "/upload-url",
            Some(serde_json::json!({ "baseRevision": revision, "size": size })), token).await?;
        let key = ticket["key"].as_str().ok_or_else(|| AppError::network("missing upload key"))?;
        let url = ticket["uploadUrl"].as_str().ok_or_else(|| AppError::network("missing upload URL"))?;
        let transfer = async {
            let file = tokio::fs::File::open(&path).await?;
            let stream = tokio_util::io::ReaderStream::new(file);
            let response = crate::http().put(url).header("content-type", "application/zip")
                .header("content-length", size.to_string()).body(reqwest::Body::wrap_stream(stream))
                .send().await.map_err(|e| AppError::network(e.to_string()))?;
            if !response.status().is_success() { return Err(AppError::network(format!("cloud storage rejected upload ({})", response.status()))); }
            let done = api(reqwest::Method::POST, "/complete",
                Some(serde_json::json!({ "baseRevision": revision, "key": key })), token).await?;
            Ok((done["revision"].as_u64().unwrap_or(revision + 1), done["updated"].as_i64().unwrap_or_default()))
        }.await;
        if transfer.is_err() {
            let _ = api(reqwest::Method::POST, "/cancel",
                Some(serde_json::json!({ "baseRevision": revision, "key": key })), token).await;
        }
        transfer
    }.await;
    let _ = std::fs::remove_file(path);
    result
}

fn archive_name_allowed(name: &str, ids: &HashSet<String>) -> bool {
    if let Some(rest) = name.strip_prefix("shared/") {
        return matches!(rest, "sync.json" | "options.json" | "servers.json" | "command_history.txt" | "hotbar.nbt" | "resourcepacks.json")
            || rest.strip_prefix("resourcepacks/").is_some_and(|file| file.len() <= 128 && is_zip(file) && safe_name(file));
    }
    if let Some(rest) = name.strip_prefix("instances/") {
        if let Some((id, file)) = rest.split_once('/') {
            return ids.contains(id) && (matches!(file, "options.json" | "servers.dat" | "command_history.txt" | "hotbar.nbt" | "icon.png" | "content.json" | "cloud-mods.json")
                || file.strip_prefix("resourcepacks/").is_some_and(|pack| pack.len() <= 128 && is_zip(pack) && safe_name(pack))
                || file.strip_prefix("mods/").is_some_and(is_mod_file));
        }
    }
    false
}

fn archive_target(name: &str) -> PathBuf {
    if let Some(rest) = name.strip_prefix("instances/") {
        let (id, file) = rest.split_once('/').unwrap();
        if file == "icon.png" { return paths::instance_icon_file(id); }
        if file == "content.json" { return paths::instance_content_index(id); }
        if file == "cloud-mods.json" { return cloud_mods_file(id); }
        return paths::instance_game_dir(id).join(file);
    }
    paths::data_root().join(name)
}

fn backup_file(path: &Path, backup_root: &Path) -> AppResult<()> {
    if !path.is_file() { return Ok(()); }
    let relative = path.strip_prefix(paths::data_root()).map_err(|e| AppError::invalid(e.to_string()))?;
    let target = backup_root.join(relative);
    if let Some(parent) = target.parent() { std::fs::create_dir_all(parent)?; }
    std::fs::copy(path, target)?;
    Ok(())
}

fn same_content<R: Read>(archive: &mut R, path: &Path) -> AppResult<bool> {
    let Ok(mut local) = File::open(path) else { return Ok(false) };
    let mut left = [0u8; 65536];
    let mut right = [0u8; 65536];
    loop {
        let from_archive = archive.read(&mut left)?;
        if from_archive == 0 { return Ok(local.read(&mut right[..1])? == 0); }
        if local.read_exact(&mut right[..from_archive]).is_err()
            || left[..from_archive] != right[..from_archive] { return Ok(false); }
    }
}

fn apply_archive(path: &Path, local: &mut LocalState) -> AppResult<()> {
    let mut zip = zip::ZipArchive::new(File::open(path)?).map_err(|e| AppError::invalid(e.to_string()))?;
    if zip.len() > 10_000 { return Err(AppError::invalid("cloud archive has too many files")); }
    let manifest: Manifest = {
        let mut file = zip.by_name("manifest.json").map_err(|e| AppError::invalid(e.to_string()))?;
        if file.size() > 1024 * 1024 { return Err(AppError::invalid("cloud manifest is too large")); }
        serde_json::from_reader(&mut file).map_err(|e| AppError::invalid(e.to_string()))?
    };
    if manifest.format != 1 || manifest.instances.len() > 500 { return Err(AppError::invalid("unsupported cloud archive")); }
    let ids: HashSet<String> = manifest.instances.iter().map(|i| i.id.clone()).collect();
    if ids.len() != manifest.instances.len() || manifest.instances.iter().any(|i| !valid_id(&i.id)
        || i.name.len() > 200 || i.mc_version.len() > 40 || i.created_at.len() > 80) {
        return Err(AppError::invalid("invalid cloud instance list"));
    }
    let mut total = 0u64;
    let mut names = HashSet::new();
    let mut cloud_options = HashMap::new();
    let mut cloud_content: HashMap<String, ContentIndex> = HashMap::new();
    let mut marked_lists: HashSet<String> = HashSet::new();
    let mut incoming_mods: HashMap<String, Vec<String>> = HashMap::new();
    let mut written_mods: HashMap<String, Vec<String>> = HashMap::new();
    for index in 0..zip.len() {
        let mut file = zip.by_index(index).map_err(|e| AppError::invalid(e.to_string()))?;
        if file.name() == "manifest.json" { continue; }
        if !archive_name_allowed(file.name(), &ids) || file.size() > entry_limit(file.name())
            || !names.insert(file.name().to_owned()) {
            return Err(AppError::invalid("invalid cloud archive entry"));
        }
        total += file.size();
        if total > MAX_ARCHIVE { return Err(AppError::invalid("cloud archive is too large")); }
        let name = file.name().to_owned();
        if name.starts_with("instances/") && name.ends_with("/options.json") {
            let values: BTreeMap<String, String> = serde_json::from_reader(&mut file)
                .map_err(|e| AppError::invalid(format!("invalid game settings: {e}")))?;
            cloud_options.insert(name, values);
        } else if name.starts_with("instances/") && name.ends_with("/content.json") {
            let content: ContentIndex = serde_json::from_reader(&mut file)
                .map_err(|e| AppError::invalid(format!("invalid mod list: {e}")))?;
            cloud_content.insert(name.split('/').nth(1).unwrap().to_owned(), content);
        } else if name.starts_with("instances/") && name.ends_with("/cloud-mods.json") {
            let keys: Vec<String> = serde_json::from_reader(&mut file)
                .map_err(|e| AppError::invalid(format!("invalid list of synced mods: {e}")))?;
            if keys.len() > 500 || keys.iter().any(|k| k.len() > 200) {
                return Err(AppError::invalid("invalid list of synced mods"));
            }
            marked_lists.insert(name.split('/').nth(1).unwrap().to_owned());
        } else if let Some((id, mod_file)) = name.strip_prefix("instances/").and_then(|rest| rest.split_once("/mods/")) {
            incoming_mods.entry(id.to_owned()).or_default().push(mod_file.to_owned());
        } else if name.starts_with("shared/") && name.ends_with(".json") {
            let _: serde_json::Value = serde_json::from_reader(&mut file)
                .map_err(|e| AppError::invalid(format!("invalid shared settings: {e}")))?;
        }
    }

    let backup_root = paths::data_root().join("cloud-sync-backups").join(uuid::Uuid::new_v4().to_string());

    // Removed cloud-managed instances are moved out of the list. Their worlds and mods
    // remain in a local backup, so another computer cannot destroy them.
    for id in &local.known_ids {
        if valid_id(id) && !ids.contains(id) && paths::instance_dir(id).is_dir() {
            let backup = backup_root.join("removed-instances").join(id);
            std::fs::create_dir_all(backup.parent().unwrap())?;
            std::fs::rename(paths::instance_dir(id), backup)?;
        }
    }
    for profile in &manifest.instances {
        let id = &profile.id;
        let mut instance = store::read_json::<Instance>(&paths::instance_config_file(id))?.unwrap_or_default();
        let cloud_icon = names.contains(&format!("instances/{id}/icon.png"));
        let same_profile = PortableInstance::from(&instance) == *profile
            && instance.icon.as_deref() == cloud_icon.then_some("icon.png");
        instance.id = id.clone();
        instance.name = profile.name.clone();
        instance.mc_version = profile.mc_version.clone();
        instance.loader = profile.loader.clone();
        instance.group = profile.group.clone();
        instance.created_at = profile.created_at.clone();
        instance.modpack_project_id = profile.modpack_project_id.clone();
        instance.modpack_version_id = profile.modpack_version_id.clone();
        instance.icon = cloud_icon.then(|| "icon.png".to_string());
        std::fs::create_dir_all(paths::instance_game_dir(id))?;
        if !same_profile {
            backup_file(&paths::instance_config_file(id), &backup_root)?;
            store::write_json(&paths::instance_config_file(id), &instance)?;
        }

        if !names.contains(&format!("instances/{id}/options.json"))
            && !sync::export_cloud_options(id, &profile.mc_version).is_empty() {
            backup_file(&paths::instance_game_dir(id).join("options.txt"), &backup_root)?;
            sync::import_cloud_options(id, &profile.mc_version, BTreeMap::new())?;
        }
        for file in ["servers.dat", "command_history.txt", "hotbar.nbt", "icon.png"] {
            if !names.contains(&format!("instances/{id}/{file}")) {
                let target = if file == "icon.png" { paths::instance_icon_file(id) } else { paths::instance_game_dir(id).join(file) };
                backup_file(&target, &backup_root)?;
                if target.is_file() { std::fs::remove_file(target)?; }
            }
        }
        // A mod the other computer uninstalled goes to the backup here as well. Without a
        // list in the archive (an older launcher uploaded it) the local mods are left alone.
        if let Some(cloud) = cloud_content.get(id) {
            for item in modrinth::read_content_index(id).items {
                let Some(folder) = content_folder(&item.kind) else { continue };
                if !safe_name(&item.filename)
                    || cloud.items.iter().any(|c| c.kind == item.kind && c.filename == item.filename) { continue; }
                let dir = paths::instance_game_dir(id).join(folder);
                for file in [dir.join(&item.filename), dir.join(format!("{}.disabled", item.filename))] {
                    backup_file(&file, &backup_root)?;
                    if file.is_file() { std::fs::remove_file(file)?; }
                }
            }
        }
        // Hand-added mods: a file received earlier that the cloud no longer carries is an
        // old build or an unmarked mod, so it goes to the backup. So does the other
        // spelling of a file that was switched on or off on the other computer.
        if marked_lists.contains(id) {
            let incoming = incoming_mods.get(id).cloned().unwrap_or_default();
            let dir = paths::instance_game_dir(id).join("mods");
            let earlier = local.received_mods.get(id).cloned().unwrap_or_default();
            let flipped = incoming.iter().map(|name| name.strip_suffix(".disabled")
                .map(str::to_owned).unwrap_or_else(|| format!("{name}.disabled")));
            let stale: Vec<String> = earlier.iter().filter(|name| !incoming.contains(name)).cloned().chain(flipped).collect();
            for name in stale.iter().filter(|name| is_mod_file(name) && !incoming.contains(name)) {
                backup_file(&dir.join(name), &backup_root)?;
                if dir.join(name).is_file() { std::fs::remove_file(dir.join(name))?; }
            }
        } else if incoming_mods.contains_key(id) {
            return Err(AppError::invalid("cloud archive carries mods without their list"));
        }
        let pack_dir = paths::instance_game_dir(id).join("resourcepacks");
        if let Ok(packs) = std::fs::read_dir(pack_dir) {
            for pack in packs.flatten() {
                let name = pack.file_name().to_string_lossy().into_owned();
                if is_zip(&name) && !names.contains(&format!("instances/{id}/resourcepacks/{name}")) {
                    backup_file(&pack.path(), &backup_root)?;
                    std::fs::remove_file(pack.path())?;
                }
            }
        }
    }
    for file in ["sync.json", "options.json", "servers.json", "command_history.txt", "hotbar.nbt", "resourcepacks.json"] {
        if !names.contains(&format!("shared/{file}")) {
            let target = paths::shared_sync_dir().join(file);
            backup_file(&target, &backup_root)?;
            if target.is_file() { std::fs::remove_file(target)?; }
        }
    }
    if let Ok(packs) = std::fs::read_dir(paths::sync_packs_dir()) {
        for pack in packs.flatten() {
            let name = pack.file_name().to_string_lossy().into_owned();
            if is_zip(&name) && !names.contains(&format!("shared/resourcepacks/{name}")) {
                backup_file(&pack.path(), &backup_root)?;
                std::fs::remove_file(pack.path())?;
            }
        }
    }
    for index in 0..zip.len() {
        let mut file = zip.by_index(index).map_err(|e| AppError::invalid(e.to_string()))?;
        let name = file.name().to_owned();
        if name == "manifest.json" { continue; }
        if name.ends_with("options.json") && name.starts_with("instances/") {
            let id = name.split('/').nth(1).unwrap();
            let profile = manifest.instances.iter().find(|i| i.id == id).unwrap();
            let cloud = cloud_options.remove(&name).unwrap();
            if sync::export_cloud_options(id, &profile.mc_version) != cloud {
                backup_file(&paths::instance_game_dir(id).join("options.txt"), &backup_root)?;
                sync::import_cloud_options(id, &profile.mc_version, cloud)?;
            }
            continue;
        }
        let target = archive_target(&name);
        // The archive uses "instances/<id>/..." and "shared/...", which are
        // the only path forms admitted by archive_name_allowed above.
        if target.is_file() && same_content(&mut file, &target)? { continue; }
        drop(file);
        let mut file = zip.by_index(index).map_err(|e| AppError::invalid(e.to_string()))?;
        if let Some(parent) = target.parent() { std::fs::create_dir_all(parent)?; }
        backup_file(&target, &backup_root)?;
        let mut out = File::create(target)?;
        std::io::copy(&mut file, &mut out)?;
        if let Some((id, mod_file)) = name.strip_prefix("instances/").and_then(|rest| rest.split_once("/mods/")) {
            written_mods.entry(id.to_owned()).or_default().push(mod_file.to_owned());
        }
    }
    // A file only counts as received when it came from the cloud. The computer that
    // marked a mod already had it, so its own build is never treated as removable.
    for id in &marked_lists {
        let incoming = incoming_mods.remove(id).unwrap_or_default();
        let mut received: Vec<String> = local.received_mods.get(id).into_iter().flatten()
            .filter(|name| incoming.contains(name)).cloned().collect();
        for name in written_mods.remove(id).unwrap_or_default() {
            if !received.contains(&name) { received.push(name); }
        }
        local.received_mods.insert(id.clone(), received);
    }
    local.received_mods.retain(|id, _| ids.contains(id));
    local.known_ids = manifest.instances.into_iter().map(|i| i.id).collect();
    Ok(())
}

async fn download(url: &str, local: &mut LocalState) -> AppResult<()> {
    let path = paths::data_root().join(format!("cloud-sync-{}.zip", uuid::Uuid::new_v4()));
    let result = async {
        let response = crate::http().get(url).send().await.map_err(|e| AppError::network(e.to_string()))?;
        if !response.status().is_success() { return Err(AppError::network(format!("cloud download failed ({})", response.status()))); }
        let mut file = tokio::fs::File::create(&path).await?;
        let mut stream = response.bytes_stream();
        let mut size = 0u64;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| AppError::network(e.to_string()))?;
            size += chunk.len() as u64;
            if size > MAX_ARCHIVE { return Err(AppError::invalid("cloud archive exceeds 512 MB")); }
            tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await?;
        }
        drop(file);
        let apply_path = path.clone();
        let mut owned_state = std::mem::take(local);
        owned_state = crate::blocking(move || {
            apply_archive(&apply_path, &mut owned_state)?;
            Ok(owned_state)
        }).await?;
        *local = owned_state;
        Ok(())
    }.await;
    let _ = std::fs::remove_file(path);
    result
}

/// Resource packs are not listed: the archive carries those files itself.
fn content_folder(kind: &str) -> Option<&'static str> {
    match kind {
        "mod" => Some("mods"),
        "shader" => Some("shaderpacks"),
        "datapack" => Some("datapacks"),
        _ => None,
    }
}

fn valid_ref(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric())
}

async fn restore_item(id: &str, item: &InstalledItem) -> AppResult<()> {
    let Some(folder) = content_folder(&item.kind) else { return Ok(()) };
    if !safe_name(&item.filename) || !valid_ref(&item.project_id) || !valid_ref(&item.version_id) {
        return Err(AppError::invalid("unusable entry in the mod list"));
    }
    let dir = paths::instance_game_dir(id).join(folder);
    if dir.join(&item.filename).exists() || dir.join(format!("{}.disabled", item.filename)).exists() {
        return Ok(());
    }
    let source = match item.provider.as_str() {
        "modrinth" => Some(modrinth::installed_file_url(item).await?),
        "curseforge" => curseforge::installed_file_url(id, item).await?,
        _ => None,
    };
    let Some((url, sha1)) = source else { return Ok(()) };
    let bytes = modrinth::download_file(&url).await?;
    if sha1.is_some_and(|expected| !expected.eq_ignore_ascii_case(&format!("{:x}", Sha1::digest(&bytes)))) {
        return Err(AppError::network("the download did not match its checksum"));
    }
    std::fs::create_dir_all(&dir)?;
    let part = dir.join(format!(".{}.part", item.filename));
    std::fs::write(&part, &bytes)?;
    std::fs::rename(&part, dir.join(&item.filename))?;
    Ok(())
}

/// Downloads what the synced mod lists name but this computer lacks. Returns the
/// instances still missing something, for the next tick to try again.
async fn restore_content(ids: Vec<String>) -> Vec<String> {
    let mut pending = Vec::new();
    for id in ids {
        let mut complete = true;
        for item in modrinth::read_content_index(&id).items {
            if let Err(e) = restore_item(&id, &item).await {
                log::warn!("cloud sync could not download {} into {id}: {e}", item.filename);
                complete = false;
            }
        }
        if !complete { pending.push(id); }
    }
    pending
}

fn busy(state: &State<'_, AppState>) -> bool {
    state.running.lock().map(|r| !r.is_empty()).unwrap_or(true)
}

#[tauri::command]
pub async fn cloud_sync_state() -> AppResult<CloudView> {
    let state = load()?;
    Ok(state.view(if state.conflict { "conflict" } else if state.enabled { "idle" } else { "off" }, false))
}

#[tauri::command]
pub async fn cloud_sync_backup_folder() -> AppResult<String> {
    crate::blocking(|| {
        let dir = paths::data_root().join("cloud-sync-backups");
        std::fs::create_dir_all(&dir)?;
        Ok(dir.to_string_lossy().into_owned())
    }).await
}

#[tauri::command]
pub async fn cloud_sync_set_enabled(enabled: bool, app: State<'_, AppState>) -> AppResult<CloudView> {
    let _guard = app.cloud_sync_lock.lock().await;
    let mut state = load()?;
    if enabled {
        let id = account_id().await?.ok_or_else(|| AppError::auth("sign in to Spectra first"))?;
        if state.account_id != id { state = LocalState { account_id: id, ..Default::default() }; }
    }
    state.enabled = enabled;
    state.conflict = false;
    state.conflict_revision = None;
    save(&state)?;
    Ok(state.view(if enabled { "idle" } else { "off" }, false))
}

#[tauri::command]
pub async fn cloud_sync_tick(app: State<'_, AppState>) -> AppResult<CloudView> {
    let _guard = app.cloud_sync_lock.lock().await;
    let mut state = load()?;
    if !state.enabled { return Ok(state.view("off", false)); }
    if busy(&app) { return Ok(state.view("deferred", false)); }
    let Some(id) = account_id().await? else { return Ok(state.view("signed_out", false)); };
    if state.account_id != id {
        state = LocalState { account_id: id, ..Default::default() };
        save(&state)?;
        return Ok(state.view("off", false));
    }
    if !state.pending_content.is_empty() {
        state.pending_content = restore_content(std::mem::take(&mut state.pending_content)).await;
        save(&state)?;
    }
    let token = spectra::stored_token().ok_or_else(|| AppError::auth("sign in to Spectra first"))?;
    let cloud = remote(&token).await?;
    let snapshot = crate::blocking(build_snapshot).await?;
    let local_changed = snapshot.fingerprint != state.fingerprint;
    let cloud_changed = cloud.revision != state.revision;
    if cloud_changed && local_changed && (cloud.revision > 0 && (!snapshot.manifest.instances.is_empty() || !snapshot.entries.is_empty()) || state.revision > 0) {
        state.conflict = true;
        state.conflict_revision = Some(cloud.revision);
        save(&state)?;
        return Ok(state.view("conflict", false));
    }
    if cloud_changed && cloud.revision > 0 {
        let url = cloud.download_url.as_deref().ok_or_else(|| AppError::network("missing cloud archive URL"))?;
        download(url, &mut state).await?;
        state.pending_content = restore_content(state.known_ids.clone()).await;
        state.revision = cloud.revision;
        state.updated = cloud.updated;
        state.fingerprint = crate::blocking(build_snapshot).await?.fingerprint;
        state.conflict = false;
        state.conflict_revision = None;
        save(&state)?;
        return Ok(state.view("synced", true));
    }
    if local_changed {
        let (revision, updated) = upload(&snapshot, cloud.revision, &token).await?;
        state.revision = revision;
        state.updated = Some(updated);
        state.fingerprint = snapshot.fingerprint;
        state.known_ids = snapshot.manifest.instances.iter().map(|i| i.id.clone()).collect();
        state.conflict = false;
        state.conflict_revision = None;
        save(&state)?;
    }
    Ok(state.view("synced", false))
}

#[tauri::command]
pub async fn cloud_sync_set_mod_file(instance_id: String, filename: String, enabled: bool) -> AppResult<()> {
    crate::blocking(move || {
        if !valid_id(&instance_id) || !filename.ends_with(".jar") || !is_mod_file(&filename) {
            return Err(AppError::invalid("this file cannot sync"));
        }
        let dir = paths::instance_game_dir(&instance_id).join("mods");
        let path = [dir.join(&filename), dir.join(format!("{filename}.disabled"))]
            .into_iter().find(|path| path.is_file())
            .ok_or_else(|| AppError::not_found("mod file not found"))?;
        if enabled && std::fs::metadata(&path)?.len() > MAX_MOD {
            return Err(AppError::invalid("mods larger than 20 MB cannot sync"));
        }
        let key = super::mods::local_mod_key(&path, &filename);
        let mut keys = cloud_mod_keys(&instance_id);
        keys.retain(|k| *k != key);
        if enabled { keys.push(key); keys.sort(); }
        store::write_json(&cloud_mods_file(&instance_id), &keys)
    }).await
}

#[tauri::command]
pub async fn cloud_sync_resolve(choice: String, app: State<'_, AppState>) -> AppResult<CloudView> {
    let _guard = app.cloud_sync_lock.lock().await;
    let mut state = load()?;
    if !state.enabled || !state.conflict { return Err(AppError::invalid("no cloud conflict to resolve")); }
    if busy(&app) { return Ok(state.view("deferred", false)); }
    let id = account_id().await?.ok_or_else(|| AppError::auth("sign in to Spectra first"))?;
    if id != state.account_id { return Err(AppError::auth("the Spectra account changed")); }
    let token = spectra::stored_token().ok_or_else(|| AppError::auth("sign in to Spectra first"))?;
    let cloud = remote(&token).await?;
    if choice == "computer" && state.conflict_revision != Some(cloud.revision) {
        state.conflict_revision = Some(cloud.revision);
        save(&state)?;
        return Ok(state.view("conflict", false));
    }
    match choice.as_str() {
        "cloud" => {
            let url = cloud.download_url.as_deref().ok_or_else(|| AppError::network("missing cloud archive URL"))?;
            state.known_ids = crate::blocking(build_snapshot).await?.manifest.instances.into_iter().map(|i| i.id).collect();
            download(url, &mut state).await?;
            state.pending_content = restore_content(state.known_ids.clone()).await;
            state.revision = cloud.revision;
            state.updated = cloud.updated;
            state.fingerprint = crate::blocking(build_snapshot).await?.fingerprint;
            state.conflict = false;
            state.conflict_revision = None;
            save(&state)?;
            Ok(state.view("synced", true))
        }
        "computer" => {
            let snapshot = crate::blocking(build_snapshot).await?;
            let (revision, updated) = upload(&snapshot, cloud.revision, &token).await?;
            state.revision = revision;
            state.updated = Some(updated);
            state.fingerprint = snapshot.fingerprint;
            state.known_ids = snapshot.manifest.instances.iter().map(|i| i.id.clone()).collect();
            state.conflict = false;
            state.conflict_revision = None;
            save(&state)?;
            Ok(state.view("synced", false))
        }
        _ => Err(AppError::invalid("invalid cloud conflict choice")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_round_trip_keeps_machine_settings_local() {
        let _guard = paths::lock_data_dir();
        let root = std::env::temp_dir().join(format!("spectra-cloud-test-{}", uuid::Uuid::new_v4()));
        std::env::set_var("SPECTRA_DATA_DIR", &root);
        let id = uuid::Uuid::new_v4().to_string();
        let mut instance = Instance {
            id: id.clone(), name: "Portable".into(), mc_version: "1.21.4".into(),
            created_at: "2026-01-01T00:00:00Z".into(), java_path: Some("C:/private/java".into()),
            ..Default::default()
        };
        store::write_json(&paths::instance_config_file(&id), &instance).unwrap();
        let game = paths::instance_game_dir(&id);
        std::fs::create_dir_all(&game).unwrap();
        std::fs::write(game.join("options.txt"), "fov:1.0\nversion:123\nresourcePacks:[\"vanilla\"]\n").unwrap();
        std::fs::write(game.join("servers.dat"), b"server list").unwrap();
        std::fs::create_dir_all(game.join("resourcepacks")).unwrap();
        std::fs::write(game.join("resourcepacks").join("Moja paczka.zip"), b"pack bytes").unwrap();
        std::fs::create_dir_all(game.join("mods")).unwrap();
        std::fs::write(game.join("mods").join("private.jar"), b"mod bytes").unwrap();
        std::fs::create_dir_all(game.join("saves")).unwrap();
        std::fs::write(game.join("saves").join("world.dat"), b"world bytes").unwrap();
        let sodium = InstalledItem {
            project_id: "AANobbMI".into(), version_id: "abc123".into(), kind: "mod".into(),
            name: "Sodium".into(), filename: "sodium.jar".into(), provider: "modrinth".into(),
            ..Default::default()
        };
        modrinth::write_content_index(&id, &ContentIndex { items: vec![sodium.clone()] }).unwrap();
        let snapshot = build_snapshot().unwrap();
        assert!(snapshot.entries.iter().all(|entry| !entry.name.contains("/mods/") && !entry.name.contains("/saves/")));
        assert!(snapshot.entries.iter().any(|entry| entry.name == format!("instances/{id}/content.json")));
        let archive = root.join("test.zip");
        write_archive(&snapshot, &archive).unwrap();
        let mut packed = zip::ZipArchive::new(File::open(&archive).unwrap()).unwrap();
        let mut manifest_json = String::new();
        packed.by_name("manifest.json").unwrap().read_to_string(&mut manifest_json).unwrap();
        assert!(!manifest_json.contains("C:/private/java"));
        drop(packed);

        instance.name = "Local edit".into();
        store::write_json(&paths::instance_config_file(&id), &instance).unwrap();
        std::fs::write(game.join("options.txt"), "fov:0.2\nversion:999\n").unwrap();
        std::fs::write(game.join("servers.dat"), b"different servers").unwrap();
        std::fs::remove_file(game.join("resourcepacks").join("Moja paczka.zip")).unwrap();
        let mut state = LocalState::default();
        apply_archive(&archive, &mut state).unwrap();
        let restored = store::read_json::<Instance>(&paths::instance_config_file(&id)).unwrap().unwrap();
        assert_eq!(restored.name, "Portable");
        assert_eq!(restored.java_path.as_deref(), Some("C:/private/java"));
        let options = std::fs::read_to_string(game.join("options.txt")).unwrap();
        assert!(options.contains("fov:1.0"));
        assert!(options.contains("version:999"));
        assert!(options.contains("resourcePacks:[\"vanilla\"]"));
        assert_eq!(std::fs::read(game.join("servers.dat")).unwrap(), b"server list");
        assert_eq!(std::fs::read(game.join("resourcepacks").join("Moja paczka.zip")).unwrap(), b"pack bytes");
        assert_eq!(state.known_ids, vec![id.clone()]);

        let second = std::env::temp_dir().join(format!("spectra-cloud-second-{}", uuid::Uuid::new_v4()));
        std::env::set_var("SPECTRA_DATA_DIR", &second);
        let mods = paths::instance_game_dir(&id).join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(mods.join("uninstalled-there.jar"), b"old").unwrap();
        std::fs::write(mods.join("added-by-hand.jar"), b"manual").unwrap();
        let gone = InstalledItem { filename: "uninstalled-there.jar".into(), ..sodium.clone() };
        modrinth::write_content_index(&id, &ContentIndex { items: vec![gone] }).unwrap();
        let mut second_state = LocalState::default();
        apply_archive(&archive, &mut second_state).unwrap();
        assert!(!mods.join("uninstalled-there.jar").exists());
        assert!(mods.join("added-by-hand.jar").exists());
        let list: Vec<String> = modrinth::read_content_index(&id).items.into_iter().map(|i| i.filename).collect();
        assert_eq!(list, ["sodium.jar"]);
        let copied = store::read_json::<Instance>(&paths::instance_config_file(&id)).unwrap().unwrap();
        assert_eq!(copied.name, "Portable");
        assert_eq!(copied.java_path, None);
        assert_eq!(std::fs::read(paths::instance_game_dir(&id).join("servers.dat")).unwrap(), b"server list");
        assert_eq!(std::fs::read(paths::instance_game_dir(&id).join("resourcepacks").join("Moja paczka.zip")).unwrap(), b"pack bytes");
        std::fs::remove_dir_all(&second).unwrap();
        std::fs::remove_dir_all(&root).unwrap();
        std::env::remove_var("SPECTRA_DATA_DIR");
    }

    #[test]
    fn cloud_archive_rejects_paths_outside_the_allowed_data() {
        let ids = HashSet::from([uuid::Uuid::new_v4().to_string()]);
        let id = ids.iter().next().unwrap();
        assert!(archive_name_allowed(&format!("instances/{id}/servers.dat"), &ids));
        assert!(archive_name_allowed(&format!("instances/{id}/content.json"), &ids));
        assert!(archive_name_allowed(&format!("instances/{id}/mods/sodium.jar"), &ids));
        assert!(!archive_name_allowed(&format!("instances/{id}/mods/../../accounts.json"), &ids));
        assert!(!archive_name_allowed(&format!("instances/{id}/mods/notes.txt"), &ids));
        assert!(!archive_name_allowed(&format!("instances/{id}/../../accounts.json"), &ids));
        assert!(!archive_name_allowed("shared/../accounts.json", &ids));
    }

    fn dev_jar(path: &Path, version: &str) {
        let mut jar = zip::ZipWriter::new(File::create(path).unwrap());
        jar.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default()).unwrap();
        jar.write_all(format!(r#"{{"id":"mydev","name":"My Dev Mod","version":"{version}"}}"#).as_bytes()).unwrap();
        jar.finish().unwrap();
    }

    #[test]
    fn marked_local_mods_follow_new_builds() {
        let _guard = paths::lock_data_dir();
        let first = std::env::temp_dir().join(format!("spectra-cloud-dev-a-{}", uuid::Uuid::new_v4()));
        let second = std::env::temp_dir().join(format!("spectra-cloud-dev-b-{}", uuid::Uuid::new_v4()));
        let id = uuid::Uuid::new_v4().to_string();
        let instance = Instance { id: id.clone(), name: "Dev".into(), mc_version: "1.21.4".into(), ..Default::default() };
        let pack = |root: &Path, file: &str| {
            std::env::set_var("SPECTRA_DATA_DIR", root);
            let archive = root.join(file);
            write_archive(&build_snapshot().unwrap(), &archive).unwrap();
            archive
        };

        std::env::set_var("SPECTRA_DATA_DIR", &first);
        store::write_json(&paths::instance_config_file(&id), &instance).unwrap();
        let mods_a = paths::instance_game_dir(&id).join("mods");
        std::fs::create_dir_all(&mods_a).unwrap();
        dev_jar(&mods_a.join("mydev-1.0.jar"), "1.0");
        std::fs::write(mods_a.join("someone-elses.jar"), b"not marked").unwrap();
        store::write_json(&cloud_mods_file(&id), &vec!["My Dev Mod".to_string()]).unwrap();
        let archive = pack(&first, "one.zip");

        std::env::set_var("SPECTRA_DATA_DIR", &second);
        let mods_b = paths::instance_game_dir(&id).join("mods");
        std::fs::create_dir_all(&mods_b).unwrap();
        std::fs::write(mods_b.join("own-on-b.jar"), b"b's own").unwrap();
        let mut state_b = LocalState::default();
        apply_archive(&archive, &mut state_b).unwrap();
        assert!(mods_b.join("mydev-1.0.jar").is_file());
        assert!(!mods_b.join("someone-elses.jar").exists());
        assert_eq!(state_b.received_mods[&id], ["mydev-1.0.jar"]);

        // A new build with a new file name replaces the old one on the other computer.
        std::env::set_var("SPECTRA_DATA_DIR", &first);
        std::fs::remove_file(mods_a.join("mydev-1.0.jar")).unwrap();
        dev_jar(&mods_a.join("mydev-1.1.jar"), "1.1");
        let archive = pack(&first, "two.zip");
        let mut state_a = LocalState::default();
        apply_archive(&archive, &mut state_a).unwrap();
        assert_eq!(state_a.received_mods[&id], Vec::<String>::new());

        std::env::set_var("SPECTRA_DATA_DIR", &second);
        apply_archive(&archive, &mut state_b).unwrap();
        assert!(!mods_b.join("mydev-1.0.jar").exists());
        assert!(mods_b.join("mydev-1.1.jar").is_file());
        assert!(mods_b.join("own-on-b.jar").is_file());

        // Unmarking it on the other computer never removes the original build.
        store::write_json(&cloud_mods_file(&id), &Vec::<String>::new()).unwrap();
        let archive = pack(&second, "three.zip");
        std::env::set_var("SPECTRA_DATA_DIR", &first);
        apply_archive(&archive, &mut state_a).unwrap();
        assert!(mods_a.join("mydev-1.1.jar").is_file());

        std::fs::remove_dir_all(&first).unwrap();
        std::fs::remove_dir_all(&second).unwrap();
        std::env::remove_var("SPECTRA_DATA_DIR");
    }

    #[test]
    fn mod_list_entries_cannot_reach_outside_the_instance() {
        assert!(valid_ref("AANobbMI") && valid_ref("9362428"));
        assert!(!valid_ref("../../api") && !valid_ref("") && !valid_ref("a/b"));
        assert_eq!(content_folder("mod"), Some("mods"));
        assert_eq!(content_folder("resourcepack"), None);
    }

    #[tokio::test]
    #[ignore = "downloads from Modrinth and CurseForge"]
    async fn restores_mods_from_both_providers() {
        let _guard = paths::lock_data_dir();
        let root = std::env::temp_dir().join(format!("spectra-cloud-restore-{}", uuid::Uuid::new_v4()));
        std::env::set_var("SPECTRA_DATA_DIR", &root);
        let id = "restore-test".to_string();
        let modmenu = InstalledItem {
            project_id: "mOgUt4GM".into(), version_id: "M3KFXLhq".into(), kind: "mod".into(),
            name: "Mod Menu".into(), filename: "modmenu-1.14.15.jar".into(), provider: "modrinth".into(),
            ..Default::default()
        };
        let jei = InstalledItem {
            project_id: "238222".into(), version_id: "9362428".into(), kind: "mod".into(),
            name: "JEI".into(), filename: "jei-1.21.1-forge-19.70.0.465.jar".into(), provider: "curseforge".into(),
            ..Default::default()
        };
        modrinth::write_content_index(&id, &ContentIndex { items: vec![modmenu, jei] }).unwrap();
        assert!(restore_content(vec![id.clone()]).await.is_empty());
        let mods = paths::instance_game_dir(&id).join("mods");
        assert_eq!(std::fs::metadata(mods.join("modmenu-1.14.15.jar")).unwrap().len(), 142243);
        assert_eq!(std::fs::metadata(mods.join("jei-1.21.1-forge-19.70.0.465.jar")).unwrap().len(), 4729562);
        std::fs::remove_dir_all(&root).unwrap();
        std::env::remove_var("SPECTRA_DATA_DIR");
    }
}
