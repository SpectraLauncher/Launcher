use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use tauri::State;

use crate::commands::{spectra, sync};
use crate::error::{AppError, AppResult};
use crate::models::{Instance, Loader};
use crate::{paths, store, AppState};

const API: &str = concat!(crate::spectra_site!(), "/api/me/launcher-sync");
const MAX_ARCHIVE: u64 = 512 * 1024 * 1024;
const MAX_FILE: u64 = 256 * 1024 * 1024;

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
            return ids.contains(id) && (matches!(file, "options.json" | "servers.dat" | "command_history.txt" | "hotbar.nbt" | "icon.png")
                || file.strip_prefix("resourcepacks/").is_some_and(|pack| pack.len() <= 128 && is_zip(pack) && safe_name(pack)));
        }
    }
    false
}

fn archive_target(name: &str) -> PathBuf {
    if let Some(rest) = name.strip_prefix("instances/") {
        let (id, file) = rest.split_once('/').unwrap();
        if file == "icon.png" { return paths::instance_icon_file(id); }
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
    }
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
        let snapshot = build_snapshot().unwrap();
        assert!(snapshot.entries.iter().all(|entry| !entry.name.contains("/mods/") && !entry.name.contains("/saves/")));
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
        let mut second_state = LocalState::default();
        apply_archive(&archive, &mut second_state).unwrap();
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
        assert!(!archive_name_allowed(&format!("instances/{id}/../../accounts.json"), &ids));
        assert!(!archive_name_allowed("shared/../accounts.json", &ids));
    }
}
