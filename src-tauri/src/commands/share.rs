//! Sharing an instance with a short code.
//!
//! The wire format is a small `.zip` holding `spectra-share.json` (the instance's
//! content index — every mod recorded with **its own** provider and exact version
//! id — plus MC version and loader) and an `overrides/` tree with everything that
//! isn't a downloadable project: `servers.dat`, `config/`, `options.txt`, …
//!
//! Deliberately *not* a `.mrpack`: an mrpack can only reference Modrinth, so a
//! CurseForge-heavy pack would balloon into hundreds of megabytes of bundled jars
//! and then show up on the receiving end as if it came from Modrinth. Here every
//! item keeps its origin, so a 150-mod pack travels as ~200 KB and the receiver's
//! update checks keep working.
//!
//! Server: Spectra-Web (`server/api/share.post.ts`) → SQLite → code valid 7 days.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};

use futures_util::TryStreamExt;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::commands::modrinth::InstalledItem;
use crate::commands::{curseforge, import, instances, modrinth};
use crate::models::{Instance, Loader};
use crate::{paths, store};

const SHARE_API: &str = "https://spectra.makoto.com.pl/api/share";
/// Soft anti-spam key — must match `SPECTRA_INGEST_KEY` on the server. Not a secret.
const INGEST_KEY: &str = "uaH8U5Gh1ecZdQQCRsvkGo2ARFByk641CYYy7YAYw";

const MANIFEST: &str = "spectra-share.json";
const FORMAT: &str = "spectra-share";

/// Overrides that are never overwritten when *updating* an instance someone
/// else shared. They are the player's own settings, not the author's content.
const KEEP_ON_SYNC: &[&str] = &["options.txt", "servers.dat", "servers.dat_old"];

/// Folders scanned for content that could be recorded as a project instead of
/// being shipped byte-for-byte.
const CONTENT_DIRS: &[&str] = &["mods", "resourcepacks", "shaderpacks", "datapacks"];

/// The only folders a share carries. A whitelist rather than a list of things
/// to skip: mods invent their own caches constantly (one instance had 910 MB of
/// generated terrain and 7 930 files of datapack cache), and a pack that ships
/// what the receiver cannot regenerate is exactly these five plus the manifest.
const SHARED_DIRS: &[&str] = &["mods", "resourcepacks", "shaderpacks", "datapacks", "config"];

/// The instance icon travels beside the manifest, so a shared pack arrives
/// looking like the original instead of a blank tile.
const ICON: &str = "icon.png";

#[derive(Serialize, Deserialize)]
struct ShareManifest {
    format: String,
    version: u32,
    name: String,
    mc_version: String,
    loader: Loader,
    /// One entry per installed project, each keeping its own `provider`.
    items: Vec<InstalledItem>,
    /// Files that matched no provider, by game-dir path (for display only —
    /// their bytes are in `overrides/` if the sharer opted in).
    #[serde(default)]
    unresolved: Vec<String>,
}

/// A content file that matches no provider — it can only travel as raw bytes,
/// so the sharer picks these one by one.
#[derive(Serialize, Clone)]
pub struct UnresolvedFile {
    /// Game-dir relative path, e.g. `mods/my-private.jar`.
    pub path: String,
    pub size: u64,
}

/// What the share dialog shows before anything is uploaded.
#[derive(Serialize)]
pub struct SharePreview {
    pub modrinth: usize,
    pub curseforge: usize,
    /// Game-dir paths of content files that resolve to no provider.
    pub unresolved: Vec<UnresolvedFile>,
    /// Combined size of those files — the cost of including them.
    pub unresolved_bytes: u64,
}

#[derive(Deserialize, Serialize)]
pub struct ShareResult {
    pub code: String,
    pub url: String,
    /// Unix ms when the code stops working.
    pub expires: i64,
    /// Which revision of this instance the code now points at. Only meaningful
    /// for account-owned shares; anonymous codes are always revision 1.
    #[serde(default = "one")]
    pub revision: u32,
    /// True when this replaced an earlier upload of the same instance — i.e.
    /// friends who already have it were told there is an update.
    #[serde(default)]
    pub pushed: bool,
}

fn one() -> u32 {
    1
}

