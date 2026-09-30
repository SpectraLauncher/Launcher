use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde_json::{json, Map, Value};
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::commands::addons::{self, Manifest};
use crate::error::{AppError, AppResult};
use crate::models::Instance;
use crate::{paths, store, AppState};

const SDK: &str = include_str!("addon_sdk.js");
const SDK_PATH: &str = "__spectra/sdk.js";
const MAIN_PATH: &str = "__spectra/main.html";
const STORAGE_LIMIT: usize = 5 * 1024 * 1024;
const FETCH_LIMIT: usize = 10 * 1024 * 1024;
const FETCH_BODY_LIMIT: usize = 1024 * 1024;
const CONTENT_KINDS: &[&str] = &["mod", "resourcepack", "shader", "datapack"];
const FORBIDDEN_HEADERS: &[&str] = &[
    "host",
    "cookie",
    "origin",
    "referer",
    "content-length",
    "connection",
    "transfer-encoding",
    "user-agent",
];

pub const HOST_METHODS: &[&str] = &[
    "instances.launch",
    "ui.toast",
    "ui.navigate",
    "ui.openUrl",
    "ui.confirm",
    "launcher.locale",
    "launcher.theme",
];

static STORAGE_LOCK: Mutex<()> = Mutex::new(());
static PORT: OnceLock<u16> = OnceLock::new();

const MAX_REQUEST_HEAD: usize = 16 * 1024;

pub fn origin() -> String {
    format!("http://127.0.0.1:{}", PORT.get().copied().unwrap_or(0))
}

pub fn start_server() -> std::io::Result<u16> {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0))?;
    listener.set_nonblocking(true)?;
    let port = listener.local_addr()?.port();
    if PORT.set(port).is_err() {
        return Ok(*PORT.get().unwrap_or(&port));
    }
    tauri::async_runtime::spawn(async move {
        let listener = match tokio::net::TcpListener::from_std(listener) {
            Ok(listener) => listener,
            Err(e) => {
                log::error!("addon file server could not start: {e}");
                return;
            }
        };
        loop {
            if let Ok((stream, _)) = listener.accept().await {
                tauri::async_runtime::spawn(handle_connection(stream, port));
            }
        }
    });
    Ok(port)
}

async fn handle_connection(mut stream: tokio::net::TcpStream, port: u16) {
    let mut head = Vec::with_capacity(1024);
    let mut chunk = [0u8; 2048];
    while !head.windows(4).any(|w| w == b"\r\n\r\n") {
        let read = tokio::time::timeout(Duration::from_secs(5), stream.read(&mut chunk)).await;
        match read {
            Ok(Ok(n)) if n > 0 => head.extend_from_slice(&chunk[..n]),
            _ => return,
        }
        if head.len() > MAX_REQUEST_HEAD {
            return;
        }
    }
    let request = String::from_utf8_lossy(&head).into_owned();
    let bytes = tauri::async_runtime::spawn_blocking(move || answer(&request, port)).await.unwrap_or_default();
    let _ = stream.write_all(&bytes).await;
    let _ = stream.shutdown().await;
}

pub fn answer(request: &str, port: u16) -> Vec<u8> {
    let mut lines = request.split("\r\n");
    let mut first = lines.next().unwrap_or_default().split(' ');
    let method = first.next().unwrap_or_default();
    let target = first.next().unwrap_or_default();
    let host = lines
        .take_while(|line| !line.is_empty())
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.trim().eq_ignore_ascii_case("host").then(|| value.trim().to_string())
        })
        .unwrap_or_default();

    let served = if host != format!("127.0.0.1:{port}") {
        Served { status: 421, mime: "text/plain; charset=utf-8", body: b"wrong host".to_vec(), csp: None }
    } else if method != "GET" && method != "HEAD" {
        Served { status: 405, mime: "text/plain; charset=utf-8", body: b"method not allowed".to_vec(), csp: None }
    } else {
        serve(target.split('?').next().unwrap_or_default())
    };
    encode(served, method == "HEAD")
}

