use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};
use tauri::{Emitter, State};

use crate::commands::spectra;
use crate::error::{AppError, AppResult};
use crate::{paths, store, AppState};

pub const API_VERSIONS: &[u32] = &[1];

pub const PERMISSIONS: &[&str] = &[
    "instances:read",
    "instances:write",
    "instances:launch",
    "logs:read",
    "servers:ping",
    "account:read",
    "skins:read",
];

pub const SLOTS: &[&str] = &[
    "sidebar.menu",
    "sidebar.footer",
    "titlebar",
    "home.header",
    "instance.header",
    "instance.menu",
    "worlds.header",
    "screenshots.header",
    "skins.header",
    "settings.header",
];

const MODES: &[&str] = &["dark", "oled", "squared"];

const ACCENTS: &[&str] = &[
    "sky", "blue", "indigo", "violet", "purple", "pink", "rose", "red", "orange", "amber", "green",
    "emerald", "teal", "cyan",
];

const MAX_ENTRIES: usize = 20;
const MAX_PERMISSIONS: usize = 30;
const MAX_FILES: usize = 2000;
const MAX_BYTES: u64 = 100 * 1024 * 1024;
const MAX_JSON: u64 = 1024 * 1024;
const MAX_LOCALE_KEYS: usize = 500;
const SKIPPED_DIRS: &[&str] = &[".git", "node_modules"];

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    pub api: u32,
    #[serde(default)]
    pub launcher: Option<String>,
    #[serde(default)]
    pub main: Option<String>,
    #[serde(default)]
    pub backend: Option<String>,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub contributes: Contributes,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Contributes {
    #[serde(default)]
    pub pages: Vec<View>,
    #[serde(default)]
    pub instance_tabs: Vec<View>,
    #[serde(default)]
    pub settings: Option<String>,
    #[serde(default)]
    pub themes: Vec<ThemeRef>,
    #[serde(default)]
    pub locales: BTreeMap<String, String>,
    #[serde(default)]
    pub buttons: Vec<Button>,
    #[serde(default)]
    pub windows: Vec<WindowDef>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct View {
    pub id: String,
    pub title: String,
    pub entry: String,
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ThemeRef {
    pub id: String,
    pub name: String,
    pub file: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Button {
    pub id: String,
    pub slot: String,
    pub title: String,
    #[serde(default)]
    pub icon: Option<String>,
    pub action: Action,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Action {
    Url { url: String },
    Page { page: String },
    Window { window: String },
    Command { command: String },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WindowDef {
    pub id: String,
    pub title: String,
    pub entry: String,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default = "yes")]
    pub resizable: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Default, Deserialize)]
struct ThemeFile {
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    accent: Option<String>,
    #[serde(default)]
    background: Option<String>,
}

pub fn valid_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 64
        && bytes.iter().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
        && bytes[0] != b'-'
        && bytes[bytes.len() - 1] != b'-'
}

fn valid_version(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 60
        && bytes[0].is_ascii_alphanumeric()
        && bytes.iter().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'+' | b'-'))
}

fn valid_host(value: &str) -> bool {
    let labels: Vec<&str> = value.split('.').collect();
    labels.len() >= 2
        && labels.iter().all(|label| {
            let b = label.as_bytes();
            !b.is_empty()
                && b.len() <= 63
                && b.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
                && b[0] != b'-'
                && b[b.len() - 1] != b'-'
        })
        && labels
            .last()
            .is_some_and(|tld| tld.len() >= 2 && tld.bytes().all(|c| c.is_ascii_lowercase()))
}

fn valid_locale(value: &str) -> bool {
    let b = value.as_bytes();
    match b.len() {
        2 => b.iter().all(u8::is_ascii_lowercase),
        5 => b[..2].iter().all(u8::is_ascii_lowercase) && b[2] == b'-' && b[3..].iter().all(u8::is_ascii_uppercase),
        _ => false,
    }
}

pub fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 512
        && !name.starts_with('/')
        && !name.contains('\\')
        && !name.contains(':')
        && !name.chars().any(char::is_control)
        && name.split('/').all(|part| !part.is_empty() && part != "." && part != "..")
}

fn text(value: &str, field: &str, max: usize) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{field} is required"));
    }
    if trimmed.chars().count() > max {
        return Err(format!("{field} is longer than {max} characters"));
    }
    Ok(())
}

fn file(files: &BTreeSet<String>, value: &str, field: &str, extensions: &[&str]) -> Result<(), String> {
    let name = value.trim().trim_start_matches("./");
    if !safe_name(name) {
        return Err(format!("{field} is not a path inside the archive"));
    }
    let lower = name.to_ascii_lowercase();
    if !extensions.iter().any(|ext| lower.ends_with(ext)) {
        return Err(format!("{field} has to end in {}", extensions.join(" or ")));
    }
    if !files.contains(name) {
        return Err(format!("{field} points at {name}, which is not in the archive"));
    }
    Ok(())
}

fn unique<'a>(ids: impl Iterator<Item = &'a String>, field: &str) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for id in ids {
        if !valid_id(id) {
            return Err(format!("{field} id {id} may only use a-z, 0-9 and dashes"));
        }
        if !seen.insert(id) {
            return Err(format!("{field} repeats the id {id}"));
        }
    }
    Ok(())
}

fn https_url(value: &str, field: &str) -> Result<(), String> {
    let url = reqwest::Url::parse(value.trim()).map_err(|_| format!("{field} is not an address"))?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() || url.host_str().is_none() {
        return Err(format!("{field} has to be a plain https address"));
    }
    Ok(())
}

