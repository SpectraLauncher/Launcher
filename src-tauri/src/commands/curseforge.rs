use std::collections::HashMap;
use std::io::{Cursor, Read, Write};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::commands::instances;
use crate::commands::modrinth::{read_content_index, write_content_index, InstalledItem};
use crate::models::{Instance, Loader};
use crate::paths;

// Calls go through the Spectra server, which holds the CurseForge key. Shipping
// the key in the binary made it readable with `strings` and broke CurseForge's
// terms; nothing about it can be kept secret on the user's machine.
const API: &str = "https://spectra.makoto.com.pl/api/curseforge";
const GAME_ID: i64 = 432;

// Whether the server has a key configured. Assumed yes until it answers 501,
// and set back on the next success, so a server configured later heals itself.
static CF_AVAILABLE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

#[tauri::command]
pub fn cf_enabled() -> bool {
    CF_AVAILABLE.load(std::sync::atomic::Ordering::Relaxed)
}

fn http() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(concat!("MakotoPD/Spectra-Launcher/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("build curseforge client")
    })
}

async fn send(req: reqwest::RequestBuilder) -> Result<reqwest::Response, String> {
    use std::sync::atomic::Ordering;

    let mut attempt = 0u32;
    loop {
        let r = req.try_clone().ok_or("request is not retryable")?;
        let resp = r.send().await.map_err(|e| e.to_string())?;

        if resp.status().as_u16() == 501 {
            CF_AVAILABLE.store(false, Ordering::Relaxed);
            return Err("CurseForge is not configured on the Spectra server".into());
        }

        if resp.status().as_u16() != 429 || attempt >= 3 {
            CF_AVAILABLE.store(true, Ordering::Relaxed);
            return Ok(resp);
        }
        let wait = resp
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or((1u64 << attempt).min(8));
        tokio::time::sleep(std::time::Duration::from_secs(wait.clamp(1, 8))).await;
        attempt += 1;
    }
}

fn class_id(kind: &str) -> i64 {
    match kind {
        "modpack" => 4471,
        "resourcepack" => 12,
        "shader" => 6552,
        "datapack" => 6945,
        _ => 6,
    }
}

fn loader_id(loader: &str) -> Option<u8> {
    match loader {
        "forge" => Some(1),
        "fabric" => Some(4),
        "quilt" => Some(5),
        "neoforge" => Some(6),
        _ => None,
    }
}

fn sort_field(index: &str) -> u8 {
    match index {
        "downloads" => 6,
        "newest" | "updated" => 3,
        "follows" => 2,
        _ => 2,
    }
}