fn encode(served: Served, head_only: bool) -> Vec<u8> {
    let reason = match served.status {
        200 => "OK",
        404 => "Not Found",
        405 => "Method Not Allowed",
        421 => "Misdirected Request",
        _ => "Error",
    };
    let mut out = format!(
        "HTTP/1.1 {} {reason}\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\n\
         Cross-Origin-Resource-Policy: cross-origin\r\nX-Content-Type-Options: nosniff\r\nCache-Control: no-store\r\n\
         Connection: close\r\n",
        served.status,
        served.mime,
        served.body.len(),
    );
    if let Some(policy) = served.csp {
        out.push_str(&format!("Content-Security-Policy: {policy}\r\n"));
    }
    out.push_str("\r\n");
    let mut bytes = out.into_bytes();
    if !head_only {
        bytes.extend_from_slice(&served.body);
    }
    bytes
}

#[derive(Debug)]
pub struct Served {
    pub status: u16,
    pub mime: &'static str,
    pub body: Vec<u8>,
    pub csp: Option<String>,
}

fn not_found() -> Served {
    Served { status: 404, mime: "text/plain; charset=utf-8", body: b"not found".to_vec(), csp: None }
}

fn percent_decode(raw: &str) -> Option<String> {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = raw.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn encode_segment(part: &str) -> String {
    part.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

pub fn mime_for(name: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();
    match lower.rsplit('.').next().unwrap_or("") {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        "wasm" => "application/wasm",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

pub fn csp(id: &str) -> String {
    let own = format!("{}/{id}/", origin());
    let sdk = format!("{}/__spectra/", origin());
    format!(
        "default-src 'none'; script-src 'unsafe-inline' {own} {sdk}; style-src 'unsafe-inline' {own}; \
         img-src {own} data: blob:; font-src {own} data:; media-src {own} data: blob:; connect-src {own}; \
         frame-src 'none'; worker-src 'none'; object-src 'none'; form-action 'none'; base-uri 'none'"
    )
}

pub fn inject_sdk(html: &str) -> String {
    let tag = format!("<script src=\"/{SDK_PATH}\"></script>");
    let trimmed = html.trim_start();
    let has_doctype = trimmed.get(..9).is_some_and(|head| head.eq_ignore_ascii_case("<!doctype"));
    if has_doctype {
        if let Some(end) = trimmed.find('>') {
            return format!("{}{tag}{}", &trimmed[..=end], &trimmed[end + 1..]);
        }
    }
    format!("{tag}{html}")
}

fn main_document(id: &str, main: &str) -> String {
    let kind = if main.ends_with(".mjs") { " type=\"module\"" } else { "" };
    let src = main.trim_start_matches("./").split('/').map(encode_segment).collect::<Vec<_>>().join("/");
    format!("<!doctype html><meta charset=\"utf-8\"><script src=\"/{SDK_PATH}\"></script><script{kind} src=\"/{id}/{src}\"></script>")
}

pub fn serve(raw_path: &str) -> Served {
    let Some(path) = percent_decode(raw_path.trim_start_matches('/')) else {
        return not_found();
    };
    if path == SDK_PATH {
        return Served {
            status: 200,
            mime: "text/javascript; charset=utf-8",
            body: SDK.as_bytes().to_vec(),
            csp: None,
        };
    }
    let Some((id, rest)) = path.split_once('/') else {
        return not_found();
    };
    if !addons::is_enabled(id) {
        return not_found();
    }
    if rest == MAIN_PATH {
        return match addons::active_manifest(id).ok().and_then(|m| m.main) {
            Some(main) => Served {
                status: 200,
                mime: "text/html; charset=utf-8",
                body: main_document(id, &main).into_bytes(),
                csp: Some(csp(id)),
            },
            None => not_found(),
        };
    }
    if !addons::safe_name(rest) || rest.starts_with("__spectra/") {
        return not_found();
    }
    let file = rest.split('/').fold(paths::addons_dir().join(id), |path, part| path.join(part));
    let Ok(bytes) = std::fs::read(&file) else {
        return not_found();
    };
    let mime = mime_for(rest);
    if mime.starts_with("text/html") {
        let html = inject_sdk(&String::from_utf8_lossy(&bytes));
        return Served { status: 200, mime, body: html.into_bytes(), csp: Some(csp(id)) };
    }
    Served { status: 200, mime, body: bytes, csp: None }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Needs {
    Nothing,
    Permission(&'static str),
    Network,
}

pub fn requirement(method: &str) -> Option<Needs> {
    Some(match method {
        "instances.list" | "instances.get" | "instances.content" | "instances.worlds" | "instances.isRunning" => {
            Needs::Permission("instances:read")
        }
        "instances.update" | "instances.setContentEnabled" => Needs::Permission("instances:write"),
        "instances.launch" | "instances.stop" => Needs::Permission("instances:launch"),
        "logs.console" | "logs.list" | "logs.read" => Needs::Permission("logs:read"),
        "servers.ping" => Needs::Permission("servers:ping"),
        "account.minecraft" | "account.spectra" => Needs::Permission("account:read"),
        "skins.list" => Needs::Permission("skins:read"),
        "http.fetch" => Needs::Network,
        "storage.get" | "storage.set" | "storage.remove" | "storage.keys" | "ui.toast" | "ui.navigate"
        | "ui.openWindow" | "ui.openUrl" | "ui.confirm" | "launcher.version" | "launcher.locale"
        | "launcher.theme" => Needs::Nothing,
        _ => return None,
    })
}

fn denied(message: impl Into<String>) -> AppError {
    AppError::new("permission_denied", message)
}

pub fn authorize(manifest: &Manifest, method: &str) -> AppResult<()> {
    match requirement(method) {
        None => Err(AppError::new("unknown_method", format!("{method} is not part of the addon API"))),
        Some(Needs::Permission(permission)) if !manifest.permissions.iter().any(|p| p == permission) => {
            Err(denied(format!("{method} needs the {permission} permission")))
        }
        _ => Ok(()),
    }
}

fn text(params: &Value, key: &str, max: usize) -> AppResult<String> {
    let value = params
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| AppError::invalid(format!("{key} is required")))?;
    if value.chars().count() > max {
        return Err(AppError::invalid(format!("{key} is too long")));
    }
    Ok(value.to_string())
}

fn https_url(raw: &str) -> Result<reqwest::Url, String> {
    let url = reqwest::Url::parse(raw).map_err(|_| "not an address".to_string())?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() || url.host_str().is_none() {
        return Err("only plain https addresses are allowed".into());
    }
    Ok(url)
}

pub fn fetch_target(permissions: &[String], raw: &str) -> Result<reqwest::Url, String> {
    let url = https_url(raw)?;
    let host = url.host_str().unwrap_or_default();
    if !host_allowed(permissions, host) {
        return Err(format!("add network:{host} to the addon's permissions"));
    }
    Ok(url)
}

fn host_allowed(permissions: &[String], host: &str) -> bool {
    permissions.iter().any(|p| p.strip_prefix("network:") == Some(host))
}

fn public_instance(instance: &Instance) -> Value {
    json!({
        "id": instance.id,
        "name": instance.name,
        "mcVersion": instance.mc_version,
        "loader": instance.loader,
        "group": instance.group,
        "memoryMb": instance.memory_mb,
        "createdAt": instance.created_at,
        "lastPlayed": instance.last_played,
        "playtimeSeconds": instance.playtime_seconds,
    })
}

fn without(value: Value, keys: &[&str]) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.into_iter().map(|v| without(v, keys)).collect()),
        Value::Object(mut map) => {
            for key in keys {
                map.remove(*key);
            }
            Value::Object(map)
        }
        other => other,
    }
}

async fn instance_id(params: &Value) -> AppResult<String> {
    let id = text(params, "id", 100)?;
    let known = crate::commands::instances::list_instances().await?.into_iter().any(|i| i.id == id);
    if !known {
        return Err(AppError::not_found("no such instance"));
    }
    Ok(id)
}

fn storage_file(id: &str) -> std::path::PathBuf {
    paths::addons_dir().join(".storage").join(format!("{id}.json"))
}

pub fn forget_storage(id: &str) {
    if addons::valid_id(id) {
        let _ = std::fs::remove_file(storage_file(id));
    }
}

pub fn storage(id: &str, method: &str, params: &Value) -> AppResult<Value> {
    let _guard = STORAGE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let file = storage_file(id);
    let mut data: Map<String, Value> = store::read_json(&file)?.unwrap_or_default();
    match method {
        "storage.keys" => Ok(json!(data.keys().collect::<Vec<_>>())),
        "storage.get" => Ok(data.get(&text(params, "key", 200)?).cloned().unwrap_or(Value::Null)),
        "storage.remove" => {
            data.remove(&text(params, "key", 200)?);
            store::write_json(&file, &data)?;
            Ok(Value::Null)
        }
        "storage.set" => {
            let key = text(params, "key", 200)?;
            data.insert(key, params.get("value").cloned().unwrap_or(Value::Null));
            if serde_json::to_vec(&data)?.len() > STORAGE_LIMIT {
                return Err(AppError::invalid("the addon's storage is limited to 5 MB"));
            }
            store::write_json(&file, &data)?;
            Ok(Value::Null)
        }
        _ => Err(AppError::new("unknown_method", method.to_string())),
    }
}

async fn fetch(manifest: &Manifest, params: &Value) -> AppResult<Value> {
    let url = fetch_target(&manifest.permissions, &text(params, "url", 2000)?).map_err(denied)?;
    let method = params.get("method").and_then(Value::as_str).unwrap_or("GET").to_ascii_uppercase();
    if !["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD"].contains(&method.as_str()) {
        return Err(AppError::invalid(format!("{method} is not an allowed method")));
    }
    let hosts: Vec<String> = manifest
        .permissions
        .iter()
        .filter_map(|p| p.strip_prefix("network:").map(str::to_string))
        .collect();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(format!("Spectra-Launcher/{} (addon {})", env!("CARGO_PKG_VERSION"), manifest.id))
        .redirect(reqwest::redirect::Policy::custom(move |attempt| {
            let allowed = attempt.url().scheme() == "https"
                && attempt.url().host_str().is_some_and(|h| hosts.iter().any(|x| x == h));
            if attempt.previous().len() < 5 && allowed {
                attempt.follow()
            } else {
                attempt.stop()
            }
        }))
        .build()?;

    let mut request = client.request(reqwest::Method::from_bytes(method.as_bytes()).map_err(|e| e.to_string())?, url);
    if let Some(headers) = params.get("headers").and_then(Value::as_object) {
        for (name, value) in headers {
            let Some(value) = value.as_str() else { continue };
            if FORBIDDEN_HEADERS.contains(&name.to_ascii_lowercase().as_str()) {
                continue;
            }
            request = request.header(name.as_str(), value);
        }
    }
    if let Some(body) = params.get("body").and_then(Value::as_str) {
        if body.len() > FETCH_BODY_LIMIT {
            return Err(AppError::invalid("the request body is limited to 1 MB"));
        }
        request = request.body(body.to_string());
    }

    let mut response = request.send().await.map_err(|e| AppError::network(format!("network error: {e}")))?;
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| AppError::network(format!("network error: {e}")))? {
        body.extend_from_slice(&chunk);
        if body.len() > FETCH_LIMIT {
            return Err(AppError::invalid("the response is larger than 10 MB"));
        }
    }
    Ok(json!({
        "status": status,
        "ok": (200..300).contains(&status),
        "contentType": content_type,
        "body": String::from_utf8_lossy(&body),
    }))
}

fn window_label(addon: &str, window: &str) -> String {
    format!("addon:{addon}:{window}")
}

pub fn open_window(app: &AppHandle, manifest: &Manifest, window: &str) -> AppResult<()> {
    let def = manifest
        .contributes
        .windows
        .iter()
        .find(|w| w.id == window)
        .ok_or_else(|| AppError::not_found("the addon has no such window"))?;
    let label = window_label(&manifest.id, &def.id);
    if let Some(existing) = app.get_webview_window(&label) {
        let _ = existing.unminimize();
        let _ = existing.show();
        let _ = existing.set_focus();
        return Ok(());
    }
    let url = format!("addon-window?addon={}&window={}", manifest.id, def.id);
    let builder = WebviewWindowBuilder::new(app, &label, WebviewUrl::App(url.into()))
        .title(format!("{} — {}", def.title, manifest.name))
        .inner_size(f64::from(def.width.unwrap_or(800)), f64::from(def.height.unwrap_or(600)))
        .min_inner_size(200.0, 150.0)
        .resizable(def.resizable);

    #[cfg(target_os = "macos")]
    let builder = builder
        .decorations(true)
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true)
        .traffic_light_position(tauri::LogicalPosition::new(12.0, 18.0));

    #[cfg(not(target_os = "macos"))]
    let builder = builder.decorations(false);

    builder.build().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn close_windows(app: &AppHandle, addon: &str) {
    let prefix = format!("addon:{addon}:");
    for (label, window) in app.webview_windows() {
        if label.starts_with(&prefix) {
            let _ = window.close();
        }
    }
}

fn check_host_call(manifest: &Manifest, method: &str, params: &Value) -> AppResult<()> {
    match method {
        "ui.navigate" => {
            let page = text(params, "page", 64)?;
            if !manifest.contributes.pages.iter().any(|p| p.id == page) {
                return Err(AppError::not_found("the addon has no such page"));
            }
        }
        "ui.openUrl" => {
            https_url(&text(params, "url", 2000)?).map_err(AppError::invalid)?;
        }
        "ui.toast" => {
            text(params, "title", 200)?;
        }
        "ui.confirm" => {
            text(params, "message", 500)?;
        }
        _ => {}
    }
    Ok(())
}

#[tauri::command]
pub async fn addon_call(
    app: AppHandle,
    state: State<'_, AppState>,
    addon: String,
    method: String,
    params: Value,
) -> AppResult<Value> {
    let manifest = addons::active_manifest(&addon)?;
    authorize(&manifest, &method)?;

    if HOST_METHODS.contains(&method.as_str()) {
        if method == "instances.launch" {
            instance_id(&params).await?;
        }
        check_host_call(&manifest, &method, &params)?;
        return Ok(json!({ "$host": true }));
    }

    match method.as_str() {
        "instances.list" => {
            let all = crate::commands::instances::list_instances().await?;
            Ok(Value::Array(all.iter().map(public_instance).collect()))
        }
        "instances.get" => {
            let id = instance_id(&params).await?;
            Ok(public_instance(&crate::commands::instances::get_instance(id).await?))
        }
        "instances.content" => {
            let id = instance_id(&params).await?;
            let kind = params.get("kind").and_then(Value::as_str).unwrap_or("mod").to_string();
            if !CONTENT_KINDS.contains(&kind.as_str()) {
                return Err(AppError::invalid(format!("kind has to be one of {}", CONTENT_KINDS.join(", "))));
            }
            let entries = crate::commands::mods::list_content(id, kind).await?;
            Ok(without(serde_json::to_value(entries)?, &["icon_path"]))
        }
        "instances.worlds" => {
            let id = instance_id(&params).await?;
            let worlds = crate::commands::content::list_worlds(id).await?;
            Ok(without(serde_json::to_value(worlds)?, &["icon_path"]))
        }
        "instances.isRunning" => {
            let id = instance_id(&params).await?;
            Ok(json!(crate::commands::launch::is_instance_running(state, id)?))
        }
        "instances.update" => {
            let id = instance_id(&params).await?;
            let mut instance = crate::commands::instances::get_instance(id).await?;
            if params.get("name").is_some() {
                instance.name = text(&params, "name", 100)?;
            }
            if let Some(memory) = params.get("memoryMb") {
                let mb = memory
                    .as_u64()
                    .filter(|mb| (512..=65536).contains(mb))
                    .ok_or_else(|| AppError::invalid("memoryMb has to be between 512 and 65536"))?;
                instance.memory_mb = Some(mb as u32);
                instance.override_memory = true;
            }
            if let Some(group) = params.get("group") {
                instance.group = match group {
                    Value::Null => None,
                    _ => Some(text(&params, "group", 60)?),
                };
            }
            crate::commands::instances::update_instance(instance.clone()).await?;
            Ok(public_instance(&instance))
        }
        "instances.setContentEnabled" => {
            let id = instance_id(&params).await?;
            let kind = text(&params, "kind", 20)?;
            if !CONTENT_KINDS.contains(&kind.as_str()) {
                return Err(AppError::invalid(format!("kind has to be one of {}", CONTENT_KINDS.join(", "))));
            }
            let filename = text(&params, "filename", 255)?;
            let enabled = params.get("enabled").and_then(Value::as_bool).unwrap_or(true);
            crate::commands::content::set_content_enabled(id, kind, filename, enabled)?;
            Ok(Value::Null)
        }
        "instances.stop" => {
            let id = instance_id(&params).await?;
            crate::commands::launch::stop_instance(state, id, false).await?;
            Ok(Value::Null)
        }
        "logs.console" => {
            let id = instance_id(&params).await?;
            let cursor = params.get("cursor").and_then(Value::as_u64).unwrap_or(0);
            Ok(serde_json::to_value(crate::commands::launch::read_console(state, id, cursor)?)?)
        }
        "logs.list" => {
            let id = instance_id(&params).await?;
            Ok(serde_json::to_value(crate::commands::content::list_log_files(id).await?)?)
        }
        "logs.read" => {
            let id = instance_id(&params).await?;
            let rel = text(&params, "rel", 300)?;
            Ok(json!(crate::commands::content::read_log_file(id, rel).await?))
        }
        "servers.ping" => {
            let host = text(&params, "host", 253)?;
            if host.contains(char::is_whitespace) {
                return Err(AppError::invalid("host is not a server address"));
            }
            let port = params.get("port").and_then(Value::as_u64).and_then(|p| u16::try_from(p).ok());
            Ok(serde_json::to_value(crate::commands::ping::ping_server(host, port).await?)?)
        }
        "account.minecraft" => {
            let file = crate::commands::auth::list_accounts().await?;
            let active = file
                .accounts
                .iter()
                .find(|a| Some(&a.uuid) == file.active_uuid.as_ref())
                .or_else(|| file.accounts.first());
            Ok(match active {
                Some(account) => json!({ "uuid": account.uuid, "username": account.username, "kind": account.kind }),
                None => Value::Null,
            })
        }
        "account.spectra" => {
            let user = crate::commands::spectra::spectra_session().await?;
            Ok(match user {
                Some(user) => json!({
                    "username": user.get("username"),
                    "name": user.get("name"),
                    "image": user.get("image"),
                }),
                None => Value::Null,
            })
        }
        "skins.list" => Ok(serde_json::to_value(crate::commands::skins::list_skins().await?)?),
        "http.fetch" => fetch(&manifest, &params).await,
        "storage.get" | "storage.set" | "storage.remove" | "storage.keys" => {
            let (id, name, params) = (manifest.id.clone(), method.clone(), params.clone());
            crate::blocking(move || storage(&id, &name, &params)).await
        }
        "ui.openWindow" => {
            open_window(&app, &manifest, &text(&params, "window", 64)?)?;
            Ok(Value::Null)
        }
        "launcher.version" => Ok(json!(env!("CARGO_PKG_VERSION"))),
        _ => Err(AppError::new("unknown_method", format!("{method} is not part of the addon API"))),
    }
}

#[tauri::command]
pub async fn addons_open_window(app: AppHandle, addon: String, window: String) -> AppResult<()> {
    let manifest = addons::active_manifest(&addon)?;
    open_window(&app, &manifest, &window)
}

#[tauri::command]
pub fn addon_origin() -> String {
    origin()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::addons::{install, Package, Staged};
    use std::io::Write;
    use std::path::Path;

    fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let mut writer = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        for (name, bytes) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();
    }

    fn with_addon<T>(run: impl FnOnce(&Path) -> T) -> T {
        let _guard = paths::lock_data_dir();
        let root = std::env::temp_dir().join(format!("spectra-host-{}", uuid::Uuid::new_v4()));
        std::env::set_var("SPECTRA_DATA_DIR", &root);
        std::fs::create_dir_all(&root).unwrap();

        let manifest = serde_json::to_vec(&json!({
            "id": "stats",
            "name": "Stats",
            "version": "1.0.0",
            "api": 1,
            "main": "main.js",
            "permissions": ["instances:read", "network:api.example.com"],
            "contributes": {
                "pages": [{ "id": "home", "title": "Home", "entry": "ui/home page.html" }],
                "windows": [{ "id": "overlay", "title": "Overlay", "entry": "ui/overlay.html" }]
            }
        }))
        .unwrap();
        let zip = root.join("stats.zip");
        write_zip(&zip, &[
            ("addon.json", &manifest),
            ("main.js", b"spectra.events.on('game:exit', () => {})"),
            ("ui/home page.html", b"<!DOCTYPE html><title>Home</title><img src=\"../icon.png\">"),
            ("ui/overlay.html", b"<p>no doctype</p>"),
            ("icon.png", b"PNG"),
        ]);
        install(&Staged {
            package: Package::Zip(zip),
            source: "file",
            project: None,
            icon: None,
            sha512: None,
            dev_path: None,
            owned: true,
        })
        .unwrap();

        let out = run(&root);
        std::fs::remove_dir_all(&root).unwrap();
        out
    }

    fn body(served: &Served) -> String {
        String::from_utf8_lossy(&served.body).into_owned()
    }

    #[test]
    fn the_sdk_is_served_to_everyone_at_one_address() {
        let served = serve("/__spectra/sdk.js");
        assert_eq!(served.status, 200);
        assert!(served.mime.starts_with("text/javascript"));
        assert!(body(&served).contains("window.spectra = Object.freeze"));
    }

    #[test]
    fn html_gets_the_sdk_first_and_a_policy_scoped_to_its_addon() {
        with_addon(|_| {
            let page = serve("/stats/ui/home%20page.html");
            assert_eq!(page.status, 200);
            assert!(body(&page).starts_with("<!DOCTYPE html><script src=\"/__spectra/sdk.js\"></script><title>"));
            let policy = page.csp.expect("html carries a policy");
            assert!(policy.contains(&format!("script-src 'unsafe-inline' {}/stats/ {}/__spectra/", origin(), origin())));
            assert!(policy.contains("frame-src 'none'"));
            assert!(policy.contains(&format!("connect-src {}/stats/;", origin())));

            let bare = serve("/stats/ui/overlay.html");
            assert!(body(&bare).starts_with("<script src=\"/__spectra/sdk.js\"></script><p>"));

            let image = serve("/stats/icon.png");
            assert_eq!(image.status, 200);
            assert_eq!(image.mime, "image/png");
            assert!(image.csp.is_none());
        });
    }

    #[test]
    fn the_main_script_gets_a_document_of_its_own() {
        with_addon(|_| {
            let main = serve("/stats/__spectra/main.html");
            assert_eq!(main.status, 200);
            let html = body(&main);
            assert!(html.contains("<script src=\"/__spectra/sdk.js\"></script><script src=\"/stats/main.js\"></script>"));
            assert!(main.csp.is_some());
        });
    }

    #[test]
    fn nothing_outside_an_enabled_addon_is_served() {
        with_addon(|root| {
            std::fs::write(root.join("secret.txt"), "private").unwrap();
            for path in [
                "/stats/../secret.txt",
                "/stats/%2e%2e/secret.txt",
                "/stats/..%2Fsecret.txt",
                "/stats/C:/Windows/win.ini",
                "/stats/__spectra/other.js",
                "/missing/icon.png",
                "/.staging/x.zip",
                "/stats/nope.png",
                "/stats/%zz",
                "/stats",
            ] {
                assert_eq!(serve(path).status, 404, "{path}");
            }

            let mut state: serde_json::Value =
                serde_json::from_slice(&std::fs::read(paths::addons_state_file()).unwrap()).unwrap();
            state["addons"][0]["enabled"] = json!(false);
            std::fs::write(paths::addons_state_file(), serde_json::to_vec(&state).unwrap()).unwrap();
            assert_eq!(serve("/stats/icon.png").status, 404);
            assert_eq!(serve("/stats/__spectra/main.html").status, 404);
        });
    }

    #[test]
    fn the_sdk_goes_after_the_doctype_whatever_its_case() {
        assert!(inject_sdk("  <!doctype html><p>x").starts_with("<!doctype html><script"));
        assert!(inject_sdk("<!DocType html>").starts_with("<!DocType html><script"));
        assert!(inject_sdk("<p>x").starts_with("<script src=\"/__spectra/sdk.js\"></script><p>"));
        assert!(inject_sdk("żółw").ends_with("żółw"));
    }

    fn manifest(permissions: &[&str]) -> Manifest {
        serde_json::from_value(json!({
            "id": "a", "name": "A", "version": "1.0.0", "api": 1,
            "permissions": permissions,
        }))
        .unwrap()
    }

    #[test]
    fn every_call_needs_the_permission_it_names() {
        let none = manifest(&[]);
        let reader = manifest(&["instances:read"]);
        assert_eq!(authorize(&none, "instances.list").unwrap_err().code, "permission_denied");
        authorize(&reader, "instances.list").unwrap();
        assert_eq!(authorize(&reader, "instances.update").unwrap_err().code, "permission_denied");
        assert_eq!(authorize(&reader, "instances.launch").unwrap_err().code, "permission_denied");
        authorize(&none, "storage.set").unwrap();
        authorize(&none, "ui.toast").unwrap();
        assert_eq!(authorize(&reader, "fs.read").unwrap_err().code, "unknown_method");
        assert_eq!(authorize(&reader, "auth_login").unwrap_err().code, "unknown_method");
    }

    #[test]
    fn every_host_method_is_a_known_method() {
        for method in HOST_METHODS {
            assert!(requirement(method).is_some(), "{method}");
        }
    }

    #[test]
    fn the_network_reaches_only_named_hosts_over_https() {
        let permissions = vec!["network:api.example.com".to_string()];
        assert!(fetch_target(&permissions, "https://api.example.com/v1/stats?x=1").is_ok());
        for bad in [
            "http://api.example.com/",
            "https://example.com/",
            "https://evil.api.example.com/",
            "https://api.example.com.evil.com/",
            "https://user:pass@api.example.com/",
            "file:///C:/Windows/win.ini",
            "not a url",
        ] {
            assert!(fetch_target(&permissions, bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn storage_keeps_values_per_addon_and_stops_at_its_limit() {
        let _guard = paths::lock_data_dir();
        let root = std::env::temp_dir().join(format!("spectra-storage-{}", uuid::Uuid::new_v4()));
        std::env::set_var("SPECTRA_DATA_DIR", &root);

        storage("one", "storage.set", &json!({ "key": "k", "value": { "n": 1 } })).unwrap();
        assert_eq!(storage("one", "storage.get", &json!({ "key": "k" })).unwrap(), json!({ "n": 1 }));
        assert_eq!(storage("two", "storage.get", &json!({ "key": "k" })).unwrap(), Value::Null);
        assert_eq!(storage("one", "storage.keys", &json!({})).unwrap(), json!(["k"]));

        let big = "x".repeat(STORAGE_LIMIT);
        assert!(storage("one", "storage.set", &json!({ "key": "big", "value": big })).is_err());
        assert_eq!(storage("one", "storage.get", &json!({ "key": "big" })).unwrap(), Value::Null);

        storage("one", "storage.remove", &json!({ "key": "k" })).unwrap();
        assert_eq!(storage("one", "storage.keys", &json!({})).unwrap(), json!([]));

        forget_storage("one");
        assert!(!storage_file("one").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_file_server_answers_only_reads_for_its_own_host() {
        use std::io::Read;
        let port = start_server().unwrap();
        assert_eq!(origin(), format!("http://127.0.0.1:{port}"));

        let ask = |request: String| {
            let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
            stream.write_all(request.as_bytes()).unwrap();
            let mut out = String::new();
            stream.read_to_string(&mut out).unwrap();
            out
        };

        let sdk = ask(format!("GET /__spectra/sdk.js?x=1 HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"));
        assert!(sdk.starts_with("HTTP/1.1 200 OK\r\n"), "{sdk}");
        assert!(sdk.contains("window.spectra"));

        let rebound = ask("GET /__spectra/sdk.js HTTP/1.1\r\nHost: evil.example\r\n\r\n".to_string());
        assert!(rebound.starts_with("HTTP/1.1 421"), "{rebound}");

        let post = ask(format!("POST /__spectra/sdk.js HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"));
        assert!(post.starts_with("HTTP/1.1 405"), "{post}");

        let head = ask(format!("HEAD /__spectra/sdk.js HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"));
        assert!(head.starts_with("HTTP/1.1 200") && !head.contains("window.spectra"), "{head}");

        let missing = ask(format!("GET /nope/x.html HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"));
        assert!(missing.starts_with("HTTP/1.1 404"), "{missing}");
    }

    #[test]
    fn closing_one_addon_never_closes_another() {
        let prefix = "addon:foo:";
        assert!(window_label("foo", "x").starts_with(prefix));
        assert!(!window_label("foo-bar", "x").starts_with(prefix));
    }
}
