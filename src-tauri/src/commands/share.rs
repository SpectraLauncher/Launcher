use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};

use futures_util::TryStreamExt;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::commands::modrinth::InstalledItem;
use crate::commands::{curseforge, import, instances, modrinth};
use crate::models::{Instance, Loader};
use crate::{paths, store};

const SHARE_API: &str = "https://usespectra.app/api/share";

pub(crate) const MANIFEST: &str = "spectra-share.json";
pub(crate) const FORMAT: &str = "spectra-share";

pub(crate) const KEEP_ON_SYNC: &[&str] = &["options.txt", "servers.dat", "servers.dat_old"];

const CONTENT_DIRS: &[&str] = &["mods", "resourcepacks", "shaderpacks", "datapacks"];

const SHARED_DIRS: &[&str] = &["mods", "resourcepacks", "shaderpacks", "datapacks", "config"];

pub(crate) const ICON: &str = "icon.png";

#[derive(Serialize, Deserialize)]
pub(crate) struct ShareManifest {
    pub(crate) format: String,
    pub(crate) version: u32,
    pub(crate) name: String,
    pub(crate) mc_version: String,
    pub(crate) loader: Loader,
    pub(crate) items: Vec<InstalledItem>,
    #[serde(default)]
    pub(crate) unresolved: Vec<String>,
}

#[derive(Serialize, Clone)]
pub struct UnresolvedFile {
    pub path: String,
    pub size: u64,
}

#[derive(Serialize)]
pub struct SharePreview {
    pub modrinth: usize,
    pub curseforge: usize,
    pub unresolved: Vec<UnresolvedFile>,
    pub unresolved_bytes: u64,
}

#[derive(Deserialize, Serialize)]
pub struct ShareResult {
    pub code: String,
    pub url: String,
    pub expires: i64,
    #[serde(default = "one")]
    pub revision: u32,
    #[serde(default)]
    pub pushed: bool,
}

fn one() -> u32 {
    1
}

#[derive(Serialize)]
pub struct ShareImportResult {
    pub instance: Instance,
    pub installed: usize,
    pub failed: Vec<String>,
    pub needs_curseforge: usize,
}

pub(crate) fn folder_for_kind(kind: &str) -> &'static str {
    match kind {
        "resourcepack" => "resourcepacks",
        "shader" => "shaderpacks",
        "datapack" => "datapacks",
        _ => "mods",
    }
}

pub(crate) fn loader_str(loader: &Loader) -> Option<String> {
    match loader {
        Loader::Vanilla => None,
        Loader::Fabric(_) => Some("fabric".into()),
        Loader::Quilt(_) => Some("quilt".into()),
        Loader::Forge(_) => Some("forge".into()),
        Loader::NeoForge(_) => Some("neoforge".into()),
    }
}

async fn link_local_files(id: &str) {
    let _ = modrinth::match_local_mods(id.to_string()).await;
    if curseforge::cf_enabled() {
        let _ = curseforge::curseforge_match_local(id.to_string()).await;
    }
}