#[derive(Deserialize)]
struct CfLogo {
    #[serde(default)]
    thumbnail_url: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

#[derive(Deserialize)]
struct CfAuthor {
    #[serde(default)]
    name: String,
}

#[derive(Deserialize)]
struct CfCategoryRef {
    #[serde(default)]
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfLinks {
    #[serde(default)]
    website_url: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfFileIndex {
    #[serde(default)]
    game_version: String,
    #[serde(default)]
    file_id: i64,
    #[serde(default)]
    filename: String,
    #[serde(default)]
    mod_loader: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfMod {
    id: i64,
    name: String,
    #[serde(default)]
    slug: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    download_count: f64,
    #[serde(default)]
    logo: Option<CfLogo>,
    #[serde(default)]
    links: Option<CfLinks>,
    #[serde(default)]
    authors: Vec<CfAuthor>,
    #[serde(default)]
    categories: Vec<CfCategoryRef>,
    #[serde(default)]
    latest_files_indexes: Vec<CfFileIndex>,
    #[serde(default)]
    screenshots: Vec<CfScreenshot>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfScreenshot {
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    thumbnail_url: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

#[derive(Deserialize, Clone)]
struct CfHashEntry {
    value: String,
    algo: i64,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CfDependency {
    mod_id: i64,
    relation_type: i64,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CfFile {
    id: i64,
    #[serde(default)]
    mod_id: i64,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    file_name: String,
    #[serde(default)]
    file_date: String,
    #[serde(default)]
    download_url: Option<String>,
    #[serde(default)]
    file_length: u64,
    #[serde(default)]
    file_fingerprint: u64,
    #[serde(default = "default_true")]
    is_available: bool,
    #[serde(default)]
    download_count: f64,
    #[serde(default = "default_release_type")]
    release_type: i64,
    #[serde(default)]
    game_versions: Vec<String>,
    #[serde(default)]
    hashes: Vec<CfHashEntry>,
    #[serde(default)]
    dependencies: Vec<CfDependency>,
}

fn default_release_type() -> i64 {
    1
}
fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct CfPagination {
    #[serde(default)]
    total_count: u64,
}

#[derive(Deserialize)]
struct CfListResponse<T> {
    data: Vec<T>,
    #[serde(default)]
    pagination: Option<CfPagination>,
}

#[derive(Deserialize)]
struct CfDataResponse<T> {
    data: T,
}

#[derive(Serialize)]
pub struct Hit {
    project_id: String,
    slug: String,
    title: String,
    description: String,
    author: String,
    project_type: String,
    downloads: u64,
    follows: u64,
    icon_url: Option<String>,
    categories: Vec<String>,
    versions: Vec<String>,
    client_side: Option<String>,
    server_side: Option<String>,
}

#[derive(Serialize)]
pub struct SearchResponse {
    hits: Vec<Hit>,
    total_hits: u64,
    offset: u64,
    limit: u64,
}

#[derive(Serialize)]
pub struct VersionFileHashes {
    sha1: Option<String>,
    sha512: Option<String>,
}

#[derive(Serialize)]
pub struct VersionFile {
    url: String,
    filename: String,
    primary: bool,
    size: u64,
    hashes: VersionFileHashes,
}

#[derive(Serialize)]
pub struct Dependency {
    project_id: Option<String>,
    version_id: Option<String>,
    dependency_type: String,
}

#[derive(Serialize)]
pub struct Version {
    id: String,
    project_id: String,
    name: String,
    version_number: String,
    version_type: String,
    loaders: Vec<String>,
    game_versions: Vec<String>,
    downloads: u64,
    date_published: String,
    files: Vec<VersionFile>,
    dependencies: Vec<Dependency>,
}

#[derive(Serialize)]
pub struct GalleryItem {
    url: String,
    raw_url: Option<String>,
    title: Option<String>,
    description: Option<String>,
    featured: bool,
}

#[derive(Serialize)]
pub struct ProjectFull {
    id: String,
    title: String,
    description: String,
    body: String,
    icon_url: Option<String>,
    gallery: Vec<GalleryItem>,
}

fn logo_url(logo: &Option<CfLogo>) -> Option<String> {
    logo.as_ref().and_then(|l| l.thumbnail_url.clone().or_else(|| l.url.clone()))
}

fn mod_to_hit(m: CfMod, kind: &str) -> Hit {
    let versions: Vec<String> = {
        let mut v: Vec<String> = m
            .latest_files_indexes
            .iter()
            .map(|i| i.game_version.clone())
            .filter(|s| s.contains('.'))
            .collect();
        v.sort();
        v.dedup();
        v
    };
    Hit {
        project_id: m.id.to_string(),
        slug: m.slug,
        title: m.name,
        description: m.summary,
        author: m.authors.first().map(|a| a.name.clone()).unwrap_or_default(),
        project_type: kind.to_string(),
        downloads: m.download_count as u64,
        follows: 0,
        icon_url: logo_url(&m.logo),
        categories: m.categories.into_iter().map(|c| c.name).collect(),
        versions,
        client_side: None,
        server_side: None,
    }
}

fn split_game_versions(raw: &[String]) -> (Vec<String>, Vec<String>) {
    let mut mc = Vec::new();
    let mut loaders = Vec::new();
    for v in raw {
        let lower = v.to_lowercase();
        match lower.as_str() {
            "forge" | "fabric" | "quilt" | "neoforge" => loaders.push(lower),
            _ if v.contains('.') => mc.push(v.clone()),
            _ => {}
        }
    }
    (mc, loaders)
}

fn file_to_version(f: CfFile) -> Version {
    let (game_versions, loaders) = split_game_versions(&f.game_versions);
    let version_type = match f.release_type {
        2 => "beta",
        3 => "alpha",
        _ => "release",
    }
    .to_string();
    let sha1 = f
        .hashes
        .iter()
        .find(|h| h.algo == 1)
        .map(|h| h.value.clone());
    let deps = f
        .dependencies
        .iter()
        .filter(|d| d.relation_type == 3)
        .map(|d| Dependency {
            project_id: Some(d.mod_id.to_string()),
            version_id: None,
            dependency_type: "required".to_string(),
        })
        .collect();
    let version_number = if f.display_name.is_empty() { f.file_name.clone() } else { f.display_name.clone() };
    Version {
        id: f.id.to_string(),
        project_id: f.mod_id.to_string(),
        name: f.display_name,
        version_number,
        version_type,
        loaders,
        game_versions,
        downloads: f.download_count as u64,
        date_published: f.file_date,
        files: vec![VersionFile {
            url: f.download_url.unwrap_or_default(),
            filename: f.file_name,
            primary: true,
            size: f.file_length,
            hashes: VersionFileHashes { sha1, sha512: None },
        }],
        dependencies: deps,
    }
}

#[derive(Deserialize)]
pub struct SearchParams {
    pub query: String,
    pub project_type: String,
    #[serde(default)]
    pub loaders: Vec<String>,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub index: String,
    #[serde(default)]
    pub offset: u64,
    #[serde(default)]
    pub limit: u64,
}

#[tauri::command]
pub async fn curseforge_search(params: SearchParams) -> Result<SearchResponse, String> {
    let kind = params.project_type.clone();
    let page_size = if params.limit == 0 { 20 } else { params.limit.min(50) };

    let mut query: Vec<(String, String)> = vec![
        ("gameId".into(), GAME_ID.to_string()),
        ("classId".into(), class_id(&kind).to_string()),
        ("index".into(), params.offset.to_string()),
        ("pageSize".into(), page_size.to_string()),
        ("sortField".into(), sort_field(&params.index).to_string()),
        ("sortOrder".into(), "desc".into()),
    ];
    if !params.query.trim().is_empty() {
        query.push(("searchFilter".into(), params.query.clone()));
    }
    if let Some(v) = params.game_versions.first() {
        query.push(("gameVersion".into(), v.clone()));
    }
    if let Some(id) = params.loaders.iter().find_map(|l| loader_id(l)) {
        query.push(("modLoaderType".into(), id.to_string()));
    }
    if !params.categories.is_empty() {
        let cats = categories_for(class_id(&kind)).await;
        let ids: Vec<String> = params
            .categories
            .iter()
            .filter_map(|c| cats.iter().find(|(n, _)| n.eq_ignore_ascii_case(c)).map(|(_, id)| id.to_string()))
            .collect();
        if !ids.is_empty() {
            query.push(("categoryIds".into(), format!("[{}]", ids.join(","))));
        }
    }

    let resp = send(http().get(format!("{API}/mods/search")).query(&query)).await?;
    if !resp.status().is_success() {
        return Err(format!("CurseForge search failed: {}", resp.status()));
    }
    let parsed: CfListResponse<CfMod> = resp.json().await.map_err(|e| e.to_string())?;
    let total_hits = parsed.pagination.map(|p| p.total_count).unwrap_or(0);
    let hits = parsed.data.into_iter().map(|m| mod_to_hit(m, &kind)).collect();
    Ok(SearchResponse { hits, total_hits, offset: params.offset, limit: page_size })
}

#[tauri::command]
pub async fn curseforge_versions(
    project_id: String,
    loaders: Option<Vec<String>>,
    game_versions: Option<Vec<String>>,
) -> Result<Vec<Version>, String> {
    let mut query: Vec<(String, String)> = vec![("pageSize".into(), "50".into()), ("index".into(), "0".into())];
    if let Some(v) = game_versions.and_then(|g| g.into_iter().next()) {
        query.push(("gameVersion".into(), v));
    }
    if let Some(id) = loaders.and_then(|l| l.iter().find_map(|x| loader_id(x))) {
        query.push(("modLoaderType".into(), id.to_string()));
    }

    let resp = send(http().get(format!("{API}/mods/{project_id}/files")).query(&query)).await?;
    if !resp.status().is_success() {
        return Err(format!("CurseForge versions failed: {}", resp.status()));
    }
    let parsed: CfListResponse<CfFile> = resp.json().await.map_err(|e| e.to_string())?;
    Ok(parsed.data.into_iter().map(file_to_version).collect())
}

#[tauri::command]
pub async fn curseforge_project(id: String) -> Result<ProjectFull, String> {
    let resp = send(http().get(format!("{API}/mods/{id}"))).await?;
    if !resp.status().is_success() {
        return Err(format!("CurseForge project failed: {}", resp.status()));
    }
    let m: CfDataResponse<CfMod> = resp.json().await.map_err(|e| e.to_string())?;
    let m = m.data;

    let body = match send(http().get(format!("{API}/mods/{id}/description"))).await {
        Ok(r) if r.status().is_success() => r
            .json::<CfDataResponse<String>>()
            .await
            .map(|d| d.data)
            .unwrap_or_default(),
        _ => String::new(),
    };

    let gallery = m
        .screenshots
        .into_iter()
        .filter_map(|s| {
            let raw = s.url.clone();
            let thumb = s.thumbnail_url.or_else(|| s.url.clone());
            thumb.map(|url| GalleryItem {
                url,
                raw_url: raw,
                title: s.title,
                description: s.description,
                featured: false,
            })
        })
        .collect();

    Ok(ProjectFull {
        id: m.id.to_string(),
        title: m.name,
        description: m.summary,
        body,
        icon_url: logo_url(&m.logo),
        gallery,
    })
}

#[derive(Serialize, Clone)]
pub struct Category {
    name: String,
    header: String,
}

#[derive(Deserialize)]
struct CfCategory {
    id: i64,
    name: String,
}

fn category_cache() -> &'static std::sync::Mutex<HashMap<i64, Vec<(String, i64)>>> {
    static C: std::sync::OnceLock<std::sync::Mutex<HashMap<i64, Vec<(String, i64)>>>> =
        std::sync::OnceLock::new();
    C.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

async fn categories_for(class_id: i64) -> Vec<(String, i64)> {
    if let Ok(cache) = category_cache().lock() {
        if let Some(v) = cache.get(&class_id) {
            return v.clone();
        }
    }
    let fetched = match send(
        http()
            .get(format!("{API}/categories"))
            .query(&[("gameId", GAME_ID.to_string()), ("classId", class_id.to_string())]),
    )
    .await
    {
        Ok(r) if r.status().is_success() => r
            .json::<CfListResponse<CfCategory>>()
            .await
            .map(|p| p.data.into_iter().map(|c| (c.name, c.id)).collect::<Vec<_>>())
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    if let Ok(mut cache) = category_cache().lock() {
        cache.insert(class_id, fetched.clone());
    }
    fetched
}

#[tauri::command]
pub async fn curseforge_categories(project_type: String) -> Result<Vec<Category>, String> {
    let cats = categories_for(class_id(&project_type)).await;
    Ok(cats.into_iter().map(|(name, id)| Category { name, header: id.to_string() }).collect())
}

#[derive(Serialize, Deserialize, Clone)]
pub struct BlockedMod {
    name: String,
    filename: String,
    project_id: String,
    file_id: String,
    url: String,
    fingerprint: u64,
}

#[derive(Serialize)]
pub struct CfInstallResult {
    added: Vec<InstalledItem>,
    blocked: Vec<BlockedMod>,
}

#[tauri::command]
pub async fn curseforge_install_with_deps(
    instance_id: String,
    project_id: String,
    file_id: String,
    game_version: Option<String>,
    loader: Option<String>,
) -> Result<CfInstallResult, String> {
    let mut index = read_content_index(&instance_id);
    let mut visited: std::collections::HashSet<String> =
        index.items.iter().map(|i| i.project_id.clone()).collect();
    let mut added = Vec::new();
    let mut blocked = Vec::new();

    install_rec(
        &instance_id,
        &project_id,
        &file_id,
        false,
        &game_version,
        &loader,
        &mut visited,
        &mut index,
        &mut added,
        &mut blocked,
    )
    .await?;

    write_content_index(&instance_id, &index)?;
    if !blocked.is_empty() {
        let mut all = get_blocked_mods(instance_id.clone());
        for b in &blocked {
            if !all.iter().any(|x| x.file_id == b.file_id) {
                all.push(b.clone());
            }
        }
        save_blocked_mods(&instance_id, &all);
    }
    Ok(CfInstallResult { added, blocked })
}

#[allow(clippy::too_many_arguments)]
fn install_rec<'a>(
    instance_id: &'a str,
    project_id: &'a str,
    file_id: &'a str,
    is_dependency: bool,
    game_version: &'a Option<String>,
    loader: &'a Option<String>,
    visited: &'a mut std::collections::HashSet<String>,
    index: &'a mut crate::commands::modrinth::ContentIndex,
    added: &'a mut Vec<InstalledItem>,
    blocked: &'a mut Vec<BlockedMod>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + 'a>> {
    Box::pin(async move {
        if is_dependency && visited.contains(project_id) {
            return Ok(());
        }
        let file = fetch_file(project_id, file_id).await?;
        let m = fetch_mod(project_id).await?;
        if is_dependency
            && crate::commands::modrinth::installed_by_other_provider(
                index,
                &m.name,
                &file.file_name,
                "curseforge",
            )
        {
            visited.insert(project_id.to_string());
            return Ok(());
        }
        let (kind, folder) = kind_and_folder(&m);

        let dir = paths::instance_game_dir(instance_id).join(folder);
        std::fs::create_dir_all(&dir).map_err(|e| format!("create {folder}: {e}"))?;

        match &file.download_url {
            Some(url) if !url.is_empty() => {
                let bytes = download(url).await?;
                std::fs::write(dir.join(safe_name(&file.file_name)), &bytes)
                    .map_err(|e| format!("write file: {e}"))?;
            }
            _ => {
                blocked.push(BlockedMod {
                    name: m.name.clone(),
                    filename: safe_name(&file.file_name),
                    project_id: project_id.to_string(),
                    file_id: file_id.to_string(),
                    url: blocked_url(Some(&m), m.id, file.id),
                    fingerprint: file.file_fingerprint,
                });
                visited.insert(project_id.to_string());
                return Ok(());
            }
        }

        visited.insert(project_id.to_string());
        let (game_versions, loaders) = split_game_versions(&file.game_versions);
        let dep_project_ids: Vec<String> = file
            .dependencies
            .iter()
            .filter(|d| d.relation_type == 3)
            .map(|d| d.mod_id.to_string())
            .collect();
        let item = InstalledItem {
            project_id: project_id.to_string(),
            version_id: file_id.to_string(),
            kind: kind.to_string(),
            name: m.name.clone(),
            filename: file.file_name.clone(),
            version_number: if file.display_name.is_empty() { file.file_name.clone() } else { file.display_name.clone() },
            icon_url: logo_url(&m.logo),
            game_versions,
            loaders,
            dependency: is_dependency,
            dependencies: dep_project_ids,
            installed_at: chrono::Utc::now().to_rfc3339(),
            provider: "curseforge".to_string(),
            author: None,
            env: None,
        };
        index.items.retain(|i| i.project_id != item.project_id);
        index.items.push(item.clone());
        added.push(item);

        for dep in file.dependencies.iter().filter(|d| d.relation_type == 3) {
            let dep_pid = dep.mod_id.to_string();
            if visited.contains(&dep_pid) {
                continue;
            }
            if let Some(dep_file) = resolve_latest_file(&dep_pid, loader, game_version).await? {
                install_rec(
                    instance_id, &dep_pid, &dep_file, true, game_version, loader, visited, index, added, blocked,
                )
                .await?;
            }
        }
        Ok(())
    })
}

async fn fetch_file(project_id: &str, file_id: &str) -> Result<CfFile, String> {
    let resp = send(http().get(format!("{API}/mods/{project_id}/files/{file_id}"))).await?;
    if !resp.status().is_success() {
        return Err(format!("CurseForge file failed: {}", resp.status()));
    }
    let d: CfDataResponse<CfFile> = resp.json().await.map_err(|e| e.to_string())?;
    Ok(d.data)
}

async fn fetch_mod(project_id: &str) -> Result<CfMod, String> {
    let resp = send(http().get(format!("{API}/mods/{project_id}"))).await?;
    if !resp.status().is_success() {
        return Err(format!("CurseForge mod failed: {}", resp.status()));
    }
    let d: CfDataResponse<CfMod> = resp.json().await.map_err(|e| e.to_string())?;
    Ok(d.data)
}

async fn resolve_latest_file(
    project_id: &str,
    loader: &Option<String>,
    game_version: &Option<String>,
) -> Result<Option<String>, String> {
    let mut query: Vec<(String, String)> = vec![("pageSize".into(), "50".into())];
    if let Some(v) = game_version {
        query.push(("gameVersion".into(), v.clone()));
    }
    if let Some(id) = loader.as_ref().and_then(|l| loader_id(l)) {
        query.push(("modLoaderType".into(), id.to_string()));
    }
    let resp = send(http().get(format!("{API}/mods/{project_id}/files")).query(&query)).await?;
    if !resp.status().is_success() {
        return Ok(None);
    }
    let parsed: CfListResponse<CfFile> = resp.json().await.map_err(|e| e.to_string())?;
    Ok(parsed.data.into_iter().next().map(|f| f.id.to_string()))
}

pub async fn bulk_latest_files(
    project_ids: &[String],
    loaders: &Option<Vec<String>>,
    game_versions: &Option<Vec<String>>,
) -> HashMap<String, (String, String)> {
    let ids: Vec<i64> = project_ids.iter().filter_map(|s| s.parse().ok()).collect();
    let mods = bulk_mods(&ids).await.unwrap_or_default();
    let want_loader = loaders.as_ref().and_then(|l| l.iter().find_map(|x| loader_id(x))).map(|l| l as i64);
    let want_gv = game_versions.as_ref().and_then(|g| g.first().cloned());

    let mut out = HashMap::new();
    for (id, m) in mods {
        let best = m
            .latest_files_indexes
            .iter()
            .filter(|i| {
                want_gv.as_ref().map(|gv| &i.game_version == gv).unwrap_or(true)
                    && want_loader.map(|l| i.mod_loader == Some(l)).unwrap_or(true)
            })
            .max_by_key(|i| i.file_id);
        if let Some(b) = best {
            out.insert(id.to_string(), (b.file_id.to_string(), b.filename.clone()));
        }
    }
    out
}

#[tauri::command]
pub async fn curseforge_update_all(
    instance_id: String,
    loaders: Option<Vec<String>>,
    game_versions: Option<Vec<String>>,
) -> Result<usize, String> {
    let mut index = read_content_index(&instance_id);
    let cf: Vec<(usize, String, String)> = index
        .items
        .iter()
        .enumerate()
        .filter(|(_, i)| i.kind == "mod" && i.provider == "curseforge")
        .map(|(pos, i)| (pos, i.project_id.clone(), i.version_id.clone()))
        .collect();
    if cf.is_empty() {
        return Ok(0);
    }

    let pids: Vec<String> = cf.iter().map(|(_, p, _)| p.clone()).collect();
    let latest = bulk_latest_files(&pids, &loaders, &game_versions).await;

    let mut need: Vec<(usize, i64)> = Vec::new();
    for (pos, pid, vid) in &cf {
        if let Some((new_id, _)) = latest.get(pid) {
            if new_id != vid {
                if let Ok(fid) = new_id.parse::<i64>() {
                    need.push((*pos, fid));
                }
            }
        }
    }
    if need.is_empty() {
        return Ok(0);
    }

    let file_ids: Vec<i64> = need.iter().map(|(_, f)| *f).collect();
    let files = bulk_files(&file_ids).await.unwrap_or_default();

    let mods_dir = paths::instance_game_dir(&instance_id).join("mods");
    std::fs::create_dir_all(&mods_dir).map_err(|e| e.to_string())?;

    let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(8));
    let mut set = tokio::task::JoinSet::new();
    for (pos, fid) in need {
        let Some(file) = files.get(&fid).cloned() else { continue };
        let url = match &file.download_url {
            Some(u) if !u.is_empty() => u.clone(),
            _ => continue,
        };
        let mods_dir = mods_dir.clone();
        let sem = sem.clone();
        set.spawn(async move {
            let _permit = sem.acquire_owned().await.ok()?;
            let bytes = download(&url).await.ok()?;
            std::fs::write(mods_dir.join(safe_name(&file.file_name)), &bytes).ok()?;
            Some((pos, file))
        });
    }

    let mut updated = 0usize;
    while let Some(res) = set.join_next().await {
        let Ok(Some((pos, file))) = res else { continue };
        let item = &mut index.items[pos];
        let old = item.filename.clone();
        let new_name = safe_name(&file.file_name);
        if old != new_name {
            let _ = std::fs::remove_file(mods_dir.join(&old));
            let _ = std::fs::remove_file(mods_dir.join(format!("{old}.disabled")));
        }
        let (gv, loaders_v) = split_game_versions(&file.game_versions);
        item.version_id = file.id.to_string();
        item.version_number = if file.display_name.is_empty() { file.file_name.clone() } else { file.display_name.clone() };
        item.filename = new_name;
        item.game_versions = gv;
        item.loaders = loaders_v;
        updated += 1;
    }
    write_content_index(&instance_id, &index)?;
    Ok(updated)
}

fn kind_and_folder(m: &CfMod) -> (&'static str, &'static str) {
    let cats: Vec<String> = m.categories.iter().map(|c| c.name.to_lowercase()).collect();
    if cats.iter().any(|c| c.contains("resource pack")) {
        ("resourcepack", "resourcepacks")
    } else if cats.iter().any(|c| c.contains("shader")) {
        ("shader", "shaderpacks")
    } else if cats.iter().any(|c| c.contains("data pack")) {
        ("datapack", "datapacks")
    } else {
        ("mod", "mods")
    }
}

fn cf_fingerprint(data: &[u8]) -> u32 {
    const M: u32 = 0x5bd1_e995;
    const R: u32 = 24;
    let filtered: Vec<u8> = data
        .iter()
        .copied()
        .filter(|&b| b != 9 && b != 10 && b != 13 && b != 32)
        .collect();
    let len = filtered.len() as u32;
    let mut h: u32 = 1 ^ len;
    let mut chunks = filtered.chunks_exact(4);
    for c in chunks.by_ref() {
        let mut k = u32::from_le_bytes([c[0], c[1], c[2], c[3]]);
        k = k.wrapping_mul(M);
        k ^= k >> R;
        k = k.wrapping_mul(M);
        h = h.wrapping_mul(M);
        h ^= k;
    }
    let rem = chunks.remainder();
    match rem.len() {
        3 => {
            h ^= (rem[2] as u32) << 16;
            h ^= (rem[1] as u32) << 8;
            h ^= rem[0] as u32;
            h = h.wrapping_mul(M);
        }
        2 => {
            h ^= (rem[1] as u32) << 8;
            h ^= rem[0] as u32;
            h = h.wrapping_mul(M);
        }
        1 => {
            h ^= rem[0] as u32;
            h = h.wrapping_mul(M);
        }
        _ => {}
    }
    h ^= h >> 13;
    h = h.wrapping_mul(M);
    h ^= h >> 15;
    h
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfFingerprintMatch {
    file: CfFile,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfFingerprintData {
    #[serde(default)]
    exact_matches: Vec<CfFingerprintMatch>,
}

#[derive(Deserialize)]
struct CfFingerprintResponse {
    data: CfFingerprintData,
}

#[tauri::command]
pub async fn curseforge_match_local(instance_id: String) -> Result<usize, String> {
    let dir = paths::instance_game_dir(&instance_id).join("mods");
    let known: std::collections::HashSet<String> =
        read_content_index(&instance_id).items.iter().map(|i| i.filename.clone()).collect();

    let mut by_fp: HashMap<u64, String> = HashMap::new();
    let Ok(entries) = std::fs::read_dir(&dir) else { return Ok(0) };
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
            by_fp.insert(cf_fingerprint(&bytes) as u64, base);
        }
    }
    if by_fp.is_empty() {
        return Ok(0);
    }

    let fingerprints: Vec<u64> = by_fp.keys().copied().collect();
    let resp = send(
        http()
            .post(format!("{API}/fingerprints"))
            .json(&serde_json::json!({ "fingerprints": fingerprints })),
    )
    .await?;
    if !resp.status().is_success() {
        return Err(format!("fingerprints failed: {}", resp.status()));
    }
    let parsed: CfFingerprintResponse = resp.json().await.map_err(|e| e.to_string())?;
    if parsed.data.exact_matches.is_empty() {
        return Ok(0);
    }

    let mod_ids: Vec<i64> = parsed
        .data
        .exact_matches
        .iter()
        .map(|m| m.file.mod_id)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let mods = bulk_mods(&mod_ids).await.unwrap_or_default();

    let mut index = read_content_index(&instance_id);
    let mut count = 0usize;
    for m in &parsed.data.exact_matches {
        let file = &m.file;
        let Some(filename) = by_fp.get(&file.file_fingerprint) else { continue };
        let cf_mod = mods.get(&file.mod_id);
        let (kind, _folder) = cf_mod.map(kind_and_folder).unwrap_or(("mod", "mods"));
        let (game_versions, loaders) = split_game_versions(&file.game_versions);
        let item = InstalledItem {
            project_id: file.mod_id.to_string(),
            version_id: file.id.to_string(),
            kind: kind.to_string(),
            name: cf_mod.map(|m| m.name.clone()).unwrap_or_else(|| filename.clone()),
            filename: filename.clone(),
            version_number: if file.display_name.is_empty() { file.file_name.clone() } else { file.display_name.clone() },
            icon_url: cf_mod.and_then(|m| logo_url(&m.logo)),
            game_versions,
            loaders,
            dependency: false,
            dependencies: Vec::new(),
            installed_at: chrono::Utc::now().to_rfc3339(),
            provider: "curseforge".to_string(),
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

#[tauri::command]
pub async fn curseforge_match_file(instance_id: String, filename: String) -> Result<bool, String> {
    let dir = paths::instance_game_dir(&instance_id).join("mods");
    let path = if dir.join(&filename).is_file() {
        dir.join(&filename)
    } else {
        dir.join(format!("{filename}.disabled"))
    };
    let bytes = std::fs::read(&path).map_err(|e| format!("read {filename}: {e}"))?;
    let fp = cf_fingerprint(&bytes) as u64;

    let resp = send(
        http()
            .post(format!("{API}/fingerprints"))
            .json(&serde_json::json!({ "fingerprints": [fp] })),
    )
    .await?;
    if !resp.status().is_success() {
        return Ok(false);
    }
    let parsed: CfFingerprintResponse = resp.json().await.map_err(|e| e.to_string())?;
    let Some(m) = parsed.data.exact_matches.into_iter().next() else { return Ok(false) };
    let file = m.file;

    let mods = bulk_mods(&[file.mod_id]).await.unwrap_or_default();
    let cf_mod = mods.get(&file.mod_id);
    let (kind, _folder) = cf_mod.map(kind_and_folder).unwrap_or(("mod", "mods"));
    let (game_versions, loaders) = split_game_versions(&file.game_versions);

    let mut index = read_content_index(&instance_id);
    let item = InstalledItem {
        project_id: file.mod_id.to_string(),
        version_id: file.id.to_string(),
        kind: kind.to_string(),
        name: cf_mod.map(|m| m.name.clone()).unwrap_or_else(|| filename.clone()),
        filename: filename.clone(),
        version_number: if file.display_name.is_empty() { file.file_name.clone() } else { file.display_name.clone() },
        icon_url: cf_mod.and_then(|m| logo_url(&m.logo)),
        game_versions,
        loaders,
        dependency: false,
        dependencies: Vec::new(),
        installed_at: chrono::Utc::now().to_rfc3339(),
        provider: "curseforge".to_string(),
        author: None,
        env: None,
    };
    index.items.retain(|i| i.project_id != item.project_id && i.filename != item.filename);
    index.items.push(item);
    write_content_index(&instance_id, &index)?;
    Ok(true)
}

async fn bulk_mods(mod_ids: &[i64]) -> Result<HashMap<i64, CfMod>, String> {
    if mod_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let resp = send(
        http()
            .post(format!("{API}/mods"))
            .json(&serde_json::json!({ "modIds": mod_ids })),
    )
    .await?;
    if !resp.status().is_success() {
        return Err(format!("bulk mods failed: {}", resp.status()));
    }
    let parsed: CfListResponse<CfMod> = resp.json().await.map_err(|e| e.to_string())?;
    Ok(parsed.data.into_iter().map(|m| (m.id, m)).collect())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfManifest {
    #[serde(default)]
    name: String,
    minecraft: CfManifestMc,
    #[serde(default)]
    files: Vec<CfManifestFile>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfManifestMc {
    version: String,
    #[serde(default)]
    mod_loaders: Vec<CfManifestLoader>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfManifestLoader {
    id: String,
    #[serde(default)]
    primary: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfManifestFile {
    #[serde(rename = "fileID")]
    file_id: i64,
    #[serde(default)]
    required: bool,
}

#[derive(Clone, Serialize)]
struct ModpackProgress {
    instance_id: String,
    current: u64,
    total: u64,
    name: String,
}

fn loader_from_manifest(loaders: &[CfManifestLoader]) -> Loader {
    let primary = loaders.iter().find(|l| l.primary).or_else(|| loaders.first());
    let Some(l) = primary else { return Loader::Vanilla };
    let (kind, ver) = l.id.split_once('-').unwrap_or((l.id.as_str(), ""));
    let v = ver.to_string();
    match kind.to_lowercase().as_str() {
        "forge" => Loader::Forge(v),
        "fabric" => Loader::Fabric(v),
        "quilt" => Loader::Quilt(v),
        "neoforge" => Loader::NeoForge(v),
        _ => Loader::Vanilla,
    }
}

#[tauri::command]
pub async fn curseforge_import_modpack_file(
    app: AppHandle,
    path: String,
    name_override: Option<String>,
) -> Result<Instance, String> {
    let bytes = std::fs::read(&path).map_err(|e| format!("read file: {e}"))?;
    install_modpack_bytes(&app, bytes, name_override, None).await
}

#[tauri::command]
pub async fn curseforge_install_modpack(
    app: AppHandle,
    project_id: String,
    file_id: String,
    name_override: Option<String>,
) -> Result<Instance, String> {
    let file = fetch_file(&project_id, &file_id).await?;
    let url = file
        .download_url
        .filter(|u| !u.is_empty())
        .ok_or("this modpack file is not available for third-party download")?;
    let bytes = download(&url).await?;
    let cf_mod = fetch_mod(&project_id).await.ok();
    let icon_url = cf_mod.and_then(|m| logo_url(&m.logo));
    install_modpack_bytes(&app, bytes, name_override, icon_url).await
}

pub(crate) async fn install_modpack_bytes(
    app: &AppHandle,
    bytes: Vec<u8>,
    name_override: Option<String>,
    icon_url: Option<String>,
) -> Result<Instance, String> {
    let (manifest, raw_manifest) = parse_cf_manifest(&bytes)?;

    let mc_version = manifest.minecraft.version.clone();
    let loader = loader_from_manifest(&manifest.minecraft.mod_loaders);
    let name = name_override
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| manifest.name.clone());

    let mut instance = instances::create_instance(name, mc_version, loader, None, None)?;

    if let Some(url) = icon_url.filter(|u| !u.trim().is_empty()) {
        if let Ok(data) = download(&url).await {
            if std::fs::write(paths::instance_icon_file(&instance.id), &data).is_ok() {
                instance.icon = Some("icon.png".to_string());
                let _ = crate::store::write_json(&paths::instance_config_file(&instance.id), &instance);
            }
        }
    }

    let file_ids: Vec<i64> = manifest.files.iter().map(|f| f.file_id).collect();
    let resolved = bulk_files(&file_ids).await.unwrap_or_default();

    let mod_ids: Vec<i64> = resolved.values().map(|f| f.mod_id).collect::<std::collections::HashSet<_>>().into_iter().collect();
    let mods = bulk_mods(&mod_ids).await.unwrap_or_default();

    let game_dir = paths::instance_game_dir(&instance.id);
    let mods_dir = game_dir.join("mods");
    std::fs::create_dir_all(&mods_dir).map_err(|e| e.to_string())?;
    let mut blocked: Vec<BlockedMod> = Vec::new();

    struct Job {
        url: String,
        dest: PathBuf,
        item: InstalledItem,
    }
    let mut jobs: Vec<Job> = Vec::new();
    for f in &manifest.files {
        let Some(file) = resolved.get(&f.file_id) else { continue };
        let cf_mod = mods.get(&file.mod_id);
        let (kind, _folder) = cf_mod.map(kind_and_folder).unwrap_or(("mod", "mods"));

        let url = match &file.download_url {
            Some(u) if !u.is_empty() => u.clone(),
            _ => {
                blocked.push(BlockedMod {
                    name: cf_mod.map(|m| m.name.clone()).unwrap_or_else(|| file.file_name.clone()),
                    filename: safe_name(&file.file_name),
                    project_id: file.mod_id.to_string(),
                    file_id: file.id.to_string(),
                    url: blocked_url(cf_mod, file.mod_id, file.id),
                    fingerprint: file.file_fingerprint,
                });
                continue;
            }
        };
        let mut dest = mods_dir.join(safe_name(&file.file_name));
        if !f.required {
            dest.set_extension(format!("{}.disabled", dest.extension().and_then(|e| e.to_str()).unwrap_or("jar")));
        }
        let (game_versions, loaders) = split_game_versions(&file.game_versions);
        let item = InstalledItem {
            project_id: file.mod_id.to_string(),
            version_id: file.id.to_string(),
            kind: kind.to_string(),
            name: cf_mod.map(|m| m.name.clone()).unwrap_or_else(|| safe_name(&file.file_name)),
            filename: safe_name(&file.file_name),
            version_number: if file.display_name.is_empty() { file.file_name.clone() } else { file.display_name.clone() },
            icon_url: cf_mod.and_then(|m| logo_url(&m.logo)),
            game_versions,
            loaders,
            dependency: false,
            dependencies: Vec::new(),
            installed_at: chrono::Utc::now().to_rfc3339(),
            provider: "curseforge".to_string(),
            author: None,
            env: None,
        };
        jobs.push(Job { url, dest, item });
    }

    let total = jobs.len() as u64;
    let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(10));
    let done = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let mut set = tokio::task::JoinSet::new();
    for job in jobs {
        let sem = sem.clone();
        let done = done.clone();
        let app = app.clone();
        let id = instance.id.clone();
        let name = instance.name.clone();
        set.spawn(async move {
            let _permit = sem.acquire_owned().await.ok()?;
            let ok = match download(&job.url).await {
                Ok(data) => std::fs::write(&job.dest, &data).is_ok(),
                Err(e) => {
                    log::warn!("modpack file failed ({}): {e}", job.item.filename);
                    false
                }
            };
            let current = done.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            let _ = app.emit("modrinth://modpack-progress", ModpackProgress { instance_id: id, current, total, name });
            ok.then_some(job.item)
        });
    }
    let mut index = read_content_index(&instance.id);
    while let Some(res) = set.join_next().await {
        if let Ok(Some(item)) = res {
            index.items.push(item);
        }
    }

    let _ = write_content_index(&instance.id, &index);

    extract_overrides(&bytes, &game_dir)?;
    let _ = std::fs::write(paths::instance_dir(&instance.id).join("manifest.json"), raw_manifest);

    save_blocked_mods(&instance.id, &blocked);
    Ok(instance)
}

fn blocked_url(cf_mod: Option<&CfMod>, mod_id: i64, file_id: i64) -> String {
    if let Some(m) = cf_mod {
        if let Some(w) = m.links.as_ref().and_then(|l| l.website_url.clone()).filter(|s| !s.is_empty()) {
            return format!("{}/download/{}", w.trim_end_matches('/'), file_id);
        }
        if !m.slug.is_empty() {
            return format!("https://www.curseforge.com/minecraft/mc-mods/{}/download/{}", m.slug, file_id);
        }
    }
    format!("https://www.curseforge.com/projects/{mod_id}")
}

fn blocked_mods_file(id: &str) -> PathBuf {
    paths::instance_dir(id).join("blocked-mods.json")
}

fn save_blocked_mods(id: &str, blocked: &[BlockedMod]) {
    let path = blocked_mods_file(id);
    if blocked.is_empty() {
        let _ = std::fs::remove_file(&path);
        return;
    }
    if let Ok(json) = serde_json::to_vec_pretty(blocked) {
        let _ = std::fs::write(&path, json);
    }
}

#[tauri::command]
pub fn get_blocked_mods(instance_id: String) -> Vec<BlockedMod> {
    std::fs::read_to_string(blocked_mods_file(&instance_id))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

#[derive(Serialize)]
pub struct BlockedResolveResult {
    resolved: usize,
    remaining: Vec<BlockedMod>,
}

#[tauri::command]
pub fn resolve_blocked_mods(instance_id: String, dir: Option<String>) -> Result<BlockedResolveResult, String> {
    let mut blocked = get_blocked_mods(instance_id.clone());
    if blocked.is_empty() {
        return Ok(BlockedResolveResult { resolved: 0, remaining: vec![] });
    }
    let scan_dir = dir
        .map(PathBuf::from)
        .or_else(dirs::download_dir)
        .ok_or("no downloads folder found")?;

    let mods_dir = paths::instance_game_dir(&instance_id).join("mods");
    std::fs::create_dir_all(&mods_dir).map_err(|e| e.to_string())?;

    let mut resolved = 0usize;
    let mut index = read_content_index(&instance_id);
    let candidates = scan_candidate_files(&scan_dir, &blocked, 2);
    for path in candidates {
        if blocked.is_empty() {
            break;
        }
        let Ok(bytes) = std::fs::read(&path) else { continue };
        let fp = cf_fingerprint(&bytes) as u64;
        if let Some(pos) = blocked.iter().position(|b| b.fingerprint == fp) {
            let b = blocked.remove(pos);
            let _ = std::fs::write(mods_dir.join(safe_name(&b.filename)), &bytes);
            index.items.retain(|i| i.filename != b.filename && i.project_id != b.project_id);
            index.items.push(InstalledItem {
                project_id: b.project_id.clone(),
                version_id: b.file_id.clone(),
                kind: "mod".to_string(),
                name: b.name.clone(),
                filename: b.filename.clone(),
                version_number: b.name.clone(),
                icon_url: None,
                game_versions: vec![],
                loaders: vec![],
                dependency: false,
                dependencies: Vec::new(),
                installed_at: chrono::Utc::now().to_rfc3339(),
                provider: "curseforge".to_string(),
                author: None,
                env: None,
            });
            resolved += 1;
        }
    }

    if resolved > 0 {
        let _ = write_content_index(&instance_id, &index);
    }
    save_blocked_mods(&instance_id, &blocked);
    Ok(BlockedResolveResult { resolved, remaining: blocked })
}

#[tauri::command]
pub fn default_downloads_dir() -> Option<String> {
    dirs::download_dir().map(|p| p.to_string_lossy().into_owned())
}

fn scan_candidate_files(dir: &Path, blocked: &[BlockedMod], depth: u32) -> Vec<PathBuf> {
    let wanted: std::collections::HashSet<String> =
        blocked.iter().map(|b| normalize_name(&b.filename)).collect();
    let mut out = Vec::new();
    collect_candidates(dir, &wanted, depth, &mut out);
    out
}

fn collect_candidates(dir: &Path, wanted: &std::collections::HashSet<String>, depth: u32, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let path = e.path();
        if path.is_dir() {
            if depth > 0 {
                collect_candidates(&path, wanted, depth - 1, out);
            }
            continue;
        }
        let name = e.file_name().to_string_lossy().to_lowercase();
        if !(name.ends_with(".jar") || name.ends_with(".zip")) {
            continue;
        }
        if wanted.contains(&normalize_name(&e.file_name().to_string_lossy())) {
            out.push(path);
        }
    }
}

fn normalize_name(name: &str) -> String {
    name.to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect()
}

async fn bulk_files(file_ids: &[i64]) -> Result<HashMap<i64, CfFile>, String> {
    if file_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let resp = send(
        http()
            .post(format!("{API}/mods/files"))
            .json(&serde_json::json!({ "fileIds": file_ids })),
    )
    .await?;
    if !resp.status().is_success() {
        return Err(format!("CurseForge bulk files failed: {}", resp.status()));
    }
    let parsed: CfListResponse<CfFile> = resp.json().await.map_err(|e| e.to_string())?;
    Ok(parsed.data.into_iter().map(|f| (f.id, f)).collect())
}

fn parse_cf_manifest(bytes: &[u8]) -> Result<(CfManifest, String), String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("open zip: {e}"))?;
    let mut f = archive
        .by_name("manifest.json")
        .map_err(|_| "not a CurseForge modpack (no manifest.json)".to_string())?;
    let mut raw = String::new();
    f.read_to_string(&mut raw).map_err(|e| e.to_string())?;
    let manifest: CfManifest = serde_json::from_str(&raw).map_err(|e| format!("parse manifest: {e}"))?;
    Ok((manifest, raw))
}

fn extract_overrides(bytes: &[u8], game_dir: &Path) -> Result<(), String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        let Some(rel) = name.strip_prefix("overrides/").or_else(|| name.strip_prefix("client-overrides/")) else {
            continue;
        };
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

#[derive(Serialize)]
struct CfExportManifest {
    #[serde(rename = "manifestType")]
    manifest_type: &'static str,
    #[serde(rename = "manifestVersion")]
    manifest_version: u32,
    name: String,
    version: String,
    author: String,
    overrides: &'static str,
    minecraft: CfExportMc,
    files: Vec<CfExportFile>,
}

#[derive(Serialize)]
struct CfExportMc {
    version: String,
    #[serde(rename = "modLoaders")]
    mod_loaders: Vec<CfExportLoader>,
}

#[derive(Serialize)]
struct CfExportLoader {
    id: String,
    primary: bool,
}

#[derive(Serialize)]
struct CfExportFile {
    #[serde(rename = "projectID")]
    project_id: i64,
    #[serde(rename = "fileID")]
    file_id: i64,
    required: bool,
}

fn export_loader_id(loader: &Loader, mc: &str) -> Option<String> {
    match loader {
        Loader::Fabric(v) => Some(format!("fabric-{v}")),
        Loader::Quilt(v) => Some(format!("quilt-{v}")),
        Loader::Forge(v) => Some(format!("forge-{v}")),
        Loader::NeoForge(v) if mc == "1.20.1" => Some(format!("neoforge-1.20.1-{v}")),
        Loader::NeoForge(v) => Some(format!("neoforge-{v}")),
        Loader::Vanilla => None,
    }
}

#[tauri::command]
pub async fn export_curseforge(
    id: String,
    dest: String,
    version: Option<String>,
    author: Option<String>,
    exclude: Vec<String>,
    include: Vec<String>,
    optional_disabled: bool,
) -> Result<(), String> {
    let instance: Instance =
        crate::store::read_json(&paths::instance_config_file(&id))?.ok_or("instance not found")?;
    let game_dir = paths::instance_game_dir(&id);
    let filter = crate::commands::import::ExportFilter::new(include, exclude);

    struct Cand {
        rel: String,
        disabled: bool,
        fp: u64,
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
            if let Ok(bytes) = std::fs::read(&path) {
                cands.push(Cand { rel, disabled, fp: cf_fingerprint(&bytes) as u64 });
            }
        }
    }

    let mut files: Vec<CfExportFile> = Vec::new();
    let mut matched: std::collections::HashSet<String> = std::collections::HashSet::new();
    if !cands.is_empty() {
        let fingerprints: Vec<u64> = cands.iter().map(|c| c.fp).collect();
        let resp = send(
            http()
                .post(format!("{API}/fingerprints"))
                .json(&serde_json::json!({ "fingerprints": fingerprints })),
        )
        .await?;
        if resp.status().is_success() {
            if let Ok(parsed) = resp.json::<CfFingerprintResponse>().await {
                for m in parsed.data.exact_matches {
                    let file = m.file;
                    if !file.is_available {
                        continue;
                    }
                    let Some(cand) = cands.iter().find(|c| c.fp == file.file_fingerprint) else { continue };
                    files.push(CfExportFile {
                        project_id: file.mod_id,
                        file_id: file.id,
                        required: !(cand.disabled && optional_disabled),
                    });
                    matched.insert(cand.rel.clone());
                }
            }
        }
    }

    let mod_loaders = export_loader_id(&instance.loader, &instance.mc_version)
        .map(|id| vec![CfExportLoader { id, primary: true }])
        .unwrap_or_default();

    let manifest = CfExportManifest {
        manifest_type: "minecraftModpack",
        manifest_version: 1,
        name: instance.name.clone(),
        version: version.filter(|v| !v.trim().is_empty()).unwrap_or_else(|| "1.0.0".into()),
        author: author.unwrap_or_default(),
        overrides: "overrides",
        minecraft: CfExportMc { version: instance.mc_version.clone(), mod_loaders },
        files,
    };

    let out = std::fs::File::create(&dest).map_err(|e| format!("create {dest}: {e}"))?;
    let mut zip = zip::ZipWriter::new(out);
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let json = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
    zip.start_file("manifest.json", opts).map_err(|e| e.to_string())?;
    zip.write_all(&json).map_err(|e| e.to_string())?;

    cf_add_overrides(&mut zip, &game_dir, "", &filter, &matched, optional_disabled, opts)?;

    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}

fn cf_add_overrides(
    zip: &mut zip::ZipWriter<std::fs::File>,
    base: &Path,
    rel: &str,
    filter: &crate::commands::import::ExportFilter,
    matched: &std::collections::HashSet<String>,
    optional_disabled: bool,
    opts: zip::write::SimpleFileOptions,
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
                cf_add_overrides(zip, base, &child_rel, filter, matched, optional_disabled, opts)?;
            }
            continue;
        }
        let lower = name.to_lowercase();
        if !filter.includes(&child_rel) || matched.contains(&child_rel) || lower == ".ds_store" || lower == "thumbs.db" || lower.ends_with(".pw.toml") {
            continue;
        }
        if child_rel.ends_with(".disabled") && !optional_disabled {
            continue;
        }
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        zip.start_file(format!("overrides/{child_rel}"), opts).map_err(|e| e.to_string())?;
        zip.write_all(&bytes).map_err(|e| e.to_string())?;
    }
    Ok(())
}

async fn download(url: &str) -> Result<Vec<u8>, String> {
    crate::commands::modrinth::https_only(url)?;
    let resp = http().get(url).send().await.map_err(|e| e.to_string())?;
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