/// Outcome of redeeming a code. The instance always exists; the counters tell
/// the UI what still needs the user's attention.
#[derive(Serialize)]
pub struct ShareImportResult {
    pub instance: Instance,
    pub installed: usize,
    /// Names of projects that could not be downloaded.
    pub failed: Vec<String>,
    /// CurseForge items skipped because this install has no CurseForge API key.
    pub needs_curseforge: usize,
}

fn folder_for_kind(kind: &str) -> &'static str {
    match kind {
        "resourcepack" => "resourcepacks",
        "shader" => "shaderpacks",
        "datapack" => "datapacks",
        _ => "mods",
    }
}

fn loader_str(loader: &Loader) -> Option<String> {
    match loader {
        Loader::Vanilla => None,
        Loader::Fabric(_) => Some("fabric".into()),
        Loader::Quilt(_) => Some("quilt".into()),
        Loader::Forge(_) => Some("forge".into()),
        Loader::NeoForge(_) => Some("neoforge".into()),
    }
}

/// Pulls hand-dropped jars into the content index so they travel as project ids
/// instead of raw bytes. Best-effort: failures just mean more overrides.
async fn link_local_files(id: &str) {
    let _ = modrinth::match_local_mods(id.to_string()).await;
    if curseforge::cf_enabled() {
        let _ = curseforge::curseforge_match_local(id.to_string()).await;
    }
}

