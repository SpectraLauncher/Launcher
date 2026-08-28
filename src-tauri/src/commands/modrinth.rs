use std::collections::HashMap;
use std::io::{Cursor, Read, Write};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::commands::instances;
use crate::models::{Instance, Loader};
use crate::{paths, store};

const API: &str = "https://api.modrinth.com/v2";
const USER_AGENT: &str = concat!("MakotoPD/Spectra-Launcher/", env!("CARGO_PKG_VERSION"), " (spectra launcher)");

fn http() -> reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .user_agent(USER_AGENT)
                .build()
                .expect("build reqwest client")
        })
        .clone()
}

fn client() -> Result<reqwest::Client, String> {
    Ok(http())
}

fn rate_gate() -> &'static tokio::sync::Semaphore {
    static SEM: std::sync::OnceLock<tokio::sync::Semaphore> = std::sync::OnceLock::new();
    SEM.get_or_init(|| tokio::sync::Semaphore::new(6))
}

async fn send(req: reqwest::RequestBuilder) -> Result<reqwest::Response, String> {
    let _permit = rate_gate().acquire().await.map_err(|e| e.to_string())?;
    let mut attempt: u32 = 0;
    loop {
        let attempt_req = req.try_clone().ok_or("request is not retryable")?;
        let resp = attempt_req.send().await.map_err(|e| e.to_string())?;
        if resp.status().as_u16() != 429 || attempt >= 5 {
            return Ok(resp);
        }
        let retry_after = resp
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.trim().parse::<u64>().ok());
        let wait = retry_after.unwrap_or_else(|| (1u64 << attempt).min(15)).clamp(1, 15);
        tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
        attempt += 1;
    }
}

#[derive(Deserialize)]
pub struct SearchParams {
    query: String,
    project_type: String,
    #[serde(default)]
    loaders: Vec<String>,
    #[serde(default)]
    game_versions: Vec<String>,
    #[serde(default)]
    categories: Vec<String>,
    #[serde(default = "default_index")]
    index: String,
    #[serde(default)]
    offset: u32,
    #[serde(default = "default_limit")]
    limit: u32,
}

fn default_index() -> String {
    "relevance".to_string()
}
fn default_limit() -> u32 {
    20
}

