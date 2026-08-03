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

use std::collections::HashSet;
use std::io::{Read, Write};

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

/// Personal or regenerable folders that are never worth sharing. (`versions`,
/// `libraries`, `assets`, … are already dropped by the exporter itself.)
const EXCLUDE: &[&str] = &["saves", "screenshots", "crash-reports", "backups"];

/// Folders scanned for content that could be recorded as a project instead of
/// being shipped byte-for-byte.
const CONTENT_DIRS: &[&str] = &["mods", "resourcepacks", "shaderpacks", "datapacks"];

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

/// What the share dialog shows before anything is uploaded.
#[derive(Serialize)]
pub struct SharePreview {
    pub modrinth: usize,
    pub curseforge: usize,
    /// Game-dir paths of content files that resolve to no provider.
    pub unresolved: Vec<String>,
    /// Combined size of those files — the cost of including them.
    pub unresolved_bytes: u64,
}

#[derive(Deserialize, Serialize)]
pub struct ShareResult {
    pub code: String,
    pub url: String,
    /// Unix ms when the code stops working.
    pub expires: i64,
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
fn scan_unresolved(id: &str, items: &[InstalledItem]) -> (Vec<String>, u64) {
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
            bytes += e.metadata().map(|m| m.len()).unwrap_or(0);
            out.push(format!("{sub}/{raw}"));
        }
    }
    out.sort();
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
/// `include_unresolved` ships provider-less files (private/local jars) as raw
/// bytes — off by default, since that's what makes a share big.
#[tauri::command]
pub async fn share_instance(id: String, include_unresolved: bool) -> Result<ShareResult, String> {
    link_local_files(&id).await;

    let instance: Instance =
        store::read_json(&paths::instance_config_file(&id))?.ok_or("instance not found")?;
    let items = modrinth::read_content_index(&id).items;
    let (unresolved, _) = scan_unresolved(&id, &items);

    let tmp = std::env::temp_dir().join(format!("spectra-share-{id}.zip"));
    let manifest = ShareManifest {
        format: FORMAT.into(),
        version: 1,
        name: instance.name.clone(),
        mc_version: instance.mc_version.clone(),
        loader: instance.loader.clone(),
        items: items.clone(),
        unresolved: unresolved.clone(),
    };
    write_pack(&tmp, &manifest, &id, include_unresolved)?;

    let bytes = std::fs::read(&tmp).map_err(|e| format!("read pack: {e}"))?;
    let _ = std::fs::remove_file(&tmp);

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
        .body(bytes)
        .send()
        .await
        .map_err(|e| format!("upload failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let detail = resp.text().await.unwrap_or_default();
        return Err(extract_message(&detail).unwrap_or_else(|| format!("upload failed ({status})")));
    }
    resp.json().await.map_err(|e| format!("bad server reply: {e}"))
}

/// Writes the share zip: manifest + everything the manifest doesn't already
/// describe. Content files already recorded as projects (and, unless asked for,
/// the unresolved ones) are marked as handled so they never hit `overrides/`.
fn write_pack(
    dest: &std::path::Path,
    manifest: &ShareManifest,
    id: &str,
    include_unresolved: bool,
) -> Result<(), String> {
    let mut handled: HashSet<String> = HashSet::new();
    for item in &manifest.items {
        let rel = format!("{}/{}", folder_for_kind(&item.kind), item.filename);
        handled.insert(format!("{rel}.disabled"));
        handled.insert(rel);
    }
    if !include_unresolved {
        handled.extend(manifest.unresolved.iter().cloned());
    }

    let file = std::fs::File::create(dest).map_err(|e| format!("create pack: {e}"))?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let json = serde_json::to_vec_pretty(manifest).map_err(|e| e.to_string())?;
    zip.start_file(MANIFEST, opts).map_err(|e| e.to_string())?;
    zip.write_all(&json).map_err(|e| e.to_string())?;

    let filter = import::ExportFilter::new(
        Vec::new(),
        EXCLUDE.iter().map(|s| s.to_string()).collect(),
    );
    modrinth::add_overrides(
        &mut zip,
        &paths::instance_game_dir(id),
        "",
        &filter,
        &handled,
        true, // keep `.disabled` files — they're part of the instance
        opts,
    )?;
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

    let code: String = code
        .trim()
        .to_uppercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    let code = if code.len() > 6 { code[code.len() - 6..].to_string() } else { code };
    if code.len() != 6 {
        return Err("a share code is 6 characters".into());
    }

    let resp = reqwest::Client::new()
        .get(format!("{SHARE_API}/{code}"))
        .send()
        .await
        .map_err(|e| format!("download failed: {e}"))?;
    if !resp.status().is_success() {
        return Err("this code does not exist or has expired".into());
    }
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?.to_vec();

    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|e| format!("open pack: {e}"))?;
    let manifest: ShareManifest = {
        let mut f = archive.by_name(MANIFEST).map_err(|_| "not a Spectra share pack".to_string())?;
        let mut s = String::new();
        f.read_to_string(&mut s).map_err(|e| e.to_string())?;
        serde_json::from_str(&s).map_err(|e| format!("parse manifest: {e}"))?
    };
    if manifest.format != FORMAT {
        return Err("not a Spectra share pack".into());
    }

    let instance = instances::create_instance(
        manifest.name.clone(),
        manifest.mc_version.clone(),
        manifest.loader.clone(),
        None,
        None,
    )?;

    // Overrides first, so configs are in place before the mods land.
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

    Ok(ShareImportResult { instance, installed, failed, needs_curseforge })
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
    use super::code_from_url;

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