pub fn validate(manifest: &Manifest, files: &BTreeSet<String>) -> Result<(), String> {
    if !valid_id(&manifest.id) {
        return Err("id may only use a-z, 0-9 and dashes, and cannot start or end with a dash".into());
    }
    text(&manifest.name, "name", 64)?;
    if !valid_version(&manifest.version) {
        return Err("version may only use letters, digits, dots, dashes and plus signs".into());
    }
    if let Some(description) = &manifest.description {
        text(description, "description", 400)?;
    }
    if !API_VERSIONS.contains(&manifest.api) {
        return Err(format!("api has to be one of {API_VERSIONS:?}"));
    }
    if let Some(range) = &manifest.launcher {
        semver::VersionReq::parse(range.trim()).map_err(|_| "launcher is not a version range".to_string())?;
    }
    if let Some(main) = &manifest.main {
        file(files, main, "main", &[".js", ".mjs"])?;
    }
    if let Some(backend) = &manifest.backend {
        file(files, backend, "backend", &[".wasm"])?;
    }

    if manifest.permissions.len() > MAX_PERMISSIONS {
        return Err(format!("permissions has more than {MAX_PERMISSIONS} entries"));
    }
    for (i, permission) in manifest.permissions.iter().enumerate() {
        let network = permission.strip_prefix("network:").is_some_and(valid_host);
        if !PERMISSIONS.contains(&permission.as_str()) && !network {
            return Err(format!("permissions[{i}] is not a permission: {permission}"));
        }
    }

    let c = &manifest.contributes;
    for (list, field) in [(&c.pages, "contributes.pages"), (&c.instance_tabs, "contributes.instanceTabs")] {
        if list.len() > MAX_ENTRIES {
            return Err(format!("{field} has more than {MAX_ENTRIES} entries"));
        }
        unique(list.iter().map(|v| &v.id), field)?;
        for (i, view) in list.iter().enumerate() {
            text(&view.title, &format!("{field}[{i}].title"), 64)?;
            file(files, &view.entry, &format!("{field}[{i}].entry"), &[".html"])?;
            if let Some(icon) = &view.icon {
                file(files, icon, &format!("{field}[{i}].icon"), &[".svg", ".png", ".webp"])?;
            }
        }
    }
    if let Some(settings) = &c.settings {
        file(files, settings, "contributes.settings", &[".html"])?;
    }

    if c.themes.len() > MAX_ENTRIES {
        return Err(format!("contributes.themes has more than {MAX_ENTRIES} entries"));
    }
    unique(c.themes.iter().map(|t| &t.id), "contributes.themes")?;
    for (i, theme) in c.themes.iter().enumerate() {
        text(&theme.name, &format!("contributes.themes[{i}].name"), 64)?;
        file(files, &theme.file, &format!("contributes.themes[{i}].file"), &[".json"])?;
    }

    if c.locales.len() > MAX_ENTRIES {
        return Err(format!("contributes.locales has more than {MAX_ENTRIES} entries"));
    }
    for (code, path) in &c.locales {
        if !valid_locale(code) {
            return Err(format!("contributes.locales has an unknown language code {code}"));
        }
        file(files, path, &format!("contributes.locales.{code}"), &[".json"])?;
    }

    if c.windows.len() > MAX_ENTRIES {
        return Err(format!("contributes.windows has more than {MAX_ENTRIES} entries"));
    }
    unique(c.windows.iter().map(|w| &w.id), "contributes.windows")?;
    for (i, window) in c.windows.iter().enumerate() {
        let field = format!("contributes.windows[{i}]");
        text(&window.title, &format!("{field}.title"), 64)?;
        file(files, &window.entry, &format!("{field}.entry"), &[".html"])?;
        if window.width.is_some_and(|w| !(200..=3840).contains(&w)) {
            return Err(format!("{field}.width has to be a whole number between 200 and 3840"));
        }
        if window.height.is_some_and(|h| !(150..=2160).contains(&h)) {
            return Err(format!("{field}.height has to be a whole number between 150 and 2160"));
        }
    }

    if c.buttons.len() > MAX_ENTRIES {
        return Err(format!("contributes.buttons has more than {MAX_ENTRIES} entries"));
    }
    unique(c.buttons.iter().map(|b| &b.id), "contributes.buttons")?;
    for (i, button) in c.buttons.iter().enumerate() {
        let field = format!("contributes.buttons[{i}]");
        if !SLOTS.contains(&button.slot.as_str()) {
            return Err(format!("{field}.slot is not a place in the launcher: {}", button.slot));
        }
        text(&button.title, &format!("{field}.title"), 64)?;
        if let Some(icon) = &button.icon {
            file(files, icon, &format!("{field}.icon"), &[".svg", ".png", ".webp"])?;
        }
        match &button.action {
            Action::Url { url } => https_url(url, &format!("{field}.action.url"))?,
            Action::Page { page } if !c.pages.iter().any(|p| &p.id == page) => {
                return Err(format!("{field} opens a page the addon does not have: {page}"));
            }
            Action::Window { window } if !c.windows.iter().any(|w| &w.id == window) => {
                return Err(format!("{field} opens a window the addon does not have: {window}"));
            }
            Action::Command { .. } if manifest.main.is_none() => {
                return Err(format!("{field} sends a command, which needs main"));
            }
            _ => {}
        }
    }

    let empty = manifest.main.is_none()
        && manifest.backend.is_none()
        && c.pages.is_empty()
        && c.instance_tabs.is_empty()
        && c.settings.is_none()
        && c.themes.is_empty()
        && c.locales.is_empty()
        && c.buttons.is_empty()
        && c.windows.is_empty();
    if empty {
        return Err("the addon does not contribute anything".into());
    }
    Ok(())
}

pub fn launcher_version() -> semver::Version {
    semver::Version::parse(env!("CARGO_PKG_VERSION")).expect("package version is semver")
}

pub fn fits_launcher(range: Option<&str>, version: &semver::Version) -> bool {
    match range {
        None => true,
        Some(range) => semver::VersionReq::parse(range.trim()).is_ok_and(|req| req.matches(version)),
    }
}

pub fn runs_code(manifest: &Manifest) -> bool {
    let c = &manifest.contributes;
    manifest.main.is_some()
        || manifest.backend.is_some()
        || !c.pages.is_empty()
        || !c.instance_tabs.is_empty()
        || c.settings.is_some()
        || !c.windows.is_empty()
        || c.buttons.iter().any(|b| !matches!(b.action, Action::Url { .. }))
}

#[derive(Debug, Clone)]
pub enum Package {
    Zip(PathBuf),
    Folder(PathBuf),
}

impl Package {
    fn files(&self) -> AppResult<BTreeSet<String>> {
        let mut out = BTreeSet::new();
        let mut total = 0u64;
        match self {
            Package::Zip(path) => {
                let mut archive = open_zip(path)?;
                if archive.len() > MAX_FILES {
                    return Err(AppError::invalid("the addon has too many files"));
                }
                for index in 0..archive.len() {
                    let entry = archive.by_index(index).map_err(|e| AppError::invalid(e.to_string()))?;
                    if entry.is_dir() {
                        continue;
                    }
                    let name = entry.name().to_string();
                    if !safe_name(&name) {
                        return Err(AppError::invalid(format!("the addon holds a file outside itself: {name}")));
                    }
                    total += entry.size();
                    if total > MAX_BYTES {
                        return Err(AppError::invalid("the addon is larger than 100 MB unpacked"));
                    }
                    out.insert(name);
                }
            }
            Package::Folder(root) => walk(root, root, &mut out, &mut total)?,
        }
        Ok(out)
    }

    fn read(&self, name: &str, limit: u64) -> AppResult<Vec<u8>> {
        if !safe_name(name) {
            return Err(AppError::invalid(format!("{name} is not a path inside the addon")));
        }
        let mut bytes = Vec::new();
        match self {
            Package::Zip(path) => {
                let mut archive = open_zip(path)?;
                let entry = archive.by_name(name).map_err(|_| AppError::not_found(format!("{name} is missing")))?;
                if entry.size() > limit {
                    return Err(AppError::invalid(format!("{name} is too large")));
                }
                entry.take(limit + 1).read_to_end(&mut bytes)?;
            }
            Package::Folder(root) => {
                let file = std::fs::File::open(root.join(name)).map_err(|_| AppError::not_found(format!("{name} is missing")))?;
                file.take(limit + 1).read_to_end(&mut bytes)?;
            }
        }
        if bytes.len() as u64 > limit {
            return Err(AppError::invalid(format!("{name} is too large")));
        }
        Ok(bytes)
    }