/// Content files that aren't in the index, with their combined size.
fn scan_unresolved(id: &str, items: &[InstalledItem]) -> (Vec<UnresolvedFile>, u64) {
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

/// Packs the instance and uploads it, returning the code to hand out.
///
/// `include` lists the unresolved files (game-dir paths, as `share_preview`
/// reports them) that should travel as raw bytes. Everything else provider-less
/// is left behind — that is what keeps a share small, and it is the sharer's
/// call file by file, since these are the ones nobody can re-download.
#[tauri::command]
pub async fn share_instance(
    app: AppHandle,
    id: String,
    include: Vec<String>,
) -> Result<ShareResult, String> {
    emit_progress(&app, "scanning", 0, 0);
    link_local_files(&id).await;

    let instance: Instance =
        store::read_json(&paths::instance_config_file(&id))?.ok_or("instance not found")?;
    let items = modrinth::read_content_index(&id).items;
    let (unresolved, _) = scan_unresolved(&id, &items);
    // Only paths that really are unresolved can be picked; anything else in the
    // list would just be ignored by the packer anyway.
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
        // Records what actually shipped, so the receiver's warning lists the
        // files it is really about to install and nothing else.
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

    // Signed in, the pack goes straight to storage — that is the only path that
    // can carry more than the proxy in front of the site allows.
    if let Some(token) = crate::commands::spectra::stored_token() {
        let result = upload_to_storage(&app, &tmp, &instance, &id, items.len(), &token).await;
        let _ = std::fs::remove_file(&tmp);
        emit_progress(&app, "done", 1, 1);
        return result;
    }

    // Signed out: the older route, through the server, with the smaller cap.
    let bytes = std::fs::read(&tmp).map_err(|e| format!("read pack: {e}"))?;
    let _ = std::fs::remove_file(&tmp);
    let size = bytes.len() as u64;
    emit_progress(&app, "uploading", 0, size);

    let resp = reqwest::Client::new()
        .post(SHARE_API)
        .query(&[
            ("name", instance.name.as_str()),
            ("mc", instance.mc_version.as_str()),
            ("loader", loader_str(&instance.loader).as_deref().unwrap_or("vanilla")),
            ("mods", &items.len().to_string()),
        ])
        .header("content-type", "application/zip")
        .header("x-spectra-key", INGEST_KEY)
        .header("origin", crate::commands::spectra::ORIGIN)
        .body(bytes)
        .send()
        .await
        .map_err(|e| format!("upload failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let detail = resp.text().await.unwrap_or_default();
        return Err(extract_message(&detail).unwrap_or_else(|| format!("upload failed ({status})")));
    }
    let result = resp.json().await.map_err(|e| format!("bad server reply: {e}"));
    emit_progress(&app, "done", 1, 1);
    result
}

/// Progress for one phase of sharing, as the UI reads it.
///
/// `total` of 0 means "no idea yet" — the bar shows movement without a
/// percentage rather than pretending to know.
fn emit_progress(app: &AppHandle, stage: &str, current: u64, total: u64) {
    let _ = app.emit(
        "share://progress",
        serde_json::json!({ "stage": stage, "current": current, "total": total }),
    );
}

/// Bytes that will end up in `overrides/`, so packing can show a percentage.
/// Metadata only — no file is opened twice.
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

/// Uploads the finished pack straight to R2 and tells the site it landed.
///
/// Three requests: ask for a signed URL, PUT the file, confirm. The bytes never
/// touch the Spectra server — it is behind a proxy that rejects bodies over
/// 100 MB, and a gigabyte of pack has no business being buffered there anyway.
/// The file is streamed off disk, so the launcher does not hold it in memory
/// either.
async fn upload_to_storage(
    app: &AppHandle,
    path: &std::path::Path,
    instance: &Instance,
    id: &str,
    mods: usize,
    token: &str,
) -> Result<ShareResult, String> {
    let size = std::fs::metadata(path).map_err(|e| format!("stat pack: {e}"))?.len();
    let client = reqwest::Client::new();

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
            .header("x-spectra-key", INGEST_KEY)
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

    // Stream the file, counting bytes as they go out so the bar means something
    // on a slow connection.
    let file = tokio::fs::File::open(path).await.map_err(|e| format!("open pack: {e}"))?;
    let handle = app.clone();
    let mut sent = 0u64;
    let mut last_emit = 0u64;
    let stream = tokio_util::io::ReaderStream::new(file).map_ok(move |chunk| {
        sent += chunk.len() as u64;
        // Every 2 MB: often enough to look live, rare enough not to flood the UI.
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


/// Content files the manifest already describes (so they travel as project ids,
/// not bytes) plus the unresolved ones the sharer left unticked.
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

/// Writes the share zip: manifest + everything the manifest doesn't already
/// describe. Content files already recorded as projects, and the unresolved
/// ones the sharer left unticked, are marked as handled so they never reach
/// `overrides/`.
fn write_pack(
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

    // The icon is not an override — it belongs to the instance, not the game dir.
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
            true, // keep `.disabled` files — they're part of the instance
            opts,
            on_file,
        )?;
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}

/// Pulls the human-readable part out of an h3 error body.
fn extract_message(body: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    v.get("statusMessage")
        .or_else(|| v.get("message"))
        .and_then(|m| m.as_str())
        .map(|s| s.to_string())
}

/// Redeems a share code into a brand-new instance: creates it, unpacks the
/// overrides, then downloads every recorded project from **its own** provider.
#[tauri::command]
pub async fn import_share(app: AppHandle, code: String) -> Result<ShareImportResult, String> {
    // CurseForge profile codes look nothing like ours (8 mixed-case chars) and are
    // resolved by an account-gated service only their own app can talk to. Say so
    // instead of mangling the input into a lookup that always fails.
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
    // The icon first — it is the one thing that makes the new instance look
    // like the one that was shared.
    if let Ok(mut entry) = archive.by_name(ICON) {
        let mut buf = Vec::new();
        if entry.read_to_end(&mut buf).is_ok()
            && std::fs::write(paths::instance_icon_file(&instance.id), &buf).is_ok()
        {
            instance.icon = Some(ICON.to_string());
            let _ = store::write_json(&paths::instance_config_file(&instance.id), &instance);
        }
    }

    // Overrides next, so configs are in place before the mods land.
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

    // ponytail: sequential installs — reuses the existing per-provider commands
    // verbatim (dependency resolution, blocked-mod handling, content index all
    // come for free). Parallelise if 150-mod packs start feeling slow.
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

        // Reuse the modpack progress channel so the activity indicator just works.
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

    // Remember the origin so the author's next push updates this instance
    // instead of arriving as a second copy of the same pack.
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

/// Trims user input ("spectra.../s/ab-cd12", " abcd12 ") down to a code.
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

/// Which revision the code currently points at.
async fn revision_of(code: &str) -> Result<u32, String> {
    let resp = reqwest::Client::new()
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

/// What a pull of a newer revision changed locally.
#[derive(Serialize)]
pub struct ShareSyncResult {
    pub revision: u32,
    pub installed: usize,
    pub removed: usize,
    pub failed: Vec<String>,
    pub needs_curseforge: usize,
}

/// Metadata the server keeps next to a code (`?meta=1`).
#[derive(Deserialize)]
struct ShareMeta {
    #[serde(default = "one")]
    revision: u32,
}

/// Downloads a code and returns its manifest plus the raw archive.
///
/// Two hops for packs in storage: the site is asked for a signed address (with
/// our bearer token, which is also how it records that this account now has
/// this revision), then the bytes come from storage **without** that header —
/// S3 refuses a request carrying two different signatures.
async fn fetch_pack(code: &str) -> Result<(ShareManifest, Vec<u8>), String> {
    let client = reqwest::Client::new();
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
        // A code from before packs moved to storage still answers with the zip.
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

/// What a sync has to change, worked out before anything is touched on disk.
struct SyncPlan<'a> {
    /// Missing entirely, or installed at a different version.
    install: Vec<&'a InstalledItem>,
    /// Dropped by the author — and originally came from them.
    remove: Vec<&'a InstalledItem>,
}

/// The three-way comparison at the heart of an update: what the author ships
/// now, what is on disk, and what this instance last took from the share.
///
/// Anything the player installed themselves appears in none of the share lists,
/// so it is never removed — that is the whole reason the previous item ids are
/// stored alongside the instance.
fn plan_sync<'a>(
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

/// Applies a newer revision of a shared pack **on top of an instance that is
/// already installed**, rather than importing it as a second copy.
///
/// Three-way in spirit: the author's new list, what is installed now, and the
/// list this instance last took from the share. Anything the player installed
/// themselves is not in that last list, so it survives; anything the author
/// dropped is, so it goes.
#[tauri::command]
pub async fn sync_share(app: AppHandle, id: String, code: String) -> Result<ShareSyncResult, String> {
    let code = normalize_code(&code)?;

    let mut instance: Instance =
        store::read_json(&paths::instance_config_file(&id))?.ok_or("instance not found")?;

    let revision = revision_of(&code).await?;

    let (manifest, bytes) = fetch_pack(&code).await?;

    let installed_now = modrinth::read_content_index(&id).items;
    let previously_from_share: HashSet<String> = instance
        .share_origin
        .as_ref()
        .map(|o| o.item_ids.iter().cloned().collect())
        .unwrap_or_default();

    let SyncPlan { install: todo, remove } =
        plan_sync(&manifest.items, &installed_now, &previously_from_share);

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

    // A re-iconed pack should look right after the update as well.
    if let Ok(mut entry) = archive.by_name(ICON) {
        let mut buf = Vec::new();
        if entry.read_to_end(&mut buf).is_ok()
            && std::fs::write(paths::instance_icon_file(&id), &buf).is_ok()
        {
            instance.icon = Some(ICON.to_string());
        }
    }

    // Overrides, minus the files that are the player's own business. `KEEP_ON_SYNC`
    // still matters for packs made before shares were limited to content folders.
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

// ===== deep link (`spectra://share/<code>`) =====

/// Extracts the code from a `spectra://share/ABC123` URL.
pub fn code_from_url(url: &str) -> Option<String> {
    let rest = url.strip_prefix("spectra://")?.trim_start_matches('/');
    let rest = rest.strip_prefix("share/").or_else(|| rest.strip_prefix("share"))?;
    let code: String = rest.trim_matches('/').chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    (code.len() == 6).then(|| code.to_uppercase())
}

/// Hands the frontend a code that arrived via deep link before the UI was ready
/// (cold start). Returns `None` once consumed.
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
        // The author shipped A and B last time; A is now gone and B moved on a
        // version. The player added C themselves.
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
        // Windows hands the URL over with the host slot filled by the path.
        assert_eq!(code_from_url("spectra://share?x=1"), None);
        assert_eq!(code_from_url("spectra://share/AB"), None);
        assert_eq!(code_from_url("https://spectra.makoto.com.pl/s/ABC123"), None);
    }
}