#[derive(Serialize, Deserialize)]
pub struct SearchHit {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub project_type: String,
    pub downloads: u64,
    pub follows: u64,
    pub icon_url: Option<String>,
    pub categories: Vec<String>,
    pub versions: Vec<String>,
    pub client_side: Option<String>,
    pub server_side: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct SearchResponse {
    pub hits: Vec<SearchHit>,
    pub total_hits: u64,
    pub offset: u64,
    pub limit: u64,
}

#[tauri::command]
pub async fn modrinth_search(params: SearchParams) -> Result<SearchResponse, String> {
    let mut facets: Vec<Vec<String>> = vec![vec![format!("project_type:{}", params.project_type)]];

    if !params.loaders.is_empty() {
        facets.push(params.loaders.iter().map(|l| format!("categories:{l}")).collect());
    }
    for c in &params.categories {
        facets.push(vec![format!("categories:{c}")]);
    }
    if !params.game_versions.is_empty() {
        facets.push(params.game_versions.iter().map(|v| format!("versions:{v}")).collect());
    }

    let facets_json = serde_json::to_string(&facets).map_err(|e| e.to_string())?;

    let resp = send(http().get(format!("{API}/search")).query(&[
        ("query", params.query.as_str()),
        ("facets", facets_json.as_str()),
        ("index", params.index.as_str()),
        ("offset", &params.offset.to_string()),
        ("limit", &params.limit.to_string()),
    ]))
    .await?;

    if !resp.status().is_success() {
        return Err(format!("Modrinth search failed: {}", resp.status()));
    }
    resp.json::<SearchResponse>().await.map_err(|e| e.to_string())
}

#[derive(Serialize, Deserialize, Clone)]
pub struct VersionFileHashes {
    pub sha1: Option<String>,
    pub sha512: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct VersionFile {
    pub url: String,
    pub filename: String,
    pub primary: bool,
    pub size: u64,
    pub hashes: VersionFileHashes,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Dependency {
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub dependency_type: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Version {
    pub id: String,
    #[serde(default)]
    pub project_id: String,
    pub name: String,
    pub version_number: String,
    pub version_type: String,
    pub loaders: Vec<String>,
    pub game_versions: Vec<String>,
    pub downloads: u64,
    pub date_published: String,
    #[serde(default)]
    pub changelog: Option<String>,
    pub files: Vec<VersionFile>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
}

#[tauri::command]
pub async fn modrinth_versions(
    project_id: String,
    loaders: Option<Vec<String>>,
    game_versions: Option<Vec<String>>,
) -> Result<Vec<Version>, String> {
    let mut req = http().get(format!("{API}/project/{project_id}/version"));
    if let Some(l) = loaders.filter(|l| !l.is_empty()) {
        req = req.query(&[("loaders", serde_json::to_string(&l).map_err(|e| e.to_string())?)]);
    }
    if let Some(g) = game_versions.filter(|g| !g.is_empty()) {
        req = req.query(&[("game_versions", serde_json::to_string(&g).map_err(|e| e.to_string())?)]);
    }

    let resp = send(req).await?;
    if !resp.status().is_success() {
        return Err(format!("Modrinth versions failed: {}", resp.status()));
    }
    resp.json::<Vec<Version>>().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn modrinth_match_file(instance_id: String, filename: String) -> Result<bool, String> {
    use sha1::{Digest, Sha1};

    let dir = paths::instance_game_dir(&instance_id).join("mods");
    let path = if dir.join(&filename).is_file() {
        dir.join(&filename)
    } else {
        dir.join(format!("{filename}.disabled"))
    };
    let bytes = std::fs::read(&path).map_err(|e| format!("read {filename}: {e}"))?;
    let mut hasher = Sha1::new();
    hasher.update(&bytes);
    let sha1 = format!("{:x}", hasher.finalize());

    let http = http();
    let resp = send(
        http.post(format!("{API}/version_files"))
            .json(&serde_json::json!({ "hashes": [sha1], "algorithm": "sha1" })),
    )
    .await?;
    if !resp.status().is_success() {
        return Ok(false);
    }
    let versions: HashMap<String, Version> = resp.json().await.map_err(|e| e.to_string())?;
    let Some(version) = versions.into_values().next() else { return Ok(false) };

    let project = fetch_project(&http, &version.project_id).await.ok();
    let (kind, _folder) = match &project {
        Some(p) => kind_and_folder(&version, p),
        None => ("mod", "mods"),
    };
    let mut index = read_content_index(&instance_id);
    let item = InstalledItem {
        project_id: version.project_id.clone(),
        version_id: version.id.clone(),
        kind: kind.to_string(),
        name: project.as_ref().map(|p| p.title.clone()).unwrap_or_else(|| filename.clone()),
        filename: filename.clone(),
        version_number: version.version_number.clone(),
        icon_url: project.as_ref().and_then(|p| p.icon_url.clone()),
        game_versions: version.game_versions.clone(),
        loaders: version.loaders.clone(),
        dependency: false,
        dependencies: Vec::new(),
        installed_at: chrono::Utc::now().to_rfc3339(),
        provider: "modrinth".to_string(),
        author: None,
        env: project
            .as_ref()
            .and_then(|p| env_from_sides(p.client_side.as_deref(), p.server_side.as_deref())),
    };
    index.items.retain(|i| i.project_id != item.project_id && i.filename != item.filename);
    index.items.push(item);
    write_content_index(&instance_id, &index)?;
    Ok(true)
}

#[derive(Serialize)]
pub struct ModUpdate {
    project_id: String,
    version_id: String,
    version_number: String,
}

fn hash_modrinth_mods(instance_id: &str, index: &ContentIndex) -> Vec<(String, InstalledItem)> {
    use sha1::{Digest, Sha1};
    let dir = paths::instance_game_dir(instance_id).join("mods");
    let mut out = Vec::new();
    for item in index.items.iter().filter(|i| i.kind == "mod" && i.provider == "modrinth") {
        let path = if dir.join(&item.filename).is_file() {
            dir.join(&item.filename)
        } else {
            dir.join(format!("{}.disabled", item.filename))
        };
        let Ok(bytes) = std::fs::read(&path) else { continue };
        let mut h = Sha1::new();
        h.update(&bytes);
        out.push((format!("{:x}", h.finalize()), item.clone()));
    }
    out
}

async fn bulk_latest_versions(
    hashes: &[String],
    loaders: &Option<Vec<String>>,
    game_versions: &Option<Vec<String>>,
) -> Result<HashMap<String, Version>, String> {
    if hashes.is_empty() {
        return Ok(HashMap::new());
    }
    let mut body = serde_json::json!({ "hashes": hashes, "algorithm": "sha1" });
    if let Some(l) = loaders.as_ref().filter(|l| !l.is_empty()) {
        body["loaders"] = serde_json::json!(l);
    }
    if let Some(g) = game_versions.as_ref().filter(|g| !g.is_empty()) {
        body["game_versions"] = serde_json::json!(g);
    }
    let resp = send(http().post(format!("{API}/version_files/update")).json(&body)).await?;
    if !resp.status().is_success() {
        return Err(format!("update check failed: {}", resp.status()));
    }
    resp.json::<HashMap<String, Version>>().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn check_mod_updates(
    instance_id: String,
    loaders: Option<Vec<String>>,
    game_versions: Option<Vec<String>>,
) -> Result<Vec<ModUpdate>, String> {
    let index = read_content_index(&instance_id);
    let mut out = Vec::new();

    let hashed = hash_modrinth_mods(&instance_id, &index);
    if !hashed.is_empty() {
        let hashes: Vec<String> = hashed.iter().map(|(h, _)| h.clone()).collect();
        if let Ok(map) = bulk_latest_versions(&hashes, &loaders, &game_versions).await {
            for (hash, item) in &hashed {
                if let Some(v) = map.get(hash) {
                    if v.id != item.version_id {
                        out.push(ModUpdate {
                            project_id: item.project_id.clone(),
                            version_id: v.id.clone(),
                            version_number: v.version_number.clone(),
                        });
                    }
                }
            }
        }
    }

    let cf_items: Vec<&InstalledItem> =
        index.items.iter().filter(|i| i.kind == "mod" && i.provider == "curseforge").collect();
    if !cf_items.is_empty() {
        let pids: Vec<String> = cf_items.iter().map(|i| i.project_id.clone()).collect();
        let map = crate::commands::curseforge::bulk_latest_files(&pids, &loaders, &game_versions).await;
        for item in cf_items {
            if let Some((vid, vnum)) = map.get(&item.project_id) {
                if vid != &item.version_id {
                    out.push(ModUpdate {
                        project_id: item.project_id.clone(),
                        version_id: vid.clone(),
                        version_number: vnum.clone(),
                    });
                }
            }
        }
    }
    Ok(out)
}

#[tauri::command]
pub async fn update_all_mods(
    instance_id: String,
    loaders: Option<Vec<String>>,
    game_versions: Option<Vec<String>>,
) -> Result<usize, String> {
    let mut index = read_content_index(&instance_id);
    let mods_dir = paths::instance_game_dir(&instance_id).join("mods");

    let hashed = hash_modrinth_mods(&instance_id, &index);
    if hashed.is_empty() {
        return Ok(0);
    }
    let hashes: Vec<String> = hashed.iter().map(|(h, _)| h.clone()).collect();
    let map = bulk_latest_versions(&hashes, &loaders, &game_versions).await?;

    let mut plan: Vec<(usize, Version)> = Vec::new();
    for (hash, item) in &hashed {
        if let Some(v) = map.get(hash) {
            if v.id != item.version_id {
                if let Some(pos) = index.items.iter().position(|i| i.project_id == item.project_id && i.provider == "modrinth") {
                    plan.push((pos, v.clone()));
                }
            }
        }
    }
    if plan.is_empty() {
        return Ok(0);
    }

    let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(8));
    let mut set = tokio::task::JoinSet::new();
    for (pos, v) in plan {
        let Some(file) = v.files.iter().find(|f| f.primary).or_else(|| v.files.first()).cloned() else { continue };
        let mods_dir = mods_dir.clone();
        let http = http();
        let sem = sem.clone();
        set.spawn(async move {
            let _permit = sem.acquire_owned().await.ok()?;
            let bytes = download_direct(&http, &file.url).await.ok()?;
            std::fs::write(mods_dir.join(safe_name(&file.filename)), &bytes).ok()?;
            Some((pos, v, file.filename))
        });
    }

    let mut updated = 0usize;
    while let Some(res) = set.join_next().await {
        let Ok(Some((pos, v, new_filename))) = res else { continue };
        let item = &mut index.items[pos];
        let old = item.filename.clone();
        if old != new_filename {
            let _ = std::fs::remove_file(mods_dir.join(&old));
            let _ = std::fs::remove_file(mods_dir.join(format!("{old}.disabled")));
        }
        item.version_id = v.id;
        item.version_number = v.version_number;
        item.filename = new_filename;
        item.game_versions = v.game_versions;
        item.loaders = v.loaders;
        item.dependencies = v
            .dependencies
            .iter()
            .filter(|d| d.dependency_type == "required")
            .filter_map(|d| d.project_id.clone())
            .collect();
        updated += 1;
    }

    write_content_index(&instance_id, &index)?;
    Ok(updated)
}

#[derive(Deserialize)]
struct RawCategory {
    name: String,
    project_type: String,
    header: String,
}

#[derive(Serialize)]
pub struct Category {
    pub name: String,
    pub header: String,
}

#[tauri::command]
pub async fn modrinth_categories(project_type: String) -> Result<Vec<Category>, String> {
    let resp = send(http().get(format!("{API}/tag/category"))).await?;
    let all: Vec<RawCategory> = resp.json().await.map_err(|e| e.to_string())?;
    Ok(all
        .into_iter()
        .filter(|c| c.project_type == project_type)
        .map(|c| Category { name: c.name, header: c.header })
        .collect())
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct InstalledItem {
    pub project_id: String,
    pub version_id: String,
    pub kind: String,
    pub name: String,
    pub filename: String,
    pub version_number: String,
    pub icon_url: Option<String>,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub dependency: bool,
    #[serde(default)]
    pub dependencies: Vec<String>,
    pub installed_at: String,
    #[serde(default = "default_provider")]
    pub provider: String,

    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub env: Option<String>,
}

pub fn default_provider() -> String {
    "modrinth".to_string()
}

#[derive(Serialize, Deserialize, Default)]
pub struct ContentIndex {
    pub items: Vec<InstalledItem>,
}

pub fn read_content_index(instance_id: &str) -> ContentIndex {
    store::read_json(&paths::instance_content_index(instance_id))
        .ok()
        .flatten()
        .unwrap_or_default()
}

pub fn write_content_index(instance_id: &str, index: &ContentIndex) -> Result<(), String> {
    store::write_json(&paths::instance_content_index(instance_id), index)
}

pub fn installed_by_other_provider(
    index: &ContentIndex,
    name: &str,
    filename: &str,
    provider: &str,
) -> bool {
    let key = |s: &str| s.trim().to_lowercase();
    let (name, filename) = (key(name), key(filename));
    index.items.iter().any(|i| {
        i.provider != provider
            && ((!name.is_empty() && key(&i.name) == name)
                || (!filename.is_empty() && key(&i.filename) == filename))
    })
}

#[tauri::command]
pub fn get_installed_content(instance_id: String) -> Result<Vec<InstalledItem>, String> {
    Ok(read_content_index(&instance_id).items)
}

#[derive(Serialize)]
pub struct Conflict {
    filename: String,
    name: String,
    kind: String,
    detail: String,
}

fn accepted_loaders(loader: &Loader) -> Vec<&'static str> {
    match loader {
        Loader::Fabric(_) => vec!["fabric"],
        Loader::Quilt(_) => vec!["quilt", "fabric"],
        Loader::Forge(_) => vec!["forge"],
        Loader::NeoForge(_) => vec!["neoforge"],
        Loader::Vanilla => vec![],
    }
}

#[tauri::command]
pub fn check_conflicts(instance_id: String) -> Result<Vec<Conflict>, String> {
    let instance: Instance =
        store::read_json(&paths::instance_config_file(&instance_id))?.ok_or("instance not found")?;
    let mut accepted = accepted_loaders(&instance.loader);
    let loader_label = match &instance.loader {
        Loader::Vanilla => "Vanilla",
        Loader::Fabric(_) => "Fabric",
        Loader::Quilt(_) => "Quilt",
        Loader::Forge(_) => "Forge",
        Loader::NeoForge(_) => "NeoForge",
    };
    let index = read_content_index(&instance_id);
    let mut out = Vec::new();

    let has_fabric_compat = index.items.iter().filter(|i| i.kind == "mod").any(|i| {
        let name = i.name.to_lowercase();
        let file = i.filename.to_lowercase();
        name.contains("sinytra connector") || file.contains("sinytra") ||
        name.contains("forgified fabric api") || file.contains("forgified")
    });

    if has_fabric_compat && (matches!(instance.loader, Loader::Forge(_)) || matches!(instance.loader, Loader::NeoForge(_))) {
        accepted.push("fabric");
    }

    let mut seen: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for item in &index.items {
        *seen.entry(item.project_id.as_str()).or_insert(0) += 1;
    }

    for item in index.items.iter().filter(|i| i.kind == "mod") {
        let loaders: Vec<String> = item.loaders.iter().map(|l| l.to_lowercase()).collect();
        let agnostic = loaders.iter().any(|l| l == "minecraft" || l == "datapack");
        if !loaders.is_empty() && !agnostic && !loaders.iter().any(|l| accepted.contains(&l.as_str())) {
            out.push(Conflict {
                filename: item.filename.clone(),
                name: item.name.clone(),
                kind: "loader".into(),
                detail: format!("{} ≠ {loader_label}", item.loaders.join("/")),
            });
        }
        if seen.get(item.project_id.as_str()).copied().unwrap_or(0) > 1 {
            out.push(Conflict {
                filename: item.filename.clone(),
                name: item.name.clone(),
                kind: "duplicate".into(),
                detail: String::new(),
            });
        }
    }
    Ok(out)
}

pub fn installed_items(instance_id: &str) -> Vec<InstalledItem> {
    read_content_index(instance_id).items
}

pub fn remove_index_entry(instance_id: &str, filename: &str) {
    let mut index = read_content_index(instance_id);
    let before = index.items.len();
    index.items.retain(|i| i.filename != filename);
    if index.items.len() != before {
        let _ = write_content_index(instance_id, &index);
    }
}

#[derive(Serialize)]
pub struct RemovableDep {
    pub project_id: String,
    pub name: String,
    pub filename: String,
    pub icon_url: Option<String>,
    pub kind: String,
}

#[tauri::command]
pub fn get_removable_dependencies(
    instance_id: String,
    filename: String,
) -> Result<Vec<RemovableDep>, String> {
    use std::collections::{HashMap, HashSet, VecDeque};

    let index = read_content_index(&instance_id);
    let items = &index.items;

    let Some(root) = items.iter().find(|i| i.filename == filename) else {
        return Ok(Vec::new());
    };

    let by_pid: HashMap<&str, &InstalledItem> =
        items.iter().map(|i| (i.project_id.as_str(), i)).collect();

    let mut removing: HashSet<String> = HashSet::new();
    removing.insert(root.project_id.clone());

    let mut result: Vec<RemovableDep> = Vec::new();
    let mut queue: VecDeque<String> = root.dependencies.iter().cloned().collect();

    while let Some(pid) = queue.pop_front() {
        if removing.contains(&pid) {
            continue;
        }
        let Some(dep) = by_pid.get(pid.as_str()) else { continue };
        if !dep.dependency {
            continue;
        }
        let still_needed = items.iter().any(|i| {
            !removing.contains(&i.project_id)
                && i.project_id != pid
                && i.dependencies.iter().any(|d| d == &pid)
        });
        if still_needed {
            continue;
        }
        removing.insert(pid.clone());
        result.push(RemovableDep {
            project_id: dep.project_id.clone(),
            name: dep.name.clone(),
            filename: dep.filename.clone(),
            icon_url: dep.icon_url.clone(),
            kind: dep.kind.clone(),
        });
        for d in &dep.dependencies {
            queue.push_back(d.clone());
        }
    }

    Ok(result)
}

#[derive(Serialize, Deserialize)]
pub struct GalleryItem {
    pub url: String,
    #[serde(default)]
    pub raw_url: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub featured: bool,
}

#[derive(Serialize, Deserialize)]
pub struct ProjectFull {
    pub id: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub body: String,
    pub icon_url: Option<String>,
    #[serde(default)]
    pub gallery: Vec<GalleryItem>,
}

#[tauri::command]
pub async fn modrinth_project(id: String) -> Result<ProjectFull, String> {
    let resp = send(http().get(format!("{API}/project/{id}"))).await?;
    if !resp.status().is_success() {
        return Err(format!("Modrinth project failed: {}", resp.status()));
    }
    resp.json::<ProjectFull>().await.map_err(|e| e.to_string())
}

#[derive(Deserialize)]
struct ProjectInfo {
    #[serde(default)]
    id: String,
    title: String,
    icon_url: Option<String>,
    project_type: String,
    #[serde(default)]
    categories: Vec<String>,
    #[serde(default)]
    client_side: Option<String>,
    #[serde(default)]
    server_side: Option<String>,
}

pub(crate) fn env_from_sides(client: Option<&str>, server: Option<&str>) -> Option<String> {
    let needed = |v: Option<&str>| matches!(v, Some("required") | Some("optional"));
    match (needed(client), needed(server)) {
        (true, true) => Some("both".into()),
        (true, false) => Some("client".into()),
        (false, true) => Some("server".into()),
        (false, false) => match (client, server) {
            (None, None) => None,
            _ => Some("singleplayer".into()),
        },
    }
}

fn kind_and_folder(version: &Version, project: &ProjectInfo) -> (&'static str, &'static str) {
    match project.project_type.as_str() {
        "resourcepack" => ("resourcepack", "resourcepacks"),
        "shader" => ("shader", "shaderpacks"),
        _ => {
            let is_datapack =
                version.loaders.iter().any(|l| l == "datapack") || project.categories.iter().any(|c| c == "datapack");
            if is_datapack {
                ("datapack", "datapacks")
            } else {
                ("mod", "mods")
            }
        }
    }
}

async fn fetch_version(http: &reqwest::Client, version_id: &str) -> Result<Version, String> {
    let resp = send(http.get(format!("{API}/version/{version_id}"))).await?;
    if !resp.status().is_success() {
        return Err(format!("version {version_id} failed: {}", resp.status()));
    }
    resp.json::<Version>().await.map_err(|e| e.to_string())
}

async fn fetch_project(http: &reqwest::Client, project_id: &str) -> Result<ProjectInfo, String> {
    let resp = send(http.get(format!("{API}/project/{project_id}"))).await?;
    if !resp.status().is_success() {
        return Err(format!("project {project_id} failed: {}", resp.status()));
    }
    resp.json::<ProjectInfo>().await.map_err(|e| e.to_string())
}

async fn resolve_latest_version(
    http: &reqwest::Client,
    project_id: &str,
    loader: &Option<String>,
    game_version: &Option<String>,
) -> Result<Option<String>, String> {
    let mut req = http.get(format!("{API}/project/{project_id}/version"));
    if let Some(l) = loader.as_ref().filter(|l| !l.is_empty()) {
        req = req.query(&[("loaders", serde_json::to_string(&[l]).map_err(|e| e.to_string())?)]);
    }
    if let Some(g) = game_version.as_ref().filter(|g| !g.is_empty()) {
        req = req.query(&[("game_versions", serde_json::to_string(&[g]).map_err(|e| e.to_string())?)]);
    }
    let list: Vec<Version> = send(req).await?.json().await.map_err(|e| e.to_string())?;
    Ok(list.into_iter().next().map(|v| v.id))
}

#[tauri::command]
pub async fn modrinth_install_with_deps(
    instance_id: String,
    version_id: String,
    game_version: Option<String>,
    loader: Option<String>,
) -> Result<Vec<InstalledItem>, String> {
    let http = client()?;
    let mut index = read_content_index(&instance_id);
    let mut visited: std::collections::HashSet<String> =
        index.items.iter().map(|i| i.project_id.clone()).collect();
    let mut added: Vec<InstalledItem> = Vec::new();

    install_rec(&http, &instance_id, &version_id, false, &game_version, &loader, &mut visited, &mut index, &mut added)
        .await?;

    write_content_index(&instance_id, &index)?;
    Ok(added)
}

#[allow(clippy::too_many_arguments)]
fn install_rec<'a>(
    http: &'a reqwest::Client,
    instance_id: &'a str,
    version_id: &'a str,
    is_dependency: bool,
    game_version: &'a Option<String>,
    loader: &'a Option<String>,
    visited: &'a mut std::collections::HashSet<String>,
    index: &'a mut ContentIndex,
    added: &'a mut Vec<InstalledItem>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + 'a>> {
    Box::pin(async move {
        let version = fetch_version(http, version_id).await?;
        if is_dependency && visited.contains(&version.project_id) {
            return Ok(());
        }
        let project = fetch_project(http, &version.project_id).await?;
        let (kind, folder) = kind_and_folder(&version, &project);

        let Some(file) = version.files.iter().find(|f| f.primary).or_else(|| version.files.first()) else {
            return Ok(());
        };

        if is_dependency
            && installed_by_other_provider(index, &project.title, &file.filename, "modrinth")
        {
            visited.insert(version.project_id.clone());
            return Ok(());
        }

        let dir = paths::instance_game_dir(instance_id).join(folder);
        std::fs::create_dir_all(&dir).map_err(|e| format!("create {folder}: {e}"))?;
        let bytes = download(http, &file.url).await?;
        std::fs::write(dir.join(safe_name(&file.filename)), &bytes).map_err(|e| format!("write file: {e}"))?;

        visited.insert(version.project_id.clone());
        let dep_project_ids: Vec<String> = version
            .dependencies
            .iter()
            .filter(|d| d.dependency_type == "required")
            .filter_map(|d| d.project_id.clone())
            .collect();
        let item = InstalledItem {
            project_id: version.project_id.clone(),
            version_id: version.id.clone(),
            kind: kind.to_string(),
            name: project.title.clone(),
            filename: file.filename.clone(),
            version_number: version.version_number.clone(),
            icon_url: project.icon_url.clone(),
            game_versions: version.game_versions.clone(),
            loaders: version.loaders.clone(),
            dependency: is_dependency,
            dependencies: dep_project_ids,
            installed_at: chrono::Utc::now().to_rfc3339(),
            provider: "modrinth".to_string(),
            author: None,
            env: None,
        };
        index.items.retain(|i| i.project_id != item.project_id);
        index.items.push(item.clone());
        added.push(item);

        for dep in version.dependencies.iter().filter(|d| d.dependency_type == "required") {
            let dep_version_id = if let Some(vid) = &dep.version_id {
                Some(vid.clone())
            } else if let Some(pid) = &dep.project_id {
                resolve_latest_version(http, pid, loader, game_version).await?
            } else {
                None
            };
            if let Some(dvid) = dep_version_id {
                install_rec(http, instance_id, &dvid, true, game_version, loader, visited, index, added).await?;
            }
        }
        Ok(())
    })
}

#[derive(Deserialize)]
struct MrIndex {
    name: String,
    files: Vec<MrFile>,
    #[serde(default)]
    dependencies: HashMap<String, String>,
}

#[derive(Deserialize)]
struct MrFile {
    path: String,
    downloads: Vec<String>,
    #[serde(default)]
    hashes: Option<MrHashes>,
    #[serde(default)]
    env: Option<MrEnv>,
}

#[derive(Deserialize)]
struct MrHashes {
    #[serde(default)]
    sha1: Option<String>,
}

#[derive(Deserialize)]
struct MrEnv {
    #[serde(default)]
    client: Option<String>,
}

#[derive(Clone, Serialize)]
struct ModpackProgress {
    instance_id: String,
    current: u64,
    total: u64,
    name: String,
}

#[tauri::command]
pub async fn modrinth_install_modpack(
    app: AppHandle,
    url: String,
    name_override: Option<String>,
    icon_url: Option<String>,
    project_id: Option<String>,
    version_id: Option<String>,
) -> Result<Instance, String> {
    let http = client()?;
    let pack_bytes = download(&http, &url).await?;
    let (raw_index, index) = parse_mrpack(&pack_bytes)?;

    let mc_version = index
        .dependencies
        .get("minecraft")
        .cloned()
        .ok_or("modpack has no Minecraft version")?;
    let loader = loader_from_deps(&index.dependencies);

    let name = name_override
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| index.name.clone());

    let mut instance = instances::create_instance(name, mc_version, loader, None, None)?;

    if let Some(icon) = icon_url.filter(|u| !u.trim().is_empty()) {
        if let Ok(bytes) = download(&http, &icon).await {
            if std::fs::write(paths::instance_icon_file(&instance.id), &bytes).is_ok() {
                instance.icon = Some("icon.png".to_string());
            }
        }
    }
    instance.modpack_project_id = project_id;
    instance.modpack_version_id = version_id;
    let _ = store::write_json(&paths::instance_config_file(&instance.id), &instance);

    apply_mrpack_files(&app, &http, &instance.id, &pack_bytes, &index, &raw_index, &instance.name).await?;
    Ok(instance)
}

#[tauri::command]
pub async fn import_file(
    app: AppHandle,
    path: String,
    name_override: Option<String>,
) -> Result<Instance, String> {
    let pack_bytes = std::fs::read(&path).map_err(|e| format!("read file: {e}"))?;

    if crate::commands::import::is_backup_zip(&pack_bytes) {
        return crate::commands::import::restore_backup_from_bytes(&pack_bytes);
    }

    if let Ok((raw_index, index)) = parse_mrpack(&pack_bytes) {
        let mc_version = index
            .dependencies
            .get("minecraft")
            .cloned()
            .ok_or("modpack has no Minecraft version")?;
        let loader = loader_from_deps(&index.dependencies);
        let name = name_override
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| index.name.clone());
        let http = client()?;
        let instance = instances::create_instance(name, mc_version, loader, None, None)?;
        apply_mrpack_files(&app, &http, &instance.id, &pack_bytes, &index, &raw_index, &instance.name).await?;
        return Ok(instance);
    }

    if zip_has_curseforge_manifest(&pack_bytes) {
        if crate::commands::curseforge::cf_enabled() {
            return crate::commands::curseforge::install_modpack_bytes(&app, pack_bytes, name_override, None).await;
        }
        return Err("This is a CurseForge modpack — add a CurseForge API key to import it.".into());
    }

    Err("Unrecognized file — expected a Modrinth .mrpack or CurseForge .zip.".into())
}

fn zip_has_curseforge_manifest(pack_bytes: &[u8]) -> bool {
    let Ok(mut archive) = zip::ZipArchive::new(Cursor::new(pack_bytes)) else { return false };
    let Ok(mut f) = archive.by_name("manifest.json") else { return false };
    let mut s = String::new();
    f.read_to_string(&mut s).is_ok() && (s.contains("minecraftModpack") || s.contains("\"manifestType\""))
}

#[derive(Serialize)]
struct MrpackIndex {
    #[serde(rename = "formatVersion")]
    format_version: u32,
    game: String,
    #[serde(rename = "versionId")]
    version_id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<String>,
    files: Vec<MrpackFile>,
    dependencies: HashMap<String, String>,
}

#[derive(Serialize)]
struct MrpackFile {
    path: String,
    hashes: MrpackHashes,
    #[serde(skip_serializing_if = "Option::is_none")]
    env: Option<MrpackEnv>,
    downloads: Vec<String>,
    #[serde(rename = "fileSize")]
    file_size: u64,
}

#[derive(Serialize)]
struct MrpackHashes {
    sha1: String,
    sha512: String,
}

#[derive(Serialize)]
struct MrpackEnv {
    client: String,
    server: String,
}

#[tauri::command]
pub async fn export_mrpack(
    id: String,
    dest: String,
    version: Option<String>,
    summary: Option<String>,
    exclude: Vec<String>,
    include: Vec<String>,
    optional_disabled: bool,
) -> Result<(), String> {
    use sha1::{Digest, Sha1};

    let instance: Instance =
        store::read_json(&paths::instance_config_file(&id))?.ok_or("instance not found")?;
    let game_dir = paths::instance_game_dir(&id);
    let filter = crate::commands::import::ExportFilter::new(include, exclude);

    struct Cand {
        rel: String,
        sha1: String,
        disabled: bool,
    }
    let mut cands: Vec<Cand> = Vec::new();
    for sub in ["mods", "resourcepacks", "shaderpacks"] {
        let Ok(entries) = std::fs::read_dir(game_dir.join(sub)) else { continue };
        for e in entries.flatten() {
            let path = e.path();
            if !path.is_file() {
                continue;
            }
            let name = e.file_name().to_string_lossy().into_owned();
            let disabled = name.ends_with(".disabled");
            if disabled && !optional_disabled {
                continue;
            }
            let rel = format!("{sub}/{name}");
            if !filter.includes(&rel) {
                continue;
            }
            let Ok(bytes) = std::fs::read(&path) else { continue };
            let mut h = Sha1::new();
            h.update(&bytes);
            cands.push(Cand { rel, sha1: format!("{:x}", h.finalize()), disabled });
        }
    }

    let mut files: Vec<MrpackFile> = Vec::new();
    let mut matched: std::collections::HashSet<String> = std::collections::HashSet::new();
    if !cands.is_empty() {
        let hashes: Vec<&str> = cands.iter().map(|c| c.sha1.as_str()).collect();
        let resp = send(
            http()
                .post(format!("{API}/version_files"))
                .json(&serde_json::json!({ "hashes": hashes, "algorithm": "sha1" })),
        )
        .await?;
        if resp.status().is_success() {
            let versions: HashMap<String, Version> = resp.json().await.map_err(|e| e.to_string())?;
            for c in &cands {
                let Some(version) = versions.get(&c.sha1) else { continue };
                let Some(file) = version.files.iter().find(|f| f.hashes.sha1.as_deref() == Some(c.sha1.as_str())) else { continue };
                let (Some(sha1), Some(sha512)) = (file.hashes.sha1.clone(), file.hashes.sha512.clone()) else { continue };
                if !file.url.starts_with("https://cdn.modrinth.com/") {
                    continue;
                }
                let path_out = if c.disabled {
                    c.rel.strip_suffix(".disabled").unwrap_or(&c.rel).to_string()
                } else {
                    c.rel.clone()
                };
                let env = if c.disabled {
                    Some(MrpackEnv { client: "optional".into(), server: "optional".into() })
                } else if c.rel.starts_with("resourcepacks/") || c.rel.starts_with("shaderpacks/") {
                    Some(MrpackEnv { client: "required".into(), server: "unsupported".into() })
                } else {
                    None
                };
                files.push(MrpackFile {
                    path: path_out,
                    hashes: MrpackHashes { sha1, sha512 },
                    env,
                    downloads: vec![file.url.clone()],
                    file_size: file.size,
                });
                matched.insert(c.rel.clone());
            }
        }
    }

    let mut dependencies = HashMap::new();
    dependencies.insert("minecraft".to_string(), instance.mc_version.clone());
    match &instance.loader {
        Loader::Fabric(v) => { dependencies.insert("fabric-loader".into(), v.clone()); }
        Loader::Quilt(v) => { dependencies.insert("quilt-loader".into(), v.clone()); }
        Loader::Forge(v) => { dependencies.insert("forge".into(), v.clone()); }
        Loader::NeoForge(v) => { dependencies.insert("neoforge".into(), v.clone()); }
        Loader::Vanilla => {}
    }

    let index = MrpackIndex {
        format_version: 1,
        game: "minecraft".into(),
        version_id: version.filter(|v| !v.trim().is_empty()).unwrap_or_else(|| "1.0.0".into()),
        name: instance.name.clone(),
        summary: summary.filter(|s| !s.trim().is_empty()),
        files,
        dependencies,
    };

    let out = std::fs::File::create(&dest).map_err(|e| format!("create {dest}: {e}"))?;
    let mut zip = zip::ZipWriter::new(out);
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let json = serde_json::to_vec_pretty(&index).map_err(|e| e.to_string())?;
    zip.start_file("modrinth.index.json", opts).map_err(|e| e.to_string())?;
    zip.write_all(&json).map_err(|e| e.to_string())?;

    add_overrides(&mut zip, &game_dir, "", &filter, &matched, optional_disabled, opts, &mut |_| {})?;

    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn add_overrides(
    zip: &mut zip::ZipWriter<std::fs::File>,
    base: &Path,
    rel: &str,
    filter: &crate::commands::import::ExportFilter,
    matched: &std::collections::HashSet<String>,
    optional_disabled: bool,
    opts: zip::write::SimpleFileOptions,
    on_file: &mut dyn FnMut(u64),
) -> Result<(), String> {
    let dir = if rel.is_empty() { base.to_path_buf() } else { base.join(rel) };
    let Ok(entries) = std::fs::read_dir(&dir) else { return Ok(()) };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if rel.is_empty() && crate::commands::import::is_never_top(&name) {
            continue;
        }
        let child_rel = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
        let path = e.path();
        if path.is_dir() {
            if filter.should_descend(&child_rel) {
                add_overrides(zip, base, &child_rel, filter, matched, optional_disabled, opts, on_file)?;
            }
            continue;
        }
        if !filter.includes(&child_rel) || matched.contains(&child_rel) || is_junk(&child_rel) {
            continue;
        }
        if child_rel.ends_with(".disabled") && !optional_disabled {
            continue;
        }
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        zip.start_file(format!("overrides/{child_rel}"), entry_opts(&child_rel, opts))
            .map_err(|e| e.to_string())?;
        zip.write_all(&bytes).map_err(|e| e.to_string())?;
        on_file(bytes.len() as u64);
    }
    Ok(())
}

fn entry_opts(
    rel: &str,
    opts: zip::write::SimpleFileOptions,
) -> zip::write::SimpleFileOptions {
    const ALREADY_COMPRESSED: &[&str] = &[
        ".jar", ".zip", ".png", ".jpg", ".jpeg", ".webp", ".ogg", ".mp3", ".gz", ".xz", ".7z",
    ];
    let name = rel.rsplit('/').next().unwrap_or(rel).to_lowercase();
    let name = name.strip_suffix(".disabled").unwrap_or(&name);
    if ALREADY_COMPRESSED.iter().any(|ext| name.ends_with(ext)) {
        opts.compression_method(zip::CompressionMethod::Stored)
    } else {
        opts
    }
}

fn is_junk(rel: &str) -> bool {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    matches!(name, ".DS_Store" | "Thumbs.db" | "thumbs.db") || name.ends_with(".pw.toml")
}

fn parse_mrpack(pack_bytes: &[u8]) -> Result<(String, MrIndex), String> {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(pack_bytes)).map_err(|e| format!("open mrpack: {e}"))?;
    let mut f = archive
        .by_name("modrinth.index.json")
        .map_err(|_| "mrpack missing modrinth.index.json".to_string())?;
    let mut raw = String::new();
    f.read_to_string(&mut raw).map_err(|e| e.to_string())?;
    let index: MrIndex = serde_json::from_str(&raw).map_err(|e| format!("parse index: {e}"))?;
    Ok((raw, index))
}

async fn apply_mrpack_files(
    app: &AppHandle,
    http: &reqwest::Client,
    instance_id: &str,
    pack_bytes: &Vec<u8>,
    index: &MrIndex,
    raw_index: &str,
    name: &str,
) -> Result<(), String> {
    let game_dir = paths::instance_game_dir(instance_id);

    let downloadable: Vec<&MrFile> = index
        .files
        .iter()
        .filter(|f| f.env.as_ref().and_then(|e| e.client.as_deref()) != Some("unsupported"))
        .collect();
    let total = downloadable.len() as u64;

    let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(10));
    let done = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let mut set = tokio::task::JoinSet::new();
    for file in &downloadable {
        let Some(src) = file.downloads.first().cloned() else { continue };
        let dest = join_safe(&game_dir, &file.path)?;
        let path_label = file.path.clone();
        let http = http.clone();
        let sem = sem.clone();
        let done = done.clone();
        let app = app.clone();
        let id = instance_id.to_string();
        let name = name.to_string();
        set.spawn(async move {
            let _permit = sem.acquire_owned().await.map_err(|e| e.to_string())?;
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            match download_direct(&http, &src).await {
                Ok(bytes) => {
                    std::fs::write(&dest, &bytes).map_err(|e| format!("write {path_label}: {e}"))?;
                }
                Err(e) => log::warn!("modpack file failed ({path_label}): {e}"),
            }
            let current = done.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            let _ = app.emit(
                "modrinth://modpack-progress",
                ModpackProgress { instance_id: id, current, total, name },
            );
            Ok::<(), String>(())
        });
    }
    while let Some(res) = set.join_next().await {
        if let Ok(Err(e)) = res {
            return Err(e);
        }
    }