    fn extract(&self, files: &BTreeSet<String>, dest: &Path) -> AppResult<()> {
        std::fs::create_dir_all(dest)?;
        match self {
            Package::Zip(path) => {
                let mut archive = open_zip(path)?;
                let mut written = 0u64;
                for index in 0..archive.len() {
                    let entry = archive.by_index(index).map_err(|e| AppError::invalid(e.to_string()))?;
                    let name = entry.name().to_string();
                    if entry.is_dir() || !files.contains(&name) {
                        continue;
                    }
                    let declared = entry.size();
                    let target = dest.join(&name);
                    if let Some(parent) = target.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    let mut out = std::fs::File::create(&target)?;
                    let copied = std::io::copy(&mut entry.take(declared + 1), &mut out)?;
                    written += copied;
                    if copied > declared || written > MAX_BYTES {
                        return Err(AppError::invalid("the addon unpacks to more than it declares"));
                    }
                    out.flush()?;
                }
            }
            Package::Folder(root) => {
                for name in files {
                    let target = dest.join(name);
                    if let Some(parent) = target.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::copy(root.join(name), target)?;
                }
            }
        }
        Ok(())
    }
}

fn open_zip(path: &Path) -> AppResult<zip::ZipArchive<std::fs::File>> {
    let file = std::fs::File::open(path)?;
    zip::ZipArchive::new(file).map_err(|_| AppError::invalid("the file is not a zip archive"))
}

fn walk(root: &Path, dir: &Path, out: &mut BTreeSet<String>, total: &mut u64) -> AppResult<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let meta = std::fs::symlink_metadata(entry.path())?;
        let file_name = entry.file_name().to_string_lossy().into_owned();
        if meta.file_type().is_symlink() || SKIPPED_DIRS.contains(&file_name.as_str()) {
            continue;
        }
        if meta.is_dir() {
            walk(root, &entry.path(), out, total)?;
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(root)
            .map_err(|e| AppError::invalid(e.to_string()))?
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        if !safe_name(&relative) {
            return Err(AppError::invalid(format!("{relative} cannot be part of an addon")));
        }
        *total += meta.len();
        if out.len() >= MAX_FILES {
            return Err(AppError::invalid("the addon has too many files"));
        }
        if *total > MAX_BYTES {
            return Err(AppError::invalid("the addon is larger than 100 MB"));
        }
        out.insert(relative);
    }
    Ok(())
}

