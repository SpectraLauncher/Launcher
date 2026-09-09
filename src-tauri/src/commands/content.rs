use std::io::Read;
use std::path::Path;

use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::paths;
use crate::error::{AppError, AppResult};

const IMAGE_EXTS: [&str; 5] = ["png", "jpg", "jpeg", "webp", "gif"];

#[derive(Serialize)]
pub struct ScreenshotInfo {
    name: String,
    path: String,
    modified: u64,
}

#[tauri::command]
pub async fn list_screenshots(id: String) -> AppResult<Vec<ScreenshotInfo>> {
    crate::blocking(move || screenshots(&id)).await
}

fn screenshots(id: &str) -> AppResult<Vec<ScreenshotInfo>> {
    let dir = paths::instance_game_dir(id).join("screenshots");
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Ok(out),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() || !has_ext(&path, &IMAGE_EXTS) {
            continue;
        }
        out.push(ScreenshotInfo {
            name: file_name(&path),
            path: path.to_string_lossy().into_owned(),
            modified: modified_millis(&entry),
        });
    }
    out.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(out)
}

#[derive(Serialize)]
pub struct WorldInfo {
    folder: String,
    name: String,
    icon_path: Option<String>,
    last_played: Option<i64>,
    version: Option<String>,
    game_mode: Option<String>,
}

#[derive(Deserialize)]
struct LevelDat {
    #[serde(rename = "Data")]
    data: LevelData,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct LevelData {
    #[serde(rename = "LevelName")]
    level_name: Option<String>,
    #[serde(rename = "LastPlayed")]
    last_played: Option<i64>,
    #[serde(rename = "Version")]
    version: Option<LevelVersion>,
    #[serde(rename = "GameType")]
    game_type: Option<i32>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct LevelVersion {
    #[serde(rename = "Name")]
    name: Option<String>,
}

#[tauri::command]
pub async fn list_worlds(id: String) -> AppResult<Vec<WorldInfo>> {
    crate::blocking(move || worlds(&id)).await
}

fn worlds(id: &str) -> AppResult<Vec<WorldInfo>> {
    let dir = paths::instance_game_dir(id).join("saves");
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Ok(out),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let folder = file_name(&path);
        let data = read_level_dat(&path.join("level.dat"));
        let icon = path.join("icon.png");
        out.push(WorldInfo {
            name: data
                .as_ref()
                .and_then(|d| d.level_name.clone())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| folder.clone()),
            last_played: data.as_ref().and_then(|d| d.last_played),
            version: data.as_ref().and_then(|d| d.version.as_ref()).and_then(|v| v.name.clone()),
            game_mode: data.as_ref().and_then(|d| d.game_type).map(game_mode_name),
            icon_path: icon.is_file().then(|| icon.to_string_lossy().into_owned()),
            folder,
        });
    }
    out.sort_by(|a, b| b.last_played.cmp(&a.last_played));
    Ok(out)
}

fn read_level_dat(path: &Path) -> Option<LevelData> {
    let bytes = std::fs::read(path).ok()?;
    let mut decoder = flate2::read::GzDecoder::new(&bytes[..]);
    let mut raw = Vec::new();
    decoder.read_to_end(&mut raw).ok()?;
    fastnbt::from_bytes::<LevelDat>(&raw).ok().map(|l| l.data)
}

fn game_mode_name(t: i32) -> String {
    match t {
        0 => "survival",
        1 => "creative",
        2 => "adventure",
        3 => "spectator",
        _ => "unknown",
    }
    .to_string()
}

#[derive(Serialize)]
pub struct PackInfo {
    name: String,
    filename: String,
    description: Option<String>,
    pack_format: Option<i64>,
    icon_path: Option<String>,
    is_zip: bool,
    enabled: bool,
}

#[tauri::command]
pub async fn list_resource_packs(id: String) -> AppResult<Vec<PackInfo>> {
    crate::blocking(move || list_packs(&paths::instance_game_dir(&id).join("resourcepacks"))).await
}

#[tauri::command]
pub async fn list_data_packs(id: String) -> AppResult<Vec<PackInfo>> {
    crate::blocking(move || list_packs(&paths::instance_game_dir(&id).join("datapacks"))).await
}