    let mut archive =
        zip::ZipArchive::new(Cursor::new(pack_bytes)).map_err(|e| format!("open mrpack: {e}"))?;
    extract_overrides(&mut archive, &game_dir)?;

    let _ = std::fs::write(paths::instance_dir(instance_id).join("modrinth.index.json"), raw_index);

    if let Err(e) = index_modpack_content(http, instance_id, &index.files).await {
        log::warn!("modpack content index failed: {e}");
    }
    Ok(())
}

#[derive(Serialize)]
pub struct ModpackUpdate {
    version_id: String,
    version_number: String,
    version_type: String,
    changelog: String,
    date_published: String,
}

fn pick_modpack_candidate(versions: Vec<Version>, mc_version: &str) -> Option<Version> {
    versions
        .into_iter()
        .filter(|v| v.version_type != "alpha")
        .find(|v| v.game_versions.iter().any(|g| g == mc_version))
}

#[tauri::command]
pub async fn check_modpack_update(instance_id: String) -> Result<Option<ModpackUpdate>, String> {
    let instance: Instance =
        store::read_json(&paths::instance_config_file(&instance_id))?.ok_or("instance not found")?;
    let (Some(pid), Some(current)) = (instance.modpack_project_id, instance.modpack_version_id) else {
        return Ok(None);
    };

    let versions: Vec<Version> = send(http().get(format!("{API}/project/{pid}/version")))
        .await?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let Some(candidate) = pick_modpack_candidate(versions, &instance.mc_version) else {
        return Ok(None);
    };
    if candidate.id == current {
        return Ok(None);
    }
    Ok(Some(ModpackUpdate {
        version_id: candidate.id,
        version_number: candidate.version_number,
        version_type: candidate.version_type,
        changelog: candidate.changelog.unwrap_or_default(),
        date_published: candidate.date_published,
    }))
}

#[tauri::command]
pub async fn update_modpack(app: AppHandle, instance_id: String) -> Result<(), String> {
    crate::commands::snapshots::snapshot_before(&app, &instance_id, "before modpack update").await;

    let mut instance: Instance =
        store::read_json(&paths::instance_config_file(&instance_id))?.ok_or("instance not found")?;
    let pid = instance.modpack_project_id.clone().ok_or("not a modpack instance")?;

    let http = http();
    let versions: Vec<Version> = send(http.get(format!("{API}/project/{pid}/version")))
        .await?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let latest = pick_modpack_candidate(versions, &instance.mc_version)
        .ok_or("no compatible modpack version")?;
    let file = latest
        .files
        .iter()
        .find(|f| f.primary)
        .or_else(|| latest.files.first())
        .ok_or("modpack version has no file")?;

    delete_pack_files(&instance_id);

    let pack_bytes = download(&http, &file.url).await?;
    let (raw_index, index) = parse_mrpack(&pack_bytes)?;
    apply_mrpack_files(&app, &http, &instance_id, &pack_bytes, &index, &raw_index, &instance.name).await?;

    instance.modpack_version_id = Some(latest.id);
    store::write_json(&paths::instance_config_file(&instance_id), &instance)?;
    Ok(())
}

fn delete_pack_files(instance_id: &str) {
    let path = paths::instance_dir(instance_id).join("modrinth.index.json");
    let Ok(raw) = std::fs::read_to_string(&path) else { return };
    let Ok(index) = serde_json::from_str::<MrIndex>(&raw) else { return };
    let game_dir = paths::instance_game_dir(instance_id);
    for f in index.files {
        if let Ok(dest) = join_safe(&game_dir, &f.path) {
            let _ = std::fs::remove_file(dest);
        }
    }
}

async fn index_modpack_content(
    http: &reqwest::Client,
    instance_id: &str,
    files: &[MrFile],
) -> Result<(), String> {
    let mut by_hash: std::collections::HashMap<String, &MrFile> = std::collections::HashMap::new();
    for f in files {
        if let Some(h) = f.hashes.as_ref().and_then(|h| h.sha1.clone()) {
            by_hash.insert(h, f);
        }
    }
    if by_hash.is_empty() {
        return Ok(());
    }

    let hashes: Vec<&String> = by_hash.keys().collect();
    let resp = send(
        http.post(format!("{API}/version_files"))
            .json(&serde_json::json!({ "hashes": hashes, "algorithm": "sha1" })),
    )
    .await?;
    if !resp.status().is_success() {
        return Err(format!("version_files: {}", resp.status()));
    }
    let versions: std::collections::HashMap<String, Version> =
        resp.json().await.map_err(|e| e.to_string())?;

    let project_ids: Vec<String> = versions
        .values()
        .map(|v| v.project_id.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let ids_json = serde_json::to_string(&project_ids).map_err(|e| e.to_string())?;
    let projects: Vec<ProjectInfo> = send(
        http.get(format!("{API}/projects")).query(&[("ids", ids_json)]),
    )
    .await?
    .json()
    .await
    .map_err(|e| e.to_string())?;
    let proj_by_id: std::collections::HashMap<String, ProjectInfo> =
        projects.into_iter().map(|p| (p.id.clone(), p)).collect();

    let mut index = read_content_index(instance_id);
    for (hash, version) in &versions {
        let Some(file) = by_hash.get(hash) else { continue };
        let filename = safe_name(&file.path);
        let project = proj_by_id.get(&version.project_id);
        let kind = match project {
            Some(p) => kind_and_folder(version, p).0,
            None => "mod",
        };
        let item = InstalledItem {
            project_id: version.project_id.clone(),
            version_id: version.id.clone(),
            kind: kind.to_string(),
            name: project.map(|p| p.title.clone()).unwrap_or_else(|| filename.clone()),
            filename,
            version_number: version.version_number.clone(),
            icon_url: project.and_then(|p| p.icon_url.clone()),
            game_versions: version.game_versions.clone(),
            loaders: version.loaders.clone(),
            dependency: false,
            dependencies: Vec::new(),
            installed_at: chrono::Utc::now().to_rfc3339(),
            provider: "modrinth".to_string(),
            author: None,
            env: None,
        };
        index.items.retain(|i| i.project_id != item.project_id);
        index.items.push(item);
    }
    write_content_index(instance_id, &index)?;
    Ok(())
}

#[tauri::command]
pub async fn match_local_mods(instance_id: String) -> Result<usize, String> {
    use sha1::{Digest, Sha1};

    let dir = paths::instance_game_dir(&instance_id).join("mods");
    let known: std::collections::HashSet<String> =
        read_content_index(&instance_id).items.iter().map(|i| i.filename.clone()).collect();

    let mut by_hash: HashMap<String, String> = HashMap::new();
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Ok(0),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let raw = entry.file_name().to_string_lossy().into_owned();
        let base = raw.strip_suffix(".disabled").unwrap_or(&raw).to_string();
        if !base.ends_with(".jar") || known.contains(&base) {
            continue;
        }
        if let Ok(bytes) = std::fs::read(&path) {
            let mut hasher = Sha1::new();
            hasher.update(&bytes);
            by_hash.insert(format!("{:x}", hasher.finalize()), base);
        }
    }
    if by_hash.is_empty() {
        return Ok(0);
    }

    let http = http();
    let hashes: Vec<&String> = by_hash.keys().collect();
    let resp = send(
        http.post(format!("{API}/version_files"))
            .json(&serde_json::json!({ "hashes": hashes, "algorithm": "sha1" })),
    )
    .await?;
    if !resp.status().is_success() {
        return Err(format!("version_files: {}", resp.status()));
    }
    let versions: HashMap<String, Version> = resp.json().await.map_err(|e| e.to_string())?;
    if versions.is_empty() {
        return Ok(0);
    }

    let project_ids: Vec<String> = versions
        .values()
        .map(|v| v.project_id.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let ids_json = serde_json::to_string(&project_ids).map_err(|e| e.to_string())?;
    let projects: Vec<ProjectInfo> = send(
        http.get(format!("{API}/projects")).query(&[("ids", ids_json)]),
    )
    .await?
    .json()
    .await
    .map_err(|e| e.to_string())?;
    let proj_by_id: HashMap<String, ProjectInfo> =
        projects.into_iter().map(|p| (p.id.clone(), p)).collect();

    let mut index = read_content_index(&instance_id);
    let mut count = 0usize;
    for (hash, version) in &versions {
        let Some(filename) = by_hash.get(hash) else { continue };
        let project = proj_by_id.get(&version.project_id);
        let kind = match project {
            Some(p) => kind_and_folder(version, p).0,
            None => "mod",
        };
        let item = InstalledItem {
            project_id: version.project_id.clone(),
            version_id: version.id.clone(),
            kind: kind.to_string(),
            name: project.map(|p| p.title.clone()).unwrap_or_else(|| filename.clone()),
            filename: filename.clone(),
            version_number: version.version_number.clone(),
            icon_url: project.and_then(|p| p.icon_url.clone()),
            game_versions: version.game_versions.clone(),
            loaders: version.loaders.clone(),
            dependency: false,
            dependencies: Vec::new(),
            installed_at: chrono::Utc::now().to_rfc3339(),
            provider: "modrinth".to_string(),
            author: None,
            env: None,
        };
        index.items.retain(|i| i.project_id != item.project_id && i.filename != item.filename);
        index.items.push(item);
        count += 1;
    }
    write_content_index(&instance_id, &index)?;
    Ok(count)
}

fn loader_from_deps(deps: &HashMap<String, String>) -> Loader {
    if let Some(v) = deps.get("fabric-loader") {
        Loader::Fabric(v.clone())
    } else if let Some(v) = deps.get("quilt-loader") {
        Loader::Quilt(v.clone())
    } else if let Some(v) = deps.get("neoforge") {
        Loader::NeoForge(v.clone())
    } else if let Some(v) = deps.get("forge") {
        Loader::Forge(v.clone())
    } else {
        Loader::Vanilla
    }
}

fn extract_overrides(
    archive: &mut zip::ZipArchive<Cursor<&Vec<u8>>>,
    game_dir: &Path,
) -> Result<(), String> {
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        let rel = name
            .strip_prefix("overrides/")
            .or_else(|| name.strip_prefix("client-overrides/"));
        let Some(rel) = rel else { continue };

        let dest = join_safe(game_dir, rel)?;
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        std::fs::write(&dest, &buf).map_err(|e| e.to_string())?;
    }
    Ok(())
}

async fn download(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, String> {
    let resp = send(client.get(url)).await?;
    if !resp.status().is_success() {
        return Err(format!("download failed ({}): {url}", resp.status()));
    }
    Ok(resp.bytes().await.map_err(|e| e.to_string())?.to_vec())
}

async fn download_direct(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, String> {
    let resp = client.get(url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("download failed ({}): {url}", resp.status()));
    }
    Ok(resp.bytes().await.map_err(|e| e.to_string())?.to_vec())
}

fn safe_name(name: &str) -> String {
    Path::new(name)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".to_string())
}

#[cfg(test)]
mod tests {
    use super::{installed_by_other_provider, ContentIndex, InstalledItem};

    fn item(name: &str, provider: &str) -> InstalledItem {
        InstalledItem {
            project_id: format!("{provider}-{name}"),
            version_id: "v".into(),
            kind: "mod".into(),
            name: name.into(),
            filename: format!("{name}.jar"),
            version_number: "1.0".into(),
            icon_url: None,
            game_versions: vec![],
            loaders: vec![],
            dependency: false,
            dependencies: vec![],
            installed_at: String::new(),
            provider: provider.into(),
            author: None,
            env: None,
        }
    }

    #[test]
    fn cross_provider_duplicates_are_detected() {
        let index = ContentIndex { items: vec![item("Just Enough Items", "modrinth")] };
        let hit = |name: &str, file: &str, provider: &str| {
            installed_by_other_provider(&index, name, file, provider)
        };

        assert!(hit("just enough items ", "Just Enough Items.jar", "curseforge"));
        assert!(hit("JEI", "just enough items.jar", "curseforge"));
        assert!(hit("Just Enough Items", "jei-neoforge-19.21.0.jar", "curseforge"));
        assert!(!hit("Just Enough Items", "Just Enough Items.jar", "modrinth"));
        assert!(!hit("Sodium", "sodium-0.6.jar", "curseforge"));
        assert!(!hit("", "", "curseforge"));
    }
}

fn join_safe(base: &Path, rel: &str) -> Result<PathBuf, String> {
    let mut out = base.to_path_buf();
    for comp in Path::new(rel).components() {
        match comp {
            Component::Normal(c) => out.push(c),
            Component::CurDir => {}
            _ => return Err(format!("unsafe path in modpack: {rel}")),
        }
    }
    Ok(out)
}