pub(crate) fn scan_unresolved(id: &str, items: &[InstalledItem]) -> (Vec<UnresolvedFile>, u64) {
    let known: HashSet<&str> = items.iter().map(|i| i.filename.as_str()).collect();
    let game_dir = paths::instance_game_dir(id);
    let mut out = Vec::new();
    let mut bytes = 0u64;

    for sub in CONTENT_DIRS {
        let Ok(entries) = std::fs::read_dir(game_dir.join(sub)) else { continue };
        for e in entries.flatten() {
            if !e.path().is_file() {
                continue;
            }
            let raw = e.file_name().to_string_lossy().into_owned();
            let base = raw.strip_suffix(".disabled").unwrap_or(&raw);
            if known.contains(base) {
                continue;
            }
            let size = e.metadata().map(|m| m.len()).unwrap_or(0);
            bytes += size;
            out.push(UnresolvedFile { path: format!("{sub}/{raw}"), size });
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    (out, bytes)
}

#[tauri::command]
pub async fn share_preview(id: String) -> Result<SharePreview, String> {
    link_local_files(&id).await;
    let items = modrinth::read_content_index(&id).items;
    let (unresolved, unresolved_bytes) = scan_unresolved(&id, &items);
    Ok(SharePreview {
        curseforge: items.iter().filter(|i| i.provider == "curseforge").count(),
        modrinth: items.iter().filter(|i| i.provider != "curseforge").count(),
        unresolved,
        unresolved_bytes,
    })
}

#[tauri::command]
pub async fn share_instance(
    app: AppHandle,
    id: String,
    include: Vec<String>,
) -> Result<ShareResult, String> {
    // Uploads are tied to an account — the server has no anonymous route any
    // more. Check before packing so a gigabyte of mods is not zipped for nothing.
    let Some(token) = crate::commands::spectra::stored_token() else {
        return Err("sign in to your Spectra account to share an instance".into());
    };

    emit_progress(&app, "scanning", 0, 0);
    link_local_files(&id).await;

    let instance: Instance =
        store::read_json(&paths::instance_config_file(&id))?.ok_or("instance not found")?;
    let items = modrinth::read_content_index(&id).items;
    let (unresolved, _) = scan_unresolved(&id, &items);
    let unresolved_paths: HashSet<String> = unresolved.iter().map(|u| u.path.clone()).collect();
    let include: HashSet<String> =
        include.into_iter().filter(|p| unresolved_paths.contains(p)).collect();

    let tmp = std::env::temp_dir().join(format!("spectra-share-{id}.zip"));
    let manifest = ShareManifest {
        format: FORMAT.into(),
        version: 1,
        name: instance.name.clone(),
        mc_version: instance.mc_version.clone(),
        loader: instance.loader.clone(),
        items: items.clone(),
        unresolved: include.iter().cloned().collect(),
    };

    let total = overrides_bytes(&paths::instance_game_dir(&id), &handled_files(&manifest, &unresolved_paths, &include));
    let mut packed = 0u64;
    {
        let app = app.clone();
        write_pack(&tmp, &manifest, &id, &unresolved_paths, &include, &mut move |bytes| {
            packed += bytes;
            emit_progress(&app, "packing", packed, total);
        })?;
    }

    let result = upload_to_storage(&app, &tmp, &instance, &id, items.len(), &token).await;
    let _ = std::fs::remove_file(&tmp);
    emit_progress(&app, "done", 1, 1);
    result
}

fn emit_progress(app: &AppHandle, stage: &str, current: u64, total: u64) {
    let _ = app.emit(
        "share://progress",
        serde_json::json!({ "stage": stage, "current": current, "total": total }),
    );
}

fn overrides_bytes(game_dir: &std::path::Path, handled: &HashSet<String>) -> u64 {
    fn walk(dir: &std::path::Path, rel: &str, handled: &HashSet<String>, total: &mut u64) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let child = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
            match e.file_type() {
                Ok(t) if t.is_dir() => walk(&e.path(), &child, handled, total),
                Ok(_) if !handled.contains(&child) => {
                    *total += e.metadata().map(|m| m.len()).unwrap_or(0);
                }
                _ => {}
            }
        }
    }
    let mut total = 0;
    for sub in SHARED_DIRS {
        walk(&game_dir.join(sub), sub, handled, &mut total);
    }
    total
}