fn list_packs(dir: &Path) -> AppResult<Vec<PackInfo>> {
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(out),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(pack) = read_pack(&path) {
            out.push(pack);
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

fn read_pack(path: &Path) -> Option<PackInfo> {
    let (base, enabled) = strip_disabled(&file_name(path));

    if path.is_dir() {
        let mcmeta = std::fs::read_to_string(path.join("pack.mcmeta")).ok();
        let icon = path.join("pack.png");
        let (description, pack_format) = parse_mcmeta(mcmeta.as_deref());
        Some(PackInfo {
            name: base.clone(),
            filename: base,
            description,
            pack_format,
            icon_path: icon.is_file().then(|| icon.to_string_lossy().into_owned()),
            is_zip: false,
            enabled,
        })
    } else if path.is_file() && base.to_lowercase().ends_with(".zip") {
        let (mcmeta, icon) = read_zip_pack_assets(path);
        let (description, pack_format) = parse_mcmeta(mcmeta.as_deref());
        Some(PackInfo {
            name: strip_zip(&base),
            filename: base,
            description,
            pack_format,
            icon_path: icon.and_then(|bytes| super::images::cache_icon(&bytes, "png")),
            is_zip: true,
            enabled,
        })
    } else {
        None
    }
}

fn read_zip_pack_assets(path: &Path) -> (Option<String>, Option<Vec<u8>>) {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return (None, None),
    };
    let mut archive = match zip::ZipArchive::new(file) {
        Ok(a) => a,
        Err(_) => return (None, None),
    };

    let mcmeta = archive.by_name("pack.mcmeta").ok().and_then(|mut f| {
        let mut s = String::new();
        f.read_to_string(&mut s).ok().map(|_| s)
    });
    let icon = archive.by_name("pack.png").ok().and_then(|mut f| {
        let mut b = Vec::new();
        f.read_to_end(&mut b).ok().map(|_| b)
    });
    (mcmeta, icon)
}

fn parse_mcmeta(text: Option<&str>) -> (Option<String>, Option<i64>) {
    let Some(text) = text else {
        return (None, None);
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else {
        return (None, None);
    };
    let pack = &v["pack"];
    let pack_format = pack["pack_format"].as_i64();
    let description = match &pack["description"] {
        serde_json::Value::Null => None,
        d => {
            let s = flatten_text(d);
            (!s.trim().is_empty()).then_some(s)
        }
    };
    (description, pack_format)
}

fn flatten_text(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(arr) => arr.iter().map(flatten_text).collect(),
        serde_json::Value::Object(obj) => {
            let mut s = obj.get("text").and_then(|t| t.as_str()).unwrap_or("").to_string();
            if let Some(extra) = obj.get("extra") {
                s.push_str(&flatten_text(extra));
            }
            s
        }
        _ => String::new(),
    }
}

#[derive(Serialize)]
pub struct ServerInfo {
    name: String,
    ip: String,
    icon_path: Option<String>,
    hidden: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ServersFile {
    servers: Vec<ServerEntry>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ServerEntry {
    name: Option<String>,
    ip: Option<String>,
    icon: Option<String>,
    hidden: Option<i8>,
}

#[tauri::command]
pub async fn list_servers(id: String) -> AppResult<Vec<ServerInfo>> {
    crate::blocking(move || servers(&id)).await
}

fn servers(id: &str) -> AppResult<Vec<ServerInfo>> {
    let path = paths::instance_game_dir(id).join("servers.dat");
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(_) => return Ok(Vec::new()),
    };
    let parsed: ServersFile = fastnbt::from_bytes(&bytes).map_err(|e| format!("parse servers.dat: {e}"))?;
    Ok(parsed
        .servers
        .into_iter()
        .map(|s| ServerInfo {
            name: s.name.unwrap_or_default(),
            ip: s.ip.unwrap_or_default(),
            icon_path: s
                .icon
                .and_then(|b64| base64::engine::general_purpose::STANDARD.decode(b64).ok())
                .and_then(|bytes| super::images::cache_icon(&bytes, "png")),
            hidden: s.hidden.unwrap_or(0) != 0,
        })
        .collect())
}

#[tauri::command]
pub async fn add_server(id: String, name: String, ip: String) -> AppResult<()> {
    crate::blocking(move || push_server(&id, name, ip)).await
}

fn push_server(id: &str, name: String, ip: String) -> AppResult<()> {
    use fastnbt::Value;
    use std::collections::HashMap;

    let game_dir = paths::instance_game_dir(id);
    let path = game_dir.join("servers.dat");

    let mut root: Value = match std::fs::read(&path) {
        Ok(bytes) => fastnbt::from_bytes(&bytes).map_err(|e| format!("parse servers.dat: {e}"))?,
        Err(_) => Value::Compound(HashMap::new()),
    };

    let Value::Compound(map) = &mut root else { return Err("invalid servers.dat".into()) };
    let servers = map.entry("servers".to_string()).or_insert_with(|| Value::List(Vec::new()));
    let Value::List(list) = servers else { return Err("invalid servers list".into()) };

    let mut entry = HashMap::new();
    entry.insert("name".to_string(), Value::String(name));
    entry.insert("ip".to_string(), Value::String(ip));
    list.push(Value::Compound(entry));

    std::fs::create_dir_all(&game_dir).map_err(|e| e.to_string())?;
    let bytes = fastnbt::to_bytes(&root).map_err(|e| format!("encode servers.dat: {e}"))?;
    (std::fs::write(&path, bytes).map_err(|e| format!("write servers.dat: {e}"))).map_err(Into::into)
}

#[tauri::command]
pub async fn delete_server(id: String, index: usize) -> AppResult<()> {
    crate::blocking(move || drop_server(&id, index)).await
}

fn drop_server(id: &str, index: usize) -> AppResult<()> {
    use fastnbt::Value;

    let path = paths::instance_game_dir(id).join("servers.dat");
    let bytes = std::fs::read(&path).map_err(|e| format!("read servers.dat: {e}"))?;
    let mut root: Value = fastnbt::from_bytes(&bytes).map_err(|e| format!("parse servers.dat: {e}"))?;

    let Value::Compound(map) = &mut root else { return Err("invalid servers.dat".into()) };
    let Some(Value::List(list)) = map.get_mut("servers") else { return Ok(()) };
    if index < list.len() {
        list.remove(index);
    }

    let out = fastnbt::to_bytes(&root).map_err(|e| format!("encode servers.dat: {e}"))?;
    (std::fs::write(&path, out).map_err(|e| format!("write servers.dat: {e}"))).map_err(Into::into)
}

#[derive(Serialize)]
pub struct ShaderInfo {
    name: String,
    filename: String,
    is_zip: bool,
    enabled: bool,
}

#[tauri::command]
pub async fn list_shaders(id: String) -> AppResult<Vec<ShaderInfo>> {
    crate::blocking(move || shaders(&id)).await
}

fn shaders(id: &str) -> AppResult<Vec<ShaderInfo>> {
    let dir = paths::instance_game_dir(id).join("shaderpacks");
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Ok(out),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let (base, enabled) = strip_disabled(&file_name(&path));
        if path.is_dir() {
            out.push(ShaderInfo { name: base.clone(), filename: base, is_zip: false, enabled });
        } else if path.is_file() && base.to_lowercase().ends_with(".zip") {
            out.push(ShaderInfo { name: strip_zip(&base), filename: base, is_zip: true, enabled });
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

fn content_folder(kind: &str) -> AppResult<&'static str> {
    match kind {
        "resourcepack" => Ok("resourcepacks"),
        "shader" => Ok("shaderpacks"),
        "datapack" => Ok("datapacks"),
        other => Err(AppError::invalid(format!("unknown content kind: {other}"))),
    }
}

#[tauri::command]
pub async fn delete_content(id: String, kind: String, filename: String) -> AppResult<()> {
    crate::blocking(move || remove_content(&id, &kind, &filename)).await
}

fn remove_content(id: &str, kind: &str, filename: &str) -> AppResult<()> {
    let folder = content_folder(kind)?;
    let safe = safe_name(filename)?;
    let base = paths::instance_game_dir(id).join(folder);
    for target in [base.join(&safe), base.join(format!("{safe}.disabled"))] {
        if target.is_dir() {
            std::fs::remove_dir_all(&target).map_err(|e| format!("delete: {e}"))?;
        } else if target.exists() {
            std::fs::remove_file(&target).map_err(|e| format!("delete: {e}"))?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn set_content_enabled(id: String, kind: String, filename: String, enabled: bool) -> AppResult<()> {
    let folder = content_folder(&kind)?;
    let safe = safe_name(&filename)?;
    let base = paths::instance_game_dir(&id).join(folder);
    let on = base.join(&safe);
    let off = base.join(format!("{safe}.disabled"));
    if enabled {
        if off.exists() {
            std::fs::rename(&off, &on).map_err(|e| e.to_string())?;
        }
    } else if on.exists() {
        std::fs::rename(&on, &off).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn delete_world(id: String, folder: String) -> AppResult<()> {
    crate::blocking(move || {
        let safe = safe_name(&folder)?;
        let target = paths::instance_game_dir(&id).join("saves").join(&safe);
        if target.is_dir() {
            (std::fs::remove_dir_all(&target).map_err(|e| format!("delete: {e}"))).map_err(Into::into)
        } else {
            Ok(())
        }
    })
    .await
}

#[tauri::command]
pub async fn backup_world(id: String, folder: String, dest: String) -> AppResult<()> {
    crate::blocking(move || {
        let safe = safe_name(&folder)?;
        let src = paths::instance_game_dir(&id).join("saves").join(&safe);
        if !src.is_dir() {
            return Err("world not found".into());
        }
        (zip_dir(&src, &safe, Path::new(&dest)).map_err(|e| format!("backup: {e}"))).map_err(Into::into)
    })
    .await
}

#[tauri::command]
pub fn delete_screenshot(id: String, name: String) -> AppResult<()> {
    let safe = safe_name(&name)?;
    let target = paths::instance_game_dir(&id).join("screenshots").join(&safe);
    if !has_ext(&target, &IMAGE_EXTS) {
        return Err("not an image".into());
    }
    if target.is_file() {
        (std::fs::remove_file(&target).map_err(|e| format!("delete: {e}"))).map_err(Into::into)
    } else {
        Ok(())
    }
}

#[derive(Serialize)]
pub struct LogFile {
    name: String,
    kind: String,
    rel: String,
    modified: u64,
    size: u64,
}

#[tauri::command]
pub async fn list_log_files(id: String) -> AppResult<Vec<LogFile>> {
    crate::blocking(move || log_files(&id)).await
}

fn log_files(id: &str) -> AppResult<Vec<LogFile>> {
    let game = paths::instance_game_dir(id);
    let mut out = Vec::new();

    if let Ok(entries) = std::fs::read_dir(game.join("logs")) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = file_name(&path);
            let lower = name.to_lowercase();
            let kind = if lower == "latest.log" {
                "latest"
            } else if lower.ends_with(".gz") {
                "archived"
            } else if lower.ends_with(".log") {
                "log"
            } else {
                continue;
            };
            out.push(LogFile {
                rel: format!("logs/{name}"),
                name,
                kind: kind.into(),
                modified: modified_millis(&entry),
                size: entry.metadata().map(|m| m.len()).unwrap_or(0),
            });
        }
    }

    if let Ok(entries) = std::fs::read_dir(game.join("crash-reports")) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && has_ext(&path, &["txt", "log"]) {
                let name = file_name(&path);
                out.push(LogFile {
                    rel: format!("crash-reports/{name}"),
                    name,
                    kind: "crash".into(),
                    modified: modified_millis(&entry),
                    size: entry.metadata().map(|m| m.len()).unwrap_or(0),
                });
            }
        }
    }

    out.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(out)
}

#[tauri::command]
pub async fn read_log_file(id: String, rel: String) -> AppResult<String> {
    crate::blocking(move || {
        let text = read_log_text(&id, &rel)?;
        const MAX: usize = 1_000_000;
        Ok(match text.char_indices().nth_back(MAX) {
            Some((cut, _)) => text[cut..].to_string(),
            None => text,
        })
    })
    .await
}

fn read_log_text(id: &str, rel: &str) -> AppResult<String> {
    if (!rel.starts_with("logs/") && !rel.starts_with("crash-reports/")) || rel.contains("..") {
        return Err("invalid log path".into());
    }
    let path = paths::instance_game_dir(id).join(rel);
    let bytes = std::fs::read(&path).map_err(|e| format!("read log: {e}"))?;

    if rel.ends_with(".gz") {
        let mut decoder = flate2::read::GzDecoder::new(&bytes[..]);
        let mut s = String::new();
        decoder.read_to_string(&mut s).map_err(|e| format!("gunzip: {e}"))?;
        Ok(s)
    } else {
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
}

fn censor(text: &str) -> String {
    redact_tokens(&redact_home(text))
}

fn redact_home(text: &str) -> String {
    let Some(home) = dirs::home_dir() else { return text.to_string() };
    let home = home.to_string_lossy().into_owned();
    if home.len() < 4 {
        return text.to_string();
    }

    let mut forms = vec![
        home.replace('\\', "\\\\"),
        home.replace('/', "\\"),
        home.replace('\\', "/"),
        home,
    ];
    forms.sort_by_key(|f| std::cmp::Reverse(f.len()));
    forms.dedup();

    let mut out = text.to_string();
    for form in forms {
        out = replace_ascii_ignore_case(&out, &form, "~");
    }
    out
}

fn replace_ascii_ignore_case(haystack: &str, needle: &str, with: &str) -> String {
    if needle.is_empty() {
        return haystack.to_string();
    }
    let folded = haystack.to_ascii_lowercase();
    let needle = needle.to_ascii_lowercase();

    let mut out = String::with_capacity(haystack.len());
    let mut at = 0;
    while let Some(hit) = folded[at..].find(&needle) {
        let start = at + hit;
        out.push_str(&haystack[at..start]);
        out.push_str(with);
        at = start + needle.len();
    }
    out.push_str(&haystack[at..]);
    out
}

const SECRET_KEYS: [&str; 6] = [
    "accesstoken",
    "access_token",
    "sessionid",
    "session_id",
    "--uuid",
    "refresh_token",
];

fn redact_tokens(text: &str) -> String {
    let folded = text.to_ascii_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut at = 0;

    while at < text.len() {
        let hit = SECRET_KEYS
            .iter()
            .filter_map(|key| folded[at..].find(key).map(|i| (at + i, key.len())))
            .min_by_key(|(start, _)| *start);

        let Some((start, key_len)) = hit else { break };
        let after_key = start + key_len;
        out.push_str(&text[at..after_key]);

        let sep_end = after_key
            + text[after_key..]
                .find(|c: char| !matches!(c, ' ' | '=' | ':' | '"' | '\'' | '\t'))
                .unwrap_or(text.len() - after_key);
        let value_end = sep_end
            + text[sep_end..]
                .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')))
                .unwrap_or(text.len() - sep_end);

        if value_end - sep_end >= 16 {
            out.push_str(&text[after_key..sep_end]);
            out.push_str("<redacted>");
            at = value_end;
        } else {
            at = after_key;
        }
    }

    out.push_str(&text[at..]);
    out
}

#[derive(Serialize)]
pub struct MclogsPaste {
    pub id: String,
    pub url: String,
    pub raw: String,
}

#[derive(Deserialize)]
struct MclogsResponse {
    success: bool,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    raw: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

#[tauri::command]
pub async fn upload_log_to_mclogs(id: String, rel: String) -> AppResult<MclogsPaste> {
    let id_for_read = id.clone();
    let rel_for_read = rel.clone();
    let mut content =
        crate::blocking(move || read_log_text(&id_for_read, &rel_for_read)).await?;

    const MAX_LINES: usize = 25_000;
    const MAX_BYTES: usize = 10 * 1024 * 1024;
    let line_count = content.lines().count();
    if line_count > MAX_LINES {
        let skip = line_count - MAX_LINES;
        content = content.lines().skip(skip).collect::<Vec<_>>().join("\n");
    }
    if content.len() > MAX_BYTES {
        let cut = (content.len() - MAX_BYTES..content.len())
            .find(|i| content.is_char_boundary(*i))
            .unwrap_or(content.len());
        content = content[cut..].to_string();
    }

    let content = crate::blocking(move || Ok(censor(&content))).await?;

    let resp = crate::http()
        .post("https://api.mclo.gs/1/log")
        .form(&[("content", content.as_str()), ("source", "SpectraLauncher")])
        .send()
        .await
        .map_err(|e| format!("upload failed: {e}"))?;

    let body: MclogsResponse = resp.json().await.map_err(|e| format!("bad response: {e}"))?;
    if !body.success {
        return Err((body.error.unwrap_or_else(|| "mclo.gs rejected the log".into())).into());
    }
    Ok(MclogsPaste {
        id: body.id.unwrap_or_default(),
        url: body.url.unwrap_or_default(),
        raw: body.raw.unwrap_or_default(),
    })
}

fn file_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

fn strip_zip(name: &str) -> String {
    name.strip_suffix(".zip").unwrap_or(name).to_string()
}

fn strip_disabled(name: &str) -> (String, bool) {
    match name.strip_suffix(".disabled") {
        Some(base) => (base.to_string(), false),
        None => (name.to_string(), true),
    }
}

fn safe_name(name: &str) -> AppResult<String> {
    Path::new(name)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|n| !n.is_empty())
        .ok_or_else(|| "invalid name".into())
}

fn zip_dir(src: &Path, root_name: &str, dest: &Path) -> std::io::Result<()> {
    use std::io::Write;
    let file = std::fs::File::create(dest)?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    fn walk<W: std::io::Write + std::io::Seek>(
        zip: &mut zip::ZipWriter<W>,
        dir: &Path,
        prefix: &str,
        opts: zip::write::SimpleFileOptions,
    ) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let rel = format!("{prefix}/{name}");
            if path.is_dir() {
                walk(zip, &path, &rel, opts)?;
            } else if path.is_file() {
                zip.start_file(&rel, opts)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
                let bytes = std::fs::read(&path)?;
                zip.write_all(&bytes)?;
            }
        }
        Ok(())
    }

    walk(&mut zip, src, root_name, opts)?;
    zip.finish().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    Ok(())
}

fn has_ext(path: &Path, exts: &[&str]) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| exts.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

fn modified_millis(entry: &std::fs::DirEntry) -> u64 {
    entry
        .metadata()
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}


#[cfg(test)]
mod censor_tests {
    use super::{censor, redact_tokens, replace_ascii_ignore_case};

    #[test]
    fn strips_the_home_directory_whatever_the_separator() {
        let home = dirs::home_dir().unwrap().to_string_lossy().into_owned();
        let slashed = home.replace('\\', "/");
        let escaped = home.replace('\\', "\\\\");

        let log = format!("loading {home}/mods/a.jar and {slashed}/x and {escaped}/y");
        let out = censor(&log);

        assert!(!out.contains(&home), "plain form survived: {out}");
        assert!(!out.contains(&slashed), "slash form survived: {out}");
        assert!(!out.contains(&escaped), "escaped form survived: {out}");
        assert!(out.contains("~/mods/a.jar"), "path was mangled: {out}");
    }

    #[test]
    fn strips_secrets_but_leaves_short_values_alone() {
        let out = redact_tokens("--accessToken eyJhbGciOiJIUzI1NiJ9.abc --width 1280");
        assert_eq!(out, "--accessToken <redacted> --width 1280");

        let out = redact_tokens(r#"{"access_token":"ey.aaaaaaaaaaaaaaaaaa","x":1}"#);
        assert_eq!(out, r#"{"access_token":"<redacted>","x":1}"#);

        assert_eq!(redact_tokens("sessionId: none"), "sessionId: none");
        assert_eq!(redact_tokens("no secrets here"), "no secrets here");
    }

    #[test]
    fn case_insensitive_replace_keeps_non_ascii_intact() {
        assert_eq!(
            replace_ascii_ignore_case("C:\\Users\\Paweł\\x", "c:\\users\\paweł", "~"),
            "~\\x",
            "an ASCII-only fold must still match the ASCII part and slice cleanly"
        );
        assert_eq!(replace_ascii_ignore_case("abc", "", "~"), "abc");
    }
}