pub struct Inspected {
    pub manifest: Manifest,
    pub files: BTreeSet<String>,
    pub themes: Vec<ThemeInfo>,
    pub locales: BTreeMap<String, BTreeMap<String, String>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeInfo {
    pub id: String,
    pub name: String,
    pub mode: Option<String>,
    pub accent: Option<String>,
    pub background: Option<String>,
}

fn read_manifest(package: &Package) -> AppResult<Manifest> {
    let bytes = package
        .read("addon.json", MAX_JSON)
        .map_err(|e| if e.code == "not_found" { AppError::invalid("the file has no addon.json") } else { e })?;
    serde_json::from_slice(&bytes).map_err(|e| AppError::invalid(format!("addon.json: {e}")))
}

fn read_theme(package: &Package, files: &BTreeSet<String>, theme: &ThemeRef) -> Result<ThemeInfo, String> {
    let bytes = package.read(&theme.file, MAX_JSON).map_err(|e| e.message)?;
    let parsed: ThemeFile = serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", theme.file))?;
    if parsed.mode.as_deref().is_some_and(|m| !MODES.contains(&m)) {
        return Err(format!("{}: mode has to be one of {}", theme.file, MODES.join(", ")));
    }
    if parsed.accent.as_deref().is_some_and(|a| !ACCENTS.contains(&a)) {
        return Err(format!("{}: accent has to be one of {}", theme.file, ACCENTS.join(", ")));
    }
    if let Some(background) = &parsed.background {
        file(files, background, &format!("{}: background", theme.file), &[".png", ".jpg", ".jpeg", ".webp"])?;
    }
    Ok(ThemeInfo {
        id: theme.id.clone(),
        name: theme.name.clone(),
        mode: parsed.mode,
        accent: parsed.accent,
        background: parsed.background.map(|b| b.trim().trim_start_matches("./").to_string()),
    })
}

fn read_locale(package: &Package, path: &str) -> Result<BTreeMap<String, String>, String> {
    let bytes = package.read(path, MAX_JSON).map_err(|e| e.message)?;
    let strings: BTreeMap<String, String> =
        serde_json::from_slice(&bytes).map_err(|_| format!("{path} has to map keys to plain strings"))?;
    if strings.len() > MAX_LOCALE_KEYS || strings.iter().any(|(k, v)| k.len() > 100 || v.chars().count() > 500) {
        return Err(format!("{path} is too large"));
    }
    Ok(strings)
}

pub fn inspect(package: &Package) -> AppResult<Inspected> {
    let files = package.files()?;
    let manifest = read_manifest(package)?;
    validate(&manifest, &files).map_err(|e| AppError::invalid(format!("addon.json: {e}")))?;

    let mut themes = Vec::new();
    for theme in &manifest.contributes.themes {
        themes.push(read_theme(package, &files, theme).map_err(AppError::invalid)?);
    }
    let mut locales = BTreeMap::new();
    for (code, path) in &manifest.contributes.locales {
        locales.insert(code.clone(), read_locale(package, path).map_err(AppError::invalid)?);
    }
    Ok(Inspected { manifest, files, themes, locales })
}

pub fn check_runs_here(manifest: &Manifest) -> AppResult<()> {
    if !fits_launcher(manifest.launcher.as_deref(), &launcher_version()) {
        return Err(AppError::new(
            "unsupported",
            format!("This addon needs Spectra {}.", manifest.launcher.as_deref().unwrap_or_default()),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Installed {
    pub id: String,
    pub version: String,
    pub source: String,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub sha512: Option<String>,
    #[serde(default)]
    pub dev_path: Option<String>,
    pub enabled: bool,
    pub installed_at: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct StateFile {
    #[serde(default)]
    addons: Vec<Installed>,
}

fn load_state() -> AppResult<Vec<Installed>> {
    Ok(store::read_json::<StateFile>(&paths::addons_state_file())?.unwrap_or_default().addons)
}

fn save_state(addons: Vec<Installed>) -> AppResult<()> {
    store::write_json(&paths::addons_state_file(), &StateFile { addons })
}

#[derive(Debug, Clone)]
pub struct Staged {
    pub package: Package,
    pub source: &'static str,
    pub project: Option<String>,
    pub icon: Option<String>,
    pub sha512: Option<String>,
    pub dev_path: Option<String>,
    pub owned: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewButton {
    pub slot: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub token: String,
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub source: String,
    pub project: Option<String>,
    pub installed: Option<String>,
    pub themes: Vec<String>,
    pub buttons: Vec<PreviewButton>,
    pub links: Vec<String>,
    pub permissions: Vec<String>,
    pub runs_code: bool,
}

pub fn preview(token: &str, staged: &Staged) -> AppResult<Preview> {
    let inspected = inspect(&staged.package)?;
    check_runs_here(&inspected.manifest)?;
    let manifest = inspected.manifest;
    let installed = load_state()?.into_iter().find(|a| a.id == manifest.id).map(|a| a.version);
    let code = runs_code(&manifest);

    let links: BTreeSet<String> = manifest
        .contributes
        .buttons
        .iter()
        .filter_map(|b| match &b.action {
            Action::Url { url } => reqwest::Url::parse(url).ok()?.host_str().map(str::to_string),
            _ => None,
        })
        .collect();

    Ok(Preview {
        token: token.to_string(),
        id: manifest.id,
        name: manifest.name,
        version: manifest.version,
        description: manifest.description,
        source: staged.source.to_string(),
        project: staged.project.clone(),
        installed,
        themes: inspected.themes.into_iter().map(|t| t.name).collect(),
        buttons: manifest
            .contributes
            .buttons
            .into_iter()
            .map(|b| PreviewButton { slot: b.slot, title: b.title })
            .collect(),
        links: links.into_iter().collect(),
        permissions: manifest.permissions,
        runs_code: code,
    })
}

pub fn install(staged: &Staged) -> AppResult<Installed> {
    let inspected = inspect(&staged.package)?;
    check_runs_here(&inspected.manifest)?;
    let id = inspected.manifest.id.clone();

    let root = paths::addons_dir();
    let temp = root.join(format!(".tmp-{}", uuid::Uuid::new_v4()));
    if let Err(e) = staged.package.extract(&inspected.files, &temp) {
        let _ = std::fs::remove_dir_all(&temp);
        return Err(e);
    }

    let target = root.join(&id);
    if target.exists() {
        std::fs::remove_dir_all(&target)?;
    }
    std::fs::rename(&temp, &target)?;

    let mut state = load_state()?;
    let enabled = state.iter().find(|a| a.id == id).map_or(true, |a| a.enabled);
    state.retain(|a| a.id != id);
    let entry = Installed {
        id,
        version: inspected.manifest.version,
        source: staged.source.to_string(),
        project: staged.project.clone(),
        icon: staged.icon.clone(),
        sha512: staged.sha512.clone(),
        dev_path: staged.dev_path.clone(),
        enabled,
        installed_at: chrono::Utc::now().to_rfc3339(),
    };
    state.push(entry.clone());
    save_state(state)?;
    crate::commands::addon_host::forget_backend(&entry.id);

    if staged.owned {
        if let Package::Zip(path) = &staged.package {
            let _ = std::fs::remove_file(path);
        }
    }
    Ok(entry)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ButtonInfo {
    pub id: String,
    pub slot: String,
    pub title: String,
    pub icon: Option<String>,
    pub action: Action,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddonInfo {
    #[serde(flatten)]
    pub installed: Installed,
    pub name: String,
    pub description: Option<String>,
    pub dir: String,
    pub error: Option<String>,
    pub themes: Vec<ThemeInfo>,
    pub buttons: Vec<ButtonInfo>,
    pub locales: BTreeMap<String, BTreeMap<String, String>>,
    pub main: Option<String>,
    pub pages: Vec<View>,
    pub instance_tabs: Vec<View>,
    pub settings: Option<String>,
    pub windows: Vec<WindowDef>,
    pub permissions: Vec<String>,
}

fn with_icon(dir: &Path, view: View) -> View {
    View { icon: view.icon.as_deref().map(|i| absolute(dir, i.trim().trim_start_matches("./"))), ..view }
}

fn absolute(dir: &Path, name: &str) -> String {
    name.split('/').fold(dir.to_path_buf(), |path, part| path.join(part)).to_string_lossy().into_owned()
}

pub fn describe(installed: Installed) -> AddonInfo {
    let dir = paths::addons_dir().join(&installed.id);
    let checked = inspect(&Package::Folder(dir.clone())).and_then(|i| check_runs_here(&i.manifest).map(|_| i));
    match checked {
        Ok(inspected) => AddonInfo {
            main: inspected.manifest.main.clone(),
            pages: inspected.manifest.contributes.pages.iter().cloned().map(|v| with_icon(&dir, v)).collect(),
            instance_tabs: inspected
                .manifest
                .contributes
                .instance_tabs
                .iter()
                .cloned()
                .map(|v| with_icon(&dir, v))
                .collect(),
            settings: inspected.manifest.contributes.settings.clone(),
            windows: inspected.manifest.contributes.windows.clone(),
            permissions: inspected.manifest.permissions.clone(),
            name: inspected.manifest.name.clone(),
            description: inspected.manifest.description.clone(),
            dir: dir.to_string_lossy().into_owned(),
            error: None,
            themes: inspected
                .themes
                .into_iter()
                .map(|t| ThemeInfo { background: t.background.as_deref().map(|b| absolute(&dir, b)), ..t })
                .collect(),
            buttons: inspected
                .manifest
                .contributes
                .buttons
                .into_iter()
                .map(|b| ButtonInfo {
                    icon: b.icon.as_deref().map(|i| absolute(&dir, i.trim().trim_start_matches("./"))),
                    id: b.id,
                    slot: b.slot,
                    title: b.title,
                    action: b.action,
                })
                .collect(),
            locales: inspected.locales,
            installed,
        },
        Err(e) => AddonInfo {
            name: installed.id.clone(),
            description: None,
            dir: dir.to_string_lossy().into_owned(),
            error: Some(e.message),
            themes: Vec::new(),
            buttons: Vec::new(),
            locales: BTreeMap::new(),
            main: None,
            pages: Vec::new(),
            instance_tabs: Vec::new(),
            settings: None,
            windows: Vec::new(),
            permissions: Vec::new(),
            installed,
        },
    }
}

pub fn active_manifest(id: &str) -> AppResult<Manifest> {
    if !valid_id(id) {
        return Err(AppError::not_found("no such addon"));
    }
    let entry = load_state()?
        .into_iter()
        .find(|a| a.id == id)
        .ok_or_else(|| AppError::not_found("no such addon"))?;
    if !entry.enabled {
        return Err(AppError::not_found("this addon is turned off"));
    }
    read_manifest(&Package::Folder(paths::addons_dir().join(id)))
}

pub fn is_enabled(id: &str) -> bool {
    valid_id(id) && load_state().is_ok_and(|state| state.iter().any(|a| a.id == id && a.enabled))
}

pub fn slug_from_url(url: &str) -> Option<String> {
    let rest = url.strip_prefix("spectra://")?.trim_start_matches('/');
    let slug = rest.strip_prefix("addon/")?.trim_end_matches('/');
    valid_id(slug).then(|| slug.to_string())
}

#[derive(Debug, Deserialize)]
struct CatalogResponse {
    project: CatalogProject,
}

#[derive(Debug, Deserialize)]
struct CatalogProject {
    slug: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    icon: Option<String>,
    #[serde(default)]
    versions: Vec<CatalogVersion>,
}

#[derive(Debug, Deserialize)]
pub struct CatalogVersion {
    #[serde(default)]
    pub number: String,
    #[serde(default)]
    pub meta: serde_json::Value,
    #[serde(default)]
    pub files: Vec<CatalogFile>,
}

#[derive(Debug, Deserialize)]
pub struct CatalogFile {
    pub id: String,
    pub size: u64,
    #[serde(default)]
    pub primary: bool,
    pub hashes: CatalogHashes,
}

#[derive(Debug, Deserialize)]
pub struct CatalogHashes {
    pub sha512: String,
}

pub fn pick_version<'a>(
    versions: &'a [CatalogVersion],
    launcher: &semver::Version,
) -> Option<(&'a CatalogVersion, &'a CatalogFile)> {
    versions
        .iter()
        .filter(|v| {
            let api = v.meta.get("api").and_then(serde_json::Value::as_u64);
            let range = v.meta.get("launcher").and_then(serde_json::Value::as_str);
            api.is_some_and(|a| API_VERSIONS.iter().any(|x| u64::from(*x) == a)) && fits_launcher(range, launcher)
        })
        .find_map(|v| v.files.iter().find(|f| f.primary).or_else(|| v.files.first()).map(|f| (v, f)))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub id: String,
    pub project: String,
    pub current: String,
    pub latest: String,
    pub new_permissions: Vec<String>,
}

pub fn update_for(
    installed: &Installed,
    granted: &[String],
    versions: &[CatalogVersion],
    launcher: &semver::Version,
) -> Option<UpdateInfo> {
    let project = installed.project.clone()?;
    let (version, file) = pick_version(versions, launcher)?;
    if installed.sha512.as_deref().is_some_and(|s| s.eq_ignore_ascii_case(file.hashes.sha512.trim())) {
        return None;
    }
    let new_permissions = version
        .meta
        .get("permissions")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .filter(|p| !granted.iter().any(|g| g == p))
        .map(str::to_string)
        .collect();
    Some(UpdateInfo {
        id: installed.id.clone(),
        project,
        current: installed.version.clone(),
        latest: version.number.clone(),
        new_permissions,
    })
}

pub fn only_addons(mut body: serde_json::Value) -> serde_json::Value {
    if let Some(hits) = body.get_mut("hits").and_then(serde_json::Value::as_array_mut) {
        hits.retain(|hit| hit.get("type").and_then(serde_json::Value::as_str) == Some("addon"));
    }
    body
}

async fn catalog_get(path: &str) -> AppResult<reqwest::Response> {
    let mut request = crate::http().get(format!("{}{path}", spectra::SITE)).header("origin", spectra::ORIGIN);
    if let Some(token) = spectra::stored_token() {
        request = request.bearer_auth(token);
    }
    let response = request.send().await.map_err(|e| AppError::network(format!("network error: {e}")))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(AppError::not_found(
            "Spectra did not find this addon. While addons are in testing, sign in with a team account.",
        ));
    }
    if !response.status().is_success() {
        return Err(AppError::network(format!("request failed ({})", response.status())));
    }
    Ok(response)
}

async fn download(file: &CatalogFile, dest: &Path) -> AppResult<()> {
    if file.size > MAX_BYTES {
        return Err(AppError::invalid("the addon is larger than 100 MB"));
    }
    let mut response = catalog_get(&format!("/api/catalog/download/{}", file.id)).await?;
    let mut out = std::fs::File::create(dest)?;
    let mut hasher = Sha512::new();
    let mut received = 0u64;
    while let Some(chunk) = response.chunk().await.map_err(|e| AppError::network(format!("network error: {e}")))? {
        received += chunk.len() as u64;
        if received > MAX_BYTES || received > file.size {
            return Err(AppError::invalid("the download is larger than the website said"));
        }
        hasher.update(&chunk);
        out.write_all(&chunk)?;
    }
    out.flush()?;
    let digest = format!("{:x}", hasher.finalize());
    if !digest.eq_ignore_ascii_case(file.hashes.sha512.trim()) {
        return Err(AppError::invalid("the downloaded file does not match its sha512"));
    }
    Ok(())
}

fn ensure_dev_mode() -> AppResult<()> {
    if crate::commands::settings::load()?.addon_dev_mode {
        Ok(())
    } else {
        Err(AppError::new("dev_mode", "Turn on developer mode to load addons from disk."))
    }
}

fn stage(state: &State<'_, AppState>, staged: Staged) -> AppResult<Preview> {
    let token = uuid::Uuid::new_v4().to_string();
    let preview = match preview(&token, &staged) {
        Ok(p) => p,
        Err(e) => {
            discard_staged(&staged);
            return Err(e);
        }
    };
    state
        .staged_addons
        .lock()
        .map_err(|_| AppError::busy("addon staging is locked"))?
        .insert(token, staged);
    Ok(preview)
}

fn discard_staged(staged: &Staged) {
    if staged.owned {
        if let Package::Zip(path) = &staged.package {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn staging_file() -> AppResult<PathBuf> {
    let dir = paths::addons_staging_dir();
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join(format!("{}.zip", uuid::Uuid::new_v4())))
}

#[tauri::command]
pub async fn addons_available() -> Option<bool> {
    let mut request = crate::http().get(format!("{}/api/catalog/gate", spectra::SITE)).header("origin", spectra::ORIGIN);
    if let Some(token) = spectra::stored_token() {
        request = request.bearer_auth(token);
    }
    let response = request.send().await.ok()?;
    Some(response.status().is_success())
}

#[tauri::command]
pub async fn addons_list() -> AppResult<Vec<AddonInfo>> {
    crate::blocking(|| Ok(load_state()?.into_iter().map(describe).collect())).await
}

#[tauri::command]
pub async fn addons_catalog(query: String) -> AppResult<serde_json::Value> {
    let q = query.chars().take(100).collect::<String>();
    let mut request = crate::http()
        .get(format!("{}/api/catalog/search", spectra::SITE))
        .header("origin", spectra::ORIGIN)
        .query(&[("type", "addon"), ("q", q.as_str()), ("limit", "40")]);
    if let Some(token) = spectra::stored_token() {
        request = request.bearer_auth(token);
    }
    let response = request.send().await.map_err(|e| AppError::network(format!("network error: {e}")))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND || response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Ok(serde_json::json!({ "hits": [], "closed": true }));
    }
    if !response.status().is_success() {
        return Err(AppError::network(format!("request failed ({})", response.status())));
    }
    let body = response.json().await.map_err(|e| AppError::network(format!("bad server reply: {e}")))?;
    Ok(only_addons(body))
}

#[tauri::command]
pub async fn addons_stage_catalog(state: State<'_, AppState>, slug: String) -> AppResult<Preview> {
    if !valid_id(&slug) {
        return Err(AppError::invalid("not an addon address"));
    }
    let response: CatalogResponse = catalog_get(&format!("/api/catalog/project/{slug}"))
        .await?
        .json()
        .await
        .map_err(|e| AppError::network(format!("bad server reply: {e}")))?;
    if response.project.kind != "addon" {
        return Err(AppError::invalid("this project is not a launcher addon"));
    }
    let (_, file) = pick_version(&response.project.versions, &launcher_version())
        .ok_or_else(|| AppError::new("unsupported", "No version of this addon works with this launcher."))?;

    let path = staging_file()?;
    if let Err(e) = download(file, &path).await {
        let _ = std::fs::remove_file(&path);
        return Err(e);
    }

    let staged = Staged {
        package: Package::Zip(path),
        source: "catalog",
        project: Some(response.project.slug),
        icon: response.project.icon,
        sha512: Some(file.hashes.sha512.to_ascii_lowercase()),
        dev_path: None,
        owned: true,
    };
    stage(&state, staged)
}

#[tauri::command]
pub async fn addons_check_updates() -> AppResult<Vec<UpdateInfo>> {
    let installed = crate::blocking(load_state).await?;
    let launcher = launcher_version();
    let mut updates = Vec::new();
    for entry in installed.into_iter().filter(|a| a.source == "catalog") {
        let Some(project) = entry.project.clone() else { continue };
        let Ok(response) = catalog_get(&format!("/api/catalog/project/{project}")).await else { continue };
        let Ok(parsed) = response.json::<CatalogResponse>().await else { continue };
        if parsed.project.kind != "addon" {
            continue;
        }
        let granted = read_manifest(&Package::Folder(paths::addons_dir().join(&entry.id)))
            .map(|m| m.permissions)
            .unwrap_or_default();
        if let Some(update) = update_for(&entry, &granted, &parsed.project.versions, &launcher) {
            updates.push(update);
        }
    }
    Ok(updates)
}

#[tauri::command]
pub async fn addons_stage_file(state: State<'_, AppState>, path: String) -> AppResult<Preview> {
    ensure_dev_mode()?;
    let copy = staging_file()?;
    std::fs::copy(&path, &copy)?;
    stage(
        &state,
        Staged {
            package: Package::Zip(copy),
            source: "file",
            project: None,
            icon: None,
            sha512: None,
            dev_path: None,
            owned: true,
        },
    )
}

#[tauri::command]
pub async fn addons_stage_folder(state: State<'_, AppState>, path: String) -> AppResult<Preview> {
    ensure_dev_mode()?;
    let folder = PathBuf::from(&path);
    if !folder.is_dir() {
        return Err(AppError::invalid("pick a folder that holds addon.json"));
    }
    stage(
        &state,
        Staged {
            package: Package::Folder(folder),
            source: "folder",
            project: None,
            icon: None,
            sha512: None,
            dev_path: Some(path),
            owned: false,
        },
    )
}

#[tauri::command]
pub async fn addons_commit(state: State<'_, AppState>, token: String) -> AppResult<Installed> {
    let staged = state
        .staged_addons
        .lock()
        .map_err(|_| AppError::busy("addon staging is locked"))?
        .remove(&token)
        .ok_or_else(|| AppError::not_found("this install has expired, start it again"))?;
    let result = crate::blocking({
        let staged = staged.clone();
        move || install(&staged)
    })
    .await;
    if result.is_err() {
        discard_staged(&staged);
    }
    result
}

#[tauri::command]
pub fn addons_discard(state: State<'_, AppState>, token: String) {
    if let Some(staged) = state.staged_addons.lock().ok().and_then(|mut map| map.remove(&token)) {
        discard_staged(&staged);
    }
}

#[tauri::command]
pub async fn addons_reload(id: String) -> AppResult<Installed> {
    ensure_dev_mode()?;
    crate::blocking(move || {
        let entry = load_state()?
            .into_iter()
            .find(|a| a.id == id)
            .ok_or_else(|| AppError::not_found("this addon is not installed"))?;
        let folder = entry.dev_path.clone().ok_or_else(|| AppError::invalid("only a folder addon can be reloaded"))?;
        install(&Staged {
            package: Package::Folder(PathBuf::from(folder)),
            source: "folder",
            project: None,
            icon: None,
            sha512: None,
            dev_path: entry.dev_path,
            owned: false,
        })
    })
    .await
}

#[tauri::command]
pub async fn addons_set_enabled(app: tauri::AppHandle, id: String, enabled: bool) -> AppResult<()> {
    if !enabled {
        crate::commands::addon_host::close_windows(&app, &id);
        crate::commands::addon_host::forget_backend(&id);
    }
    crate::blocking(move || {
        let mut state = load_state()?;
        let entry = state
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or_else(|| AppError::not_found("this addon is not installed"))?;
        entry.enabled = enabled;
        save_state(state)
    })
    .await
}

#[tauri::command]
pub async fn addons_uninstall(app: tauri::AppHandle, id: String) -> AppResult<()> {
    crate::commands::addon_host::close_windows(&app, &id);
    crate::commands::addon_host::forget_backend(&id);
    crate::blocking(move || {
        if !valid_id(&id) {
            return Err(AppError::invalid("not an addon id"));
        }
        let mut state = load_state()?;
        state.retain(|a| a.id != id);
        save_state(state)?;
        let dir = paths::addons_dir().join(&id);
        if dir.exists() {
            std::fs::remove_dir_all(dir)?;
        }
        crate::commands::addon_host::forget_storage(&id);
        Ok(())
    })
    .await
}

#[tauri::command]
pub fn addons_open_folder() -> AppResult<()> {
    let dir = paths::addons_dir();
    std::fs::create_dir_all(&dir)?;
    crate::commands::instances::open_in_file_manager(&dir)
}

#[tauri::command]
pub fn take_pending_addon(state: State<'_, AppState>) -> Option<String> {
    state.pending_addon.lock().ok()?.take()
}

pub fn open_from_link(app: &tauri::AppHandle, slug: String) {
    use tauri::Manager;
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut pending) = state.pending_addon.lock() {
            *pending = Some(slug.clone());
        }
    }
    let _ = app.emit("addon://open", &slug);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn manifest(json: serde_json::Value) -> Manifest {
        serde_json::from_value(json).unwrap()
    }

    fn files(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    fn theme_addon() -> serde_json::Value {
        serde_json::json!({
            "id": "night-sky",
            "name": "Night Sky",
            "version": "1.0.0",
            "api": 1,
            "contributes": {
                "themes": [{ "id": "night", "name": "Night", "file": "themes/night.json" }],
                "locales": { "pl": "locales/pl.json" },
                "buttons": [{
                    "id": "discord", "slot": "sidebar.menu", "title": "%discord%", "icon": "icons/discord.svg",
                    "action": { "type": "url", "url": "https://discord.gg/spectra" }
                }]
            }
        })
    }

    fn theme_files() -> BTreeSet<String> {
        files(&["addon.json", "themes/night.json", "locales/pl.json", "icons/discord.svg", "bg.webp"])
    }

    fn rejects(json: serde_json::Value, names: &[&str], needle: &str) {
        let err = validate(&manifest(json), &files(names)).unwrap_err();
        assert!(err.contains(needle), "{err} should mention {needle}");
    }

    fn with(mut base: serde_json::Value, key: &str, value: serde_json::Value) -> serde_json::Value {
        base[key] = value;
        base
    }

    fn base() -> serde_json::Value {
        serde_json::json!({ "id": "my-addon", "name": "My addon", "version": "1.0.0", "api": 1, "main": "main.js" })
    }

    #[test]
    fn a_theme_and_link_addon_is_valid_and_needs_no_code() {
        let m = manifest(theme_addon());
        validate(&m, &theme_files()).unwrap();
        assert!(!runs_code(&m));
    }

    #[test]
    fn anything_with_a_view_or_script_runs_code() {
        let mut code = theme_addon();
        code["contributes"]["pages"] = serde_json::json!([{ "id": "p", "title": "P", "entry": "p.html" }]);
        assert!(runs_code(&manifest(code)));
        assert!(runs_code(&manifest(base())));
    }

    #[test]
    fn identity_rules_match_the_website() {
        for id in ["Big", "-dash", "dash-", "", "a_b"] {
            rejects(with(base(), "id", id.into()), &["main.js"], "id");
        }
        rejects(with(base(), "version", "1.0 beta".into()), &["main.js"], "version");
        rejects(with(base(), "api", 2.into()), &["main.js"], "api");
        rejects(with(base(), "launcher", "not a range".into()), &["main.js"], "launcher");
    }

    #[test]
    fn paths_stay_inside_the_package() {
        for path in ["../main.js", "/main.js", "C:/main.js", "a\\main.js"] {
            rejects(with(base(), "main", path.into()), &["main.js"], "inside the archive");
        }
        rejects(with(base(), "main", "missing.js".into()), &["main.js"], "not in the archive");
        rejects(with(base(), "main", "main.exe".into()), &["main.exe"], ".js");
    }

    #[test]
    fn permissions_are_a_closed_list_plus_named_hosts() {
        let ok = with(base(), "permissions", serde_json::json!(["instances:read", "network:api.example.com"]));
        validate(&manifest(ok), &files(&["main.js"])).unwrap();
        for bad in ["fs:write", "network:*", "network:localhost", "network:https://x.com", "network:*.x.com"] {
            rejects(with(base(), "permissions", serde_json::json!([bad])), &["main.js"], "permission");
        }
    }

    #[test]
    fn buttons_need_a_known_slot_and_a_safe_action() {
        let button = |slot: &str, action: serde_json::Value| {
            let mut m = theme_addon();
            m["contributes"]["buttons"] = serde_json::json!([{ "id": "b", "slot": slot, "title": "B", "action": action }]);
            m
        };
        for slot in SLOTS {
            let ok = button(slot, serde_json::json!({ "type": "url", "url": "https://x.com" }));
            validate(&manifest(ok), &theme_files()).unwrap();
        }
        let names = ["addon.json", "themes/night.json", "locales/pl.json"];
        rejects(button("everywhere", serde_json::json!({ "type": "url", "url": "https://x.com" })), &names, "not a place");
        for url in ["http://x.com", "javascript:alert(1)", "https://u:p@x.com", "file:///C:/x"] {
            rejects(button("titlebar", serde_json::json!({ "type": "url", "url": url })), &names, "url");
        }
        rejects(button("titlebar", serde_json::json!({ "type": "page", "page": "nope" })), &names, "page the addon does not have");
        rejects(button("titlebar", serde_json::json!({ "type": "window", "window": "nope" })), &names, "window the addon does not have");
        rejects(button("titlebar", serde_json::json!({ "type": "command", "command": "go" })), &names, "needs main");
    }

    #[test]
    fn an_empty_addon_is_refused() {
        let mut m = base();
        m.as_object_mut().unwrap().remove("main");
        rejects(m, &[], "contribute anything");
    }

    #[test]
    fn launcher_ranges_use_semver_requirements() {
        let v = |s: &str| semver::Version::parse(s).unwrap();
        assert!(fits_launcher(None, &v("0.9.0")));
        assert!(!fits_launcher(Some(">=0.10.0"), &v("0.9.0")));
        assert!(fits_launcher(Some(">=0.10.0"), &v("0.10.1")));
        assert!(fits_launcher(Some(">=0.9.0, <1.0.0"), &v("0.9.5")));
        assert!(!fits_launcher(Some("garbage"), &v("0.9.0")));
    }

    #[test]
    fn deep_links_name_an_addon_by_slug() {
        assert_eq!(slug_from_url("spectra://addon/better-stats").as_deref(), Some("better-stats"));
        assert_eq!(slug_from_url("spectra://addon/better-stats/").as_deref(), Some("better-stats"));
        assert_eq!(slug_from_url("spectra://addon/../x"), None);
        assert_eq!(slug_from_url("spectra://addon/Big"), None);
        assert_eq!(slug_from_url("spectra://launch/abc"), None);
    }

    #[test]
    fn the_newest_version_this_launcher_can_run_wins() {
        let version = |api: u64, launcher: Option<&str>, id: &str| CatalogVersion {
            number: id.into(),
            meta: serde_json::json!({ "api": api, "launcher": launcher }),
            files: vec![
                CatalogFile { id: format!("{id}-extra"), size: 1, primary: false, hashes: CatalogHashes { sha512: "a".into() } },
                CatalogFile { id: id.into(), size: 1, primary: true, hashes: CatalogHashes { sha512: "b".into() } },
            ],
        };
        let versions = vec![
            version(2, None, "future-api"),
            version(1, Some(">=5.0.0"), "future-launcher"),
            version(1, None, "fits"),
            version(1, None, "older"),
        ];
        let v = semver::Version::parse("0.9.0").unwrap();
        assert_eq!(pick_version(&versions, &v).unwrap().1.id, "fits");
        assert!(pick_version(&versions[..2], &v).is_none());
    }

    fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let mut writer = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        for (name, bytes) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();
    }

    fn theme_zip(path: &Path, theme: &[u8]) {
        let manifest = serde_json::to_vec(&theme_addon()).unwrap();
        let locale = serde_json::to_vec(&serde_json::json!({ "discord": "Nasz Discord" })).unwrap();
        write_zip(path, &[
            ("addon.json", &manifest),
            ("themes/night.json", theme),
            ("locales/pl.json", &locale),
            ("icons/discord.svg", b"<svg xmlns='http://www.w3.org/2000/svg'/>"),
            ("bg.webp", b"RIFF"),
        ]);
    }

    fn temp_root() -> PathBuf {
        let root = std::env::temp_dir().join(format!("spectra-addons-{}", uuid::Uuid::new_v4()));
        std::env::set_var("SPECTRA_DATA_DIR", &root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn staged(path: &Path, owned: bool) -> Staged {
        Staged {
            package: Package::Zip(path.to_path_buf()),
            source: "file",
            project: None,
            icon: None,
            sha512: None,
            dev_path: None,
            owned,
        }
    }

    #[test]
    fn a_package_installs_lists_and_keeps_its_switch_on_update() {
        let _guard = paths::lock_data_dir();
        let root = temp_root();

        let zip_path = root.join("night.zip");
        let theme = serde_json::to_vec(&serde_json::json!({ "mode": "oled", "accent": "indigo", "background": "bg.webp" })).unwrap();
        theme_zip(&zip_path, &theme);

        let shown = preview("t", &staged(&zip_path, true)).unwrap();
        assert_eq!(shown.links, vec!["discord.gg".to_string()]);
        assert_eq!(shown.themes, vec!["Night".to_string()]);

        let installed = install(&staged(&zip_path, true)).unwrap();
        assert_eq!(installed.id, "night-sky");
        assert!(installed.enabled);
        assert!(!zip_path.exists());

        let listed: Vec<AddonInfo> = load_state().unwrap().into_iter().map(describe).collect();
        assert_eq!(listed.len(), 1);
        let info = &listed[0];
        assert_eq!(info.error, None);
        assert_eq!(info.themes[0].accent.as_deref(), Some("indigo"));
        assert!(Path::new(info.themes[0].background.as_deref().unwrap()).is_file());
        assert!(Path::new(info.buttons[0].icon.as_deref().unwrap()).is_file());
        assert_eq!(info.locales["pl"]["discord"], "Nasz Discord");

        let mut state = load_state().unwrap();
        state[0].enabled = false;
        save_state(state).unwrap();
        theme_zip(&zip_path, b"{\"mode\":\"dark\"}");
        let again = install(&staged(&zip_path, true)).unwrap();
        assert!(!again.enabled);
        assert_eq!(load_state().unwrap().len(), 1);

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn broken_packages_never_reach_the_addons_folder() {
        let _guard = paths::lock_data_dir();
        let root = temp_root();

        let bad_theme = root.join("bad-theme.zip");
        theme_zip(&bad_theme, b"{\"accent\":\"chartreuse\"}");
        assert!(install(&staged(&bad_theme, false)).unwrap_err().message.contains("accent"));

        let traversal = root.join("traversal.zip");
        write_zip(&traversal, &[("addon.json", b"{}"), ("../evil.txt", b"x")]);
        assert!(install(&staged(&traversal, false)).unwrap_err().message.contains("outside"));

        let too_new = root.join("too-new.zip");
        let manifest = serde_json::to_vec(&with(base(), "launcher", ">=99.0.0".into())).unwrap();
        write_zip(&too_new, &[("addon.json", &manifest), ("main.js", b"")]);
        assert_eq!(install(&staged(&too_new, false)).unwrap_err().code, "unsupported");

        let not_zip = root.join("plain.zip");
        std::fs::write(&not_zip, b"not a zip").unwrap();
        assert!(install(&staged(&not_zip, false)).is_err());

        assert!(load_state().unwrap().is_empty());
        assert!(!paths::addons_dir().join("night-sky").exists());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_folder_package_skips_git_and_dependencies() {
        let root = std::env::temp_dir().join(format!("spectra-addon-dir-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::create_dir_all(root.join("node_modules/x")).unwrap();
        std::fs::create_dir_all(root.join("themes")).unwrap();
        std::fs::write(root.join(".git/HEAD"), "ref").unwrap();
        std::fs::write(root.join("node_modules/x/index.js"), "").unwrap();
        std::fs::write(root.join("themes/night.json"), "{}").unwrap();
        std::fs::write(root.join("addon.json"), "{}").unwrap();

        let listed = Package::Folder(root.clone()).files().unwrap();
        assert_eq!(listed, files(&["addon.json", "themes/night.json"]));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn safe_names_are_plain_relative_paths() {
        for ok in ["addon.json", "ui/a.html", "themes/x/y.json"] {
            assert!(safe_name(ok), "{ok}");
        }
        for bad in ["", "/abs", "../x", "a/../b", "a\\b", "C:/x", "a//b", "./a", "a\u{0}b"] {
            assert!(!safe_name(bad), "{bad:?}");
        }
    }

    #[test]
    fn an_update_is_a_different_file_and_names_what_it_newly_asks_for() {
        let installed = Installed {
            id: "stats".into(),
            version: "1.0.0".into(),
            source: "catalog".into(),
            project: Some("stats".into()),
            icon: None,
            sha512: Some("AAA".into()),
            dev_path: None,
            enabled: true,
            installed_at: String::new(),
        };
        let version = |number: &str, sha: &str, permissions: serde_json::Value| CatalogVersion {
            number: number.into(),
            meta: serde_json::json!({ "api": 1, "permissions": permissions }),
            files: vec![CatalogFile { id: "f".into(), size: 1, primary: true, hashes: CatalogHashes { sha512: sha.into() } }],
        };
        let v = semver::Version::parse("0.9.0").unwrap();
        let granted = vec!["instances:read".to_string()];

        assert!(update_for(&installed, &granted, &[version("1.0.0", "aaa", serde_json::json!(["instances:read"]))], &v).is_none());

        let newer = [version("1.1.0", "bbb", serde_json::json!(["instances:read", "logs:read"]))];
        let update = update_for(&installed, &granted, &newer, &v).unwrap();
        assert_eq!((update.current.as_str(), update.latest.as_str()), ("1.0.0", "1.1.0"));
        assert_eq!(update.new_permissions, vec!["logs:read".to_string()]);

        let from_file = Installed { project: None, ..installed };
        assert!(update_for(&from_file, &granted, &newer, &v).is_none());
    }

    #[test]
    fn the_catalog_list_keeps_only_addons() {
        let body = serde_json::json!({
            "hits": [{ "slug": "a", "type": "addon" }, { "slug": "m", "type": "mod" }, { "slug": "x" }],
            "total": 3,
        });
        let kept = only_addons(body);
        let slugs: Vec<&str> = kept["hits"].as_array().unwrap().iter().map(|h| h["slug"].as_str().unwrap()).collect();
        assert_eq!(slugs, vec!["a"]);
        assert_eq!(only_addons(serde_json::json!({ "hits": [], "closed": true }))["closed"], true);
    }

    #[test]
    fn the_example_addon_passes_every_check() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/sandbox-check");
        let inspected = inspect(&Package::Folder(dir)).unwrap();
        check_runs_here(&inspected.manifest).unwrap();
        assert!(runs_code(&inspected.manifest));
        assert_eq!(inspected.themes[0].background.as_deref(), Some("themes/background.png"));
        assert_eq!(inspected.locales["pl"]["check"], "Test piaskownicy");
        let slots: BTreeSet<&str> = inspected.manifest.contributes.buttons.iter().map(|b| b.slot.as_str()).collect();
        for slot in ["sidebar.menu", "titlebar", "instance.menu", "instance.header", "home.header"] {
            assert!(slots.contains(slot), "{slot}");
        }
    }
}