async fn upload_to_storage(
    app: &AppHandle,
    path: &std::path::Path,
    instance: &Instance,
    id: &str,
    mods: usize,
    token: &str,
) -> Result<ShareResult, String> {
    let size = std::fs::metadata(path).map_err(|e| format!("stat pack: {e}"))?.len();
    let client = crate::http();

    #[derive(Deserialize)]
    struct Ticket {
        code: String,
        revision: u32,
        url: String,
        #[serde(rename = "uploadUrl")]
        upload_url: String,
    }

    let ticket: Ticket = {
        let resp = client
            .post(format!("{SHARE_API}/upload-url"))
            .header("origin", crate::commands::spectra::ORIGIN)
            .bearer_auth(token)
            .json(&serde_json::json!({
                "size": size,
                "name": instance.name,
                "mc": instance.mc_version,
                "loader": loader_str(&instance.loader).unwrap_or_else(|| "vanilla".into()),
                "mods": mods,
                "instance": id,
            }))
            .send()
            .await
            .map_err(|e| format!("upload failed: {e}"))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let detail = resp.text().await.unwrap_or_default();
            return Err(extract_message(&detail).unwrap_or_else(|| format!("upload failed ({status})")));
        }
        resp.json().await.map_err(|e| format!("bad server reply: {e}"))?
    };

    let file = tokio::fs::File::open(path).await.map_err(|e| format!("open pack: {e}"))?;
    let handle = app.clone();
    let mut sent = 0u64;
    let mut last_emit = 0u64;
    let stream = tokio_util::io::ReaderStream::new(file).map_ok(move |chunk| {
        sent += chunk.len() as u64;
        if sent - last_emit >= 2 * 1024 * 1024 || sent == size {
            last_emit = sent;
            emit_progress(&handle, "uploading", sent, size);
        }
        chunk
    });

    let put = client
        .put(&ticket.upload_url)
        .header("content-type", "application/zip")
        .header("content-length", size.to_string())
        .body(reqwest::Body::wrap_stream(stream))
        .send()
        .await
        .map_err(|e| format!("upload failed: {e}"))?;
    if !put.status().is_success() {
        return Err(format!("storage rejected the pack ({})", put.status()));
    }

    emit_progress(app, "finishing", 0, 0);
    let done = client
        .post(format!("{SHARE_API}/{}/complete", ticket.code))
        .header("origin", crate::commands::spectra::ORIGIN)
        .bearer_auth(token)
        .json(&serde_json::json!({ "size": size }))
        .send()
        .await
        .map_err(|e| format!("could not finish the share: {e}"))?;
    if !done.status().is_success() {
        let status = done.status();
        let detail = done.text().await.unwrap_or_default();
        return Err(extract_message(&detail).unwrap_or_else(|| format!("could not finish the share ({status})")));
    }

    #[derive(Deserialize)]
    struct Completed {
        revision: u32,
        expires: i64,
        pushed: bool,
    }
    let completed: Completed = done.json().await.map_err(|e| format!("bad server reply: {e}"))?;

    Ok(ShareResult {
        code: ticket.code,
        url: ticket.url,
        expires: completed.expires,
        revision: completed.revision.max(ticket.revision),
        pushed: completed.pushed,
    })
}

fn handled_files(
    manifest: &ShareManifest,
    unresolved: &HashSet<String>,
    include: &HashSet<String>,
) -> HashSet<String> {
    let mut handled: HashSet<String> = HashSet::new();
    for item in &manifest.items {
        let rel = format!("{}/{}", folder_for_kind(&item.kind), item.filename);
        handled.insert(format!("{rel}.disabled"));
        handled.insert(rel);
    }
    handled.extend(unresolved.difference(include).cloned());
    handled
}

