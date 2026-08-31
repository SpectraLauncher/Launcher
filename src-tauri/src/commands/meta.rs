use serde::{Deserialize, Serialize};

use crate::paths;

#[derive(Debug, Serialize)]
pub struct MinecraftVersion {
    pub id: String,
    pub kind: String,
    pub release_time: String,
}

#[derive(Debug, Serialize)]
pub struct LoaderVersion {
    pub version: String,
    pub stable: bool,
}

const MANIFEST_URL: &str = "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json";

const META_TTL: std::time::Duration = std::time::Duration::from_secs(600);

async fn get_bytes(url: &str) -> Result<Vec<u8>, String> {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    use std::time::Instant;

    static CACHE: OnceLock<Mutex<HashMap<String, (Instant, Vec<u8>)>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Mutex::default);

    if let Ok(map) = cache.lock() {
        if let Some((at, body)) = map.get(url) {
            if at.elapsed() < META_TTL {
                return Ok(body.clone());
            }
        }
    }

    let body = crate::http()
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .bytes()
        .await
        .map_err(|e| e.to_string())?
        .to_vec();

    if let Ok(mut map) = cache.lock() {
        map.insert(url.to_string(), (Instant::now(), body.clone()));
    }
    Ok(body)
}

#[derive(Deserialize)]
struct Manifest {
    versions: Vec<ManifestVersion>,
}

#[derive(Deserialize)]
struct ManifestVersion {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(rename = "releaseTime")]
    release_time: String,
}

#[tauri::command]
pub async fn get_minecraft_versions(
    include_snapshots: bool,
) -> Result<Vec<MinecraftVersion>, String> {
    let cache = paths::cache_dir().join("version_manifest_v2.json");

    let bytes = match get_bytes(MANIFEST_URL).await {
        Ok(b) => {
            let _ = std::fs::create_dir_all(paths::cache_dir());
            let _ = std::fs::write(&cache, &b);
            b
        }
        Err(_) => std::fs::read(&cache)
            .map_err(|_| "could not fetch version manifest (offline, no cache)".to_string())?,
    };

    let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    Ok(manifest
        .versions
        .into_iter()
        .filter(|v| include_snapshots || v.kind == "release")
        .map(|v| MinecraftVersion {
            id: v.id,
            kind: v.kind,
            release_time: v.release_time,
        })
        .collect())
}

#[tauri::command]
pub async fn get_loader_versions(
    loader: String,
    mc_version: String,
) -> Result<Vec<LoaderVersion>, String> {
    match loader.as_str() {
        "vanilla" => Ok(Vec::new()),
        "fabric" => fetch_fabric(&mc_version).await,
        "quilt" => fetch_quilt(&mc_version).await,
        "neoforge" => fetch_neoforge(&mc_version).await,
        "forge" => fetch_forge(&mc_version).await,
        other => Err(format!("unknown loader: {other}")),
    }
}

#[derive(Deserialize)]
struct FabricEntry {
    loader: FabricLoader,
}
#[derive(Deserialize)]
struct FabricLoader {
    version: String,
    stable: bool,
}

async fn fetch_fabric(mc: &str) -> Result<Vec<LoaderVersion>, String> {
    let url = format!("https://meta.fabricmc.net/v2/versions/loader/{mc}");
    let entries: Vec<FabricEntry> =
        serde_json::from_slice(&get_bytes(&url).await?).map_err(|e| e.to_string())?;
    Ok(entries
        .into_iter()
        .map(|e| LoaderVersion {
            version: e.loader.version,
            stable: e.loader.stable,
        })
        .collect())
}

#[derive(Deserialize)]
struct QuiltEntry {
    loader: QuiltLoader,
}
#[derive(Deserialize)]
struct QuiltLoader {
    version: String,
}

async fn fetch_quilt(mc: &str) -> Result<Vec<LoaderVersion>, String> {
    let url = format!("https://meta.quiltmc.org/v3/versions/loader/{mc}");
    let entries: Vec<QuiltEntry> =
        serde_json::from_slice(&get_bytes(&url).await?).map_err(|e| e.to_string())?;
    Ok(entries
        .into_iter()
        .map(|e| {
            let stable = !e.loader.version.contains("beta");
            LoaderVersion {
                version: e.loader.version,
                stable,
            }
        })
        .collect())
}

#[derive(Deserialize)]
struct NeoForgeResponse {
    versions: Vec<String>,
}

fn neoforge_prefix(mc: &str) -> Result<String, String> {
    let parts: Vec<&str> = mc.split('.').collect();
    if parts.len() < 2 || parts[0] != "1" {
        return Err(format!("unsupported MC version for NeoForge: {mc}"));
    }
    let minor = parts[1];
    let patch = parts.get(2).copied().unwrap_or("0");
    Ok(format!("{minor}.{patch}."))
}

async fn fetch_neoforge(mc: &str) -> Result<Vec<LoaderVersion>, String> {
    let url = "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge";
    let resp: NeoForgeResponse =
        serde_json::from_slice(&get_bytes(url).await?).map_err(|e| e.to_string())?;

    let prefix = neoforge_prefix(mc)?;
    let mut list: Vec<LoaderVersion> = resp
        .versions
        .into_iter()
        .filter(|v| v.starts_with(&prefix))
        .map(|v| {
            let stable = !v.contains("beta");
            LoaderVersion { version: v, stable }
        })
        .collect();
    list.reverse();
    Ok(list)
}

async fn fetch_forge(mc: &str) -> Result<Vec<LoaderVersion>, String> {
    let url = "https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml";
    let xml = String::from_utf8_lossy(&get_bytes(url).await?).into_owned();

    let prefix = format!("{mc}-");
    let mut list: Vec<LoaderVersion> = xml
        .split("<version>")
        .skip(1)
        .filter_map(|seg| seg.split("</version>").next())
        .map(|s| s.trim().to_string())
        .filter(|v| v.starts_with(&prefix))
        .map(|version| LoaderVersion {
            version,
            stable: true,
        })
        .collect();
    list.reverse();
    Ok(list)
}
