//! Version-list metadata for the create-instance pickers.
//!
//! We do NOT download game files here — Lyceris' `install()` does that. These
//! commands only return the *lists* of versions the UI lets the user pick from:
//! Minecraft versions (Mojang) and mod-loader versions (Fabric/Quilt/NeoForge/Forge).

use serde::{Deserialize, Serialize};

use crate::paths;

#[derive(Debug, Serialize)]
pub struct MinecraftVersion {
    pub id: String,
    /// "release", "snapshot", "old_beta", "old_alpha".
    pub kind: String,
    pub release_time: String,
}

#[derive(Debug, Serialize)]
pub struct LoaderVersion {
    pub version: String,
    pub stable: bool,
}

const MANIFEST_URL: &str = "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json";

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("SpectraLauncher")
        .build()
        .map_err(|e| e.to_string())
}

// === Minecraft versions ===

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

/// Mojang's full version list, newest first. Cached to `cache/` so the picker
/// still works offline after the first online fetch.
#[tauri::command]
pub async fn get_minecraft_versions(
    include_snapshots: bool,
) -> Result<Vec<MinecraftVersion>, String> {
    let cache = paths::cache_dir().join("version_manifest_v2.json");

    let bytes = match client()?.get(MANIFEST_URL).send().await {
        Ok(resp) if resp.status().is_success() => {
            let b = resp.bytes().await.map_err(|e| e.to_string())?;
            let _ = std::fs::create_dir_all(paths::cache_dir());
            let _ = std::fs::write(&cache, &b);
            b.to_vec()
        }
        _ => std::fs::read(&cache)
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

// === Loader versions ===

/// Loader versions for a given Minecraft version, newest first. Vanilla returns
/// an empty list.
#[tauri::command]
pub async fn get_loader_versions(
    loader: String,
    mc_version: String,
) -> Result<Vec<LoaderVersion>, String> {
    let client = client()?;
    match loader.as_str() {
        "vanilla" => Ok(Vec::new()),
        "fabric" => fetch_fabric(&client, &mc_version).await,
        "quilt" => fetch_quilt(&client, &mc_version).await,
        "neoforge" => fetch_neoforge(&client, &mc_version).await,
        "forge" => fetch_forge(&client, &mc_version).await,
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

async fn fetch_fabric(client: &reqwest::Client, mc: &str) -> Result<Vec<LoaderVersion>, String> {
    let url = format!("https://meta.fabricmc.net/v2/versions/loader/{mc}");
    let entries: Vec<FabricEntry> = client
        .get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    // Already newest-first.
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

async fn fetch_quilt(client: &reqwest::Client, mc: &str) -> Result<Vec<LoaderVersion>, String> {
    let url = format!("https://meta.quiltmc.org/v3/versions/loader/{mc}");
    let entries: Vec<QuiltEntry> = client
        .get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
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

/// NeoForge versions encode the MC version in the first two segments:
/// MC `1.21.4` → `21.4.x`, MC `1.21` → `21.0.x`.
fn neoforge_prefix(mc: &str) -> Result<String, String> {
    let parts: Vec<&str> = mc.split('.').collect();
    if parts.len() < 2 || parts[0] != "1" {
        return Err(format!("unsupported MC version for NeoForge: {mc}"));
    }
    let minor = parts[1];
    let patch = parts.get(2).copied().unwrap_or("0");
    Ok(format!("{minor}.{patch}."))
}

async fn fetch_neoforge(client: &reqwest::Client, mc: &str) -> Result<Vec<LoaderVersion>, String> {
    let url = "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge";
    let resp: NeoForgeResponse = client
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

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
    list.reverse(); // API is oldest-first
    Ok(list)
}

async fn fetch_forge(client: &reqwest::Client, mc: &str) -> Result<Vec<LoaderVersion>, String> {
    let url = "https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml";
    let xml = client
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?;

    // Forge maven versions look like "1.21.4-54.1.0"; we keep the whole string
    // for display and strip the "{mc}-" prefix at launch (Lyceris re-adds it).
    // Pull them out of the XML without an XML crate.
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
    list.reverse(); // XML is oldest-first
    Ok(list)
}