pub(crate) fn write_pack(
    dest: &std::path::Path,
    manifest: &ShareManifest,
    id: &str,
    unresolved: &HashSet<String>,
    include: &HashSet<String>,
    on_file: &mut dyn FnMut(u64),
) -> Result<(), String> {
    let handled = handled_files(manifest, unresolved, include);

    let file = std::fs::File::create(dest).map_err(|e| format!("create pack: {e}"))?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let json = serde_json::to_vec_pretty(manifest).map_err(|e| e.to_string())?;
    zip.start_file(MANIFEST, opts).map_err(|e| e.to_string())?;
    zip.write_all(&json).map_err(|e| e.to_string())?;

    let icon_path = paths::instance_icon_file(id);
    if let Ok(bytes) = std::fs::read(&icon_path) {
        zip.start_file(ICON, opts).map_err(|e| e.to_string())?;
        zip.write_all(&bytes).map_err(|e| e.to_string())?;
    }

    let game_dir = paths::instance_game_dir(id);
    let filter = import::ExportFilter::new(Vec::new(), Vec::new());
    for sub in SHARED_DIRS {
        modrinth::add_overrides(
            &mut zip,
            &game_dir,
            sub,
            &filter,
            &handled,
            true,
            opts,
            on_file,
        )?;
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}

fn extract_message(body: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    v.get("statusMessage")
        .or_else(|| v.get("message"))
        .and_then(|m| m.as_str())
        .map(|s| s.to_string())
}

#[tauri::command]
pub async fn import_share(app: AppHandle, code: String) -> Result<ShareImportResult, String> {
    if code.to_lowercase().contains("curseforge.com") {
        return Err("That's a CurseForge profile link. Spectra can't redeem those — \
                    ask the sender to use the CurseForge app's \"Export profile\" \
                    and import the .zip instead."
            .into());
    }

    let code = normalize_code(&code)?;
    let (manifest, bytes) = fetch_pack(&code).await?;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|e| format!("open pack: {e}"))?;

    let instance = instances::create_instance(
        manifest.name.clone(),
        manifest.mc_version.clone(),
        manifest.loader.clone(),
        None,
        None,
    )?;

    let mut instance = instance;
    if let Ok(mut entry) = archive.by_name(ICON) {
        let mut buf = Vec::new();
        if entry.read_to_end(&mut buf).is_ok()
            && std::fs::write(paths::instance_icon_file(&instance.id), &buf).is_ok()
        {
            instance.icon = Some(ICON.to_string());
            let _ = store::write_json(&paths::instance_config_file(&instance.id), &instance);
        }
    }

    let game_dir = paths::instance_game_dir(&instance.id);
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_dir() {
            continue;
        }
        let Some(rel) = entry.name().strip_prefix("overrides/").map(str::to_string) else { continue };
        let dest = import::join_safe(&game_dir, &rel)?;
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        std::fs::write(&dest, &buf).map_err(|e| format!("write {rel}: {e}"))?;
    }

    let cf_enabled = curseforge::cf_enabled();
    let loader = loader_str(&manifest.loader);
    let mc = Some(manifest.mc_version.clone());
    let total = manifest.items.len() as u64;
    let mut installed = 0usize;
    let mut failed: Vec<String> = Vec::new();
    let mut needs_curseforge = 0usize;

    for (i, item) in manifest.items.iter().enumerate() {
        let result = if item.provider == "curseforge" {
            if !cf_enabled {
                needs_curseforge += 1;
                continue;
            }
            curseforge::curseforge_install_with_deps(
                instance.id.clone(),
                item.project_id.clone(),
                item.version_id.clone(),
                mc.clone(),
                loader.clone(),
            )
            .await
            .map(|_| ())
        } else {
            modrinth::modrinth_install_with_deps(
                instance.id.clone(),
                item.version_id.clone(),
                mc.clone(),
                loader.clone(),
            )
            .await
            .map(|_| ())
        };

        match result {
            Ok(()) => installed += 1,
            Err(e) => {
                log::warn!("share: {} failed: {e}", item.name);
                failed.push(item.name.clone());
            }
        }

        let _ = app.emit(
            "modrinth://modpack-progress",
            serde_json::json!({
                "instance_id": instance.id,
                "name": manifest.name,
                "current": i as u64 + 1,
                "total": total,
            }),
        );
    }

    if let Ok(revision) = revision_of(&code).await {
        instance.share_origin = Some(crate::models::ShareOrigin {
            code: code.clone(),
            revision,
            item_ids: manifest.items.iter().map(|i| i.project_id.clone()).collect(),
        });
        let _ = store::write_json(&paths::instance_config_file(&instance.id), &instance);
    }

    Ok(ShareImportResult { instance, installed, failed, needs_curseforge })
}

fn normalize_code(raw: &str) -> Result<String, String> {
    let code: String = raw
        .trim()
        .to_uppercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    let code = if code.len() > 6 { code[code.len() - 6..].to_string() } else { code };
    if code.len() != 6 {
        return Err("a share code is 6 characters".into());
    }
    Ok(code)
}

async fn revision_of(code: &str) -> Result<u32, String> {
    let resp = crate::http()
        .get(format!("{SHARE_API}/{code}"))
        .query(&[("meta", "1")])
        .header("origin", crate::commands::spectra::ORIGIN)
        .send()
        .await
        .map_err(|e| format!("network error: {e}"))?;
    if !resp.status().is_success() {
        return Err("this code does not exist or has expired".into());
    }
    let meta: ShareMeta = resp.json().await.map_err(|e| format!("bad server reply: {e}"))?;
    Ok(meta.revision)
}

#[derive(Serialize)]
pub struct ShareSyncResult {
    pub revision: u32,
    pub installed: usize,
    pub removed: usize,
    pub failed: Vec<String>,
    pub needs_curseforge: usize,
}

#[derive(Deserialize)]
struct ShareMeta {
    #[serde(default = "one")]
    revision: u32,
}

async fn fetch_pack(code: &str) -> Result<(ShareManifest, Vec<u8>), String> {
    let client = crate::http();
    let mut req = client
        .get(format!("{SHARE_API}/{code}"))
        .query(&[("url", "1")])
        .header("origin", crate::commands::spectra::ORIGIN);
    if let Some(token) = crate::commands::spectra::stored_token() {
        req = req.bearer_auth(token);
    }
    let resp = req.send().await.map_err(|e| format!("download failed: {e}"))?;
    if !resp.status().is_success() {
        return Err("this code does not exist or has expired".into());
    }

    #[derive(Deserialize)]
    struct Address {
        url: String,
    }

    let json = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|t| t.contains("json"));

    let bytes = if json {
        let address: Address = resp.json().await.map_err(|e| format!("bad server reply: {e}"))?;
        let stored = client
            .get(address.url)
            .send()
            .await
            .map_err(|e| format!("download failed: {e}"))?;
        if !stored.status().is_success() {
            return Err(format!("storage refused the download ({})", stored.status()));
        }
        stored.bytes().await.map_err(|e| e.to_string())?.to_vec()
    } else {
        resp.bytes().await.map_err(|e| e.to_string())?.to_vec()
    };

    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes.clone()))
        .map_err(|e| format!("open pack: {e}"))?;
    let manifest: ShareManifest = {
        let mut f = archive
            .by_name(MANIFEST)
            .map_err(|_| "not a Spectra share pack".to_string())?;
        let mut s = String::new();
        f.read_to_string(&mut s).map_err(|e| e.to_string())?;
        serde_json::from_str(&s).map_err(|e| format!("parse manifest: {e}"))?
    };
    if manifest.format != FORMAT {
        return Err("not a Spectra share pack".into());
    }
    Ok((manifest, bytes))
}

pub(crate) struct SyncPlan<'a> {
    pub(crate) install: Vec<&'a InstalledItem>,
    pub(crate) remove: Vec<&'a InstalledItem>,
}

pub(crate) fn plan_sync<'a>(
    wanted: &'a [InstalledItem],
    installed: &'a [InstalledItem],
    from_share: &HashSet<String>,
) -> SyncPlan<'a> {
    let have: HashMap<&str, &str> = installed
        .iter()
        .map(|i| (i.project_id.as_str(), i.version_id.as_str()))
        .collect();
    let keep: HashSet<&str> = wanted.iter().map(|i| i.project_id.as_str()).collect();

    SyncPlan {
        install: wanted
            .iter()
            .filter(|i| have.get(i.project_id.as_str()) != Some(&i.version_id.as_str()))
            .collect(),
        remove: installed
            .iter()
            .filter(|i| !keep.contains(i.project_id.as_str()) && from_share.contains(&i.project_id))
            .collect(),
    }
}

#[tauri::command]
pub async fn sync_share(app: AppHandle, id: String, code: String) -> Result<ShareSyncResult, String> {
    let code = normalize_code(&code)?;

    let mut instance: Instance =
        store::read_json(&paths::instance_config_file(&id))?.ok_or("instance not found")?;

    let revision = revision_of(&code).await?;

    let (manifest, bytes) = fetch_pack(&code).await?;

    crate::commands::snapshots::snapshot_before(&app, &id, "before update").await;

    let owned: HashSet<String> = instance
        .share_origin
        .as_ref()
        .map(|o| o.item_ids.iter().cloned().collect())
        .unwrap_or_default();

    let installed_now = modrinth::read_content_index(&id).items;
    let SyncPlan { install: todo, remove } = plan_sync(&manifest.items, &installed_now, &owned);

    let mut removed = 0usize;
    for item in remove {
        let path = paths::instance_game_dir(&id)
            .join(folder_for_kind(&item.kind))
            .join(&item.filename);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("jar.disabled"));
        modrinth::remove_index_entry(&id, &item.filename);
        removed += 1;
    }

    let game_dir = paths::instance_game_dir(&id);
    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| format!("open pack: {e}"))?;

    if let Ok(mut entry) = archive.by_name(ICON) {
        let mut buf = Vec::new();
        if entry.read_to_end(&mut buf).is_ok()
            && std::fs::write(paths::instance_icon_file(&id), &buf).is_ok()
        {
            instance.icon = Some(ICON.to_string());
        }
    }

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_dir() {
            continue;
        }
        let Some(rel) = entry.name().strip_prefix("overrides/").map(str::to_string) else { continue };
        if KEEP_ON_SYNC.iter().any(|k| rel.eq_ignore_ascii_case(k)) {
            continue;
        }
        let dest = import::join_safe(&game_dir, &rel)?;
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        std::fs::write(&dest, &buf).map_err(|e| format!("write {rel}: {e}"))?;
    }

    let cf_enabled = curseforge::cf_enabled();
    let loader = loader_str(&manifest.loader);
    let mc = Some(manifest.mc_version.clone());
    let total = todo.len() as u64;
    let mut installed = 0usize;
    let mut failed: Vec<String> = Vec::new();
    let mut needs_curseforge = 0usize;

    for (i, item) in todo.iter().enumerate() {
        let result = if item.provider == "curseforge" {
            if !cf_enabled {
                needs_curseforge += 1;
                continue;
            }
            curseforge::curseforge_install_with_deps(
                id.clone(),
                item.project_id.clone(),
                item.version_id.clone(),
                mc.clone(),
                loader.clone(),
            )
            .await
            .map(|_| ())
        } else {
            modrinth::modrinth_install_with_deps(
                id.clone(),
                item.version_id.clone(),
                mc.clone(),
                loader.clone(),
            )
            .await
            .map(|_| ())
        };

        match result {
            Ok(()) => installed += 1,
            Err(e) => {
                log::warn!("sync: {} failed: {e}", item.name);
                failed.push(item.name.clone());
            }
        }

        let _ = app.emit(
            "modrinth://modpack-progress",
            serde_json::json!({
                "instance_id": id,
                "name": manifest.name,
                "current": i as u64 + 1,
                "total": total,
            }),
        );
    }

    instance.share_origin = Some(crate::models::ShareOrigin {
        code: code.clone(),
        revision,
        item_ids: manifest.items.iter().map(|i| i.project_id.clone()).collect(),
    });
    store::write_json(&paths::instance_config_file(&id), &instance)?;

    Ok(ShareSyncResult { revision, installed, removed, failed, needs_curseforge })
}

pub fn code_from_url(url: &str) -> Option<String> {
    let rest = url.strip_prefix("spectra://")?.trim_start_matches('/');
    let rest = rest.strip_prefix("share/").or_else(|| rest.strip_prefix("share"))?;
    let code: String = rest.trim_matches('/').chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    (code.len() == 6).then(|| code.to_uppercase())
}

#[tauri::command]
pub fn take_pending_share(state: tauri::State<'_, crate::AppState>) -> Option<String> {
    state.pending_share.lock().ok()?.take()
}

#[cfg(test)]
mod tests {
    use super::{code_from_url, plan_sync};
    use crate::commands::modrinth::InstalledItem;
    use std::collections::HashSet;

    fn item(project: &str, version: &str) -> InstalledItem {
        InstalledItem {
            project_id: project.into(),
            version_id: version.into(),
            kind: "mod".into(),
            name: project.into(),
            filename: format!("{project}.jar"),
            version_number: version.into(),
            ..Default::default()
        }
    }

    #[test]
    fn a_sync_leaves_the_players_own_mods_alone() {
        let wanted = vec![item("b", "v2")];
        let installed = vec![item("a", "v1"), item("b", "v1"), item("c", "v1")];
        let from_share: HashSet<String> = ["a".to_string(), "b".to_string()].into_iter().collect();

        let plan = plan_sync(&wanted, &installed, &from_share);

        assert_eq!(plan.install.len(), 1, "only the bumped item is reinstalled");
        assert_eq!(plan.install[0].project_id, "b");
        assert_eq!(plan.remove.len(), 1, "only the author's dropped item goes");
        assert_eq!(plan.remove[0].project_id, "a");
    }

    #[test]
    fn an_unchanged_pack_is_a_no_op() {
        let wanted = vec![item("a", "v1")];
        let installed = vec![item("a", "v1")];
        let from_share: HashSet<String> = ["a".to_string()].into_iter().collect();

        let plan = plan_sync(&wanted, &installed, &from_share);

        assert!(plan.install.is_empty());
        assert!(plan.remove.is_empty());
    }

    #[test]
    fn parses_deep_links() {
        assert_eq!(code_from_url("spectra://share/ABC123"), Some("ABC123".into()));
        assert_eq!(code_from_url("spectra://share/abc123/"), Some("ABC123".into()));
        assert_eq!(code_from_url("spectra://share?x=1"), None);
        assert_eq!(code_from_url("spectra://share/AB"), None);
        assert_eq!(code_from_url("https://usespectra.app/s/ABC123"), None);
    }
}
