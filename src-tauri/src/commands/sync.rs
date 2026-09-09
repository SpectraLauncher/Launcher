use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

use crate::error::{AppError, AppResult};
use crate::models::Instance;
use crate::{paths, store};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncOption {
    GameOptions,
    MultiplayerServers,
    CommandHistory,
    CreativeHotbars,
    ResourcePacks,
}

impl SyncOption {
    pub const ALL: [SyncOption; 5] = [
        SyncOption::GameOptions,
        SyncOption::MultiplayerServers,
        SyncOption::CommandHistory,
        SyncOption::CreativeHotbars,
        SyncOption::ResourcePacks,
    ];

    fn key(self) -> &'static str {
        match self {
            SyncOption::GameOptions => "game_options",
            SyncOption::MultiplayerServers => "multiplayer_servers",
            SyncOption::CommandHistory => "command_history",
            SyncOption::CreativeHotbars => "creative_hotbars",
            SyncOption::ResourcePacks => "resource_packs",
        }
    }

    fn version_sensitive(self) -> bool {
        matches!(self, SyncOption::CreativeHotbars)
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SyncFlags {
    pub game_options: bool,
    pub multiplayer_servers: bool,
    pub command_history: bool,
    pub creative_hotbars: bool,
    pub resource_packs: bool,
}

impl SyncFlags {
    fn get(self, option: SyncOption) -> bool {
        match option {
            SyncOption::GameOptions => self.game_options,
            SyncOption::MultiplayerServers => self.multiplayer_servers,
            SyncOption::CommandHistory => self.command_history,
            SyncOption::CreativeHotbars => self.creative_hotbars,
            SyncOption::ResourcePacks => self.resource_packs,
        }
    }

    fn set(&mut self, option: SyncOption, value: bool) {
        match option {
            SyncOption::GameOptions => self.game_options = value,
            SyncOption::MultiplayerServers => self.multiplayer_servers = value,
            SyncOption::CommandHistory => self.command_history = value,
            SyncOption::CreativeHotbars => self.creative_hotbars = value,
            SyncOption::ResourcePacks => self.resource_packs = value,
        }
    }

    fn any(self) -> bool {
        SyncOption::ALL.iter().any(|o| self.get(*o))
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SyncState {
    pub global: SyncFlags,
    pub excluded: HashMap<String, SyncFlags>,
    pub seeded_from: BTreeMap<String, String>,
}

impl SyncState {
    pub fn participates(&self, instance_id: &str, option: SyncOption) -> bool {
        self.global.get(option)
            && !self.excluded.get(instance_id).copied().unwrap_or_default().get(option)
    }

    fn seed_version(&self, option: SyncOption) -> Option<&str> {
        self.seeded_from.get(option.key()).map(String::as_str)
    }
}

pub fn load_state() -> AppResult<SyncState> {
    Ok(store::read_json::<SyncState>(&paths::sync_state_file())?.unwrap_or_default())
}

fn save_state(state: &SyncState) -> AppResult<()> {
    store::write_json(&paths::sync_state_file(), state)
}

pub fn flattening_era(mc_version: &str) -> bool {
    let mut parts = mc_version.split('.');
    let Some("1") = parts.next() else { return true };
    match parts.next().and_then(|m| m.parse::<u32>().ok()) {
        Some(minor) => minor >= 13,
        None => true,
    }
}

fn era_matches(state: &SyncState, option: SyncOption, mc_version: &str) -> bool {
    if !option.version_sensitive() {
        return true;
    }
    match state.seed_version(option) {
        Some(seed) => flattening_era(seed) == flattening_era(mc_version),
        None => true,
    }
}

const SYNCED_OPTION_KEYS: [&str; 30] = [
    "autoJump",
    "bobView",
    "chatOpacity",
    "chatScale",
    "chatVisibility",
    "chatWidth",
    "darkMojangStudiosBackground",
    "discrete_mouse_scroll",
    "enableVsync",
    "entityShadows",
    "fov",
    "fullscreen",
    "gamma",
    "guiScale",
    "hideLightningFlashes",
    "invertYMouse",
    "lang",
    "maxFps",
    "mouseSensitivity",
    "particles",
    "renderDistance",
    "simulationDistance",
    "soundCategory_ambient",
    "soundCategory_block",
    "soundCategory_hostile",
    "soundCategory_master",
    "soundCategory_music",
    "soundCategory_neutral",
    "soundCategory_player",
    "soundCategory_record",
];

fn is_synced_option_key(key: &str, era: bool) -> bool {
    if key.starts_with("key_") {
        return era;
    }
    SYNCED_OPTION_KEYS.contains(&key)
}

fn read_options(path: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Ok(text) = std::fs::read_to_string(path) else { return out };
    for line in text.lines() {
        if let Some((key, value)) = line.split_once(':') {
            out.insert(key.to_string(), value.to_string());
        }
    }
    out
}

fn write_options(path: &Path, values: &BTreeMap<String, String>) -> AppResult<()> {
    let body: String = values.iter().map(|(k, v)| format!("{k}:{v}\n")).collect();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("options dir: {e}"))?;
    }
    std::fs::write(path, body).map_err(|e| format!("write options.txt: {e}").into())
}

fn pull_options(id: &str, mc_version: &str) -> AppResult<()> {
    let shared: BTreeMap<String, String> =
        store::read_json(&paths::sync_options_file())?.unwrap_or_default();
    if shared.is_empty() {
        return Ok(());
    }
    let era = flattening_era(mc_version);
    let path = paths::instance_game_dir(id).join("options.txt");
    let mut local = read_options(&path);
    for (key, value) in shared.iter().filter(|(k, _)| is_synced_option_key(k, era)) {
        local.insert(key.clone(), value.clone());
    }
    write_options(&path, &local)
}

fn push_options(id: &str, mc_version: &str) -> AppResult<()> {
    let path = paths::instance_game_dir(id).join("options.txt");
    let local = read_options(&path);
    if local.is_empty() {
        return Ok(());
    }
    let era = flattening_era(mc_version);
    let mut shared: BTreeMap<String, String> =
        store::read_json(&paths::sync_options_file())?.unwrap_or_default();
    for (key, value) in local.iter().filter(|(k, _)| is_synced_option_key(k, era)) {
        shared.insert(key.clone(), value.clone());
    }
    store::write_json(&paths::sync_options_file(), &shared)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SyncedServer {
    pub name: String,
    pub ip: String,
    #[serde(default)]
    pub accept_textures: Option<i8>,
    #[serde(default)]
    pub hidden: Option<i8>,
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
    #[serde(rename = "acceptTextures")]
    accept_textures: Option<i8>,
    hidden: Option<i8>,
}

fn read_instance_servers(id: &str) -> Vec<SyncedServer> {
    let path = paths::instance_game_dir(id).join("servers.dat");
    let Ok(bytes) = std::fs::read(&path) else { return Vec::new() };
    let Ok(parsed) = fastnbt::from_bytes::<ServersFile>(&bytes) else { return Vec::new() };
    parsed
        .servers
        .into_iter()
        .filter_map(|s| {
            let ip = s.ip?;
            if ip.trim().is_empty() {
                return None;
            }
            Some(SyncedServer {
                name: s.name.unwrap_or_default(),
                ip,
                accept_textures: s.accept_textures,
                hidden: s.hidden,
            })
        })
        .collect()
}

fn write_instance_servers(id: &str, servers: &[SyncedServer]) -> AppResult<()> {
    use fastnbt::Value;

    let game_dir = paths::instance_game_dir(id);
    let list: Vec<Value> = servers
        .iter()
        .map(|s| {
            let mut entry = HashMap::new();
            entry.insert("name".to_string(), Value::String(s.name.clone()));
            entry.insert("ip".to_string(), Value::String(s.ip.clone()));
            if let Some(v) = s.accept_textures {
                entry.insert("acceptTextures".to_string(), Value::Byte(v));
            }
            if let Some(v) = s.hidden {
                entry.insert("hidden".to_string(), Value::Byte(v));
            }
            Value::Compound(entry)
        })
        .collect();

    let mut root = HashMap::new();
    root.insert("servers".to_string(), Value::List(list));

    std::fs::create_dir_all(&game_dir).map_err(|e| format!("game dir: {e}"))?;
    let bytes = fastnbt::to_bytes(&Value::Compound(root))
        .map_err(|e| format!("encode servers.dat: {e}"))?;
    std::fs::write(game_dir.join("servers.dat"), bytes)
        .map_err(|e| format!("write servers.dat: {e}").into())
}

fn merge_servers(shared: &mut Vec<SyncedServer>, local: Vec<SyncedServer>) {
    for server in local {
        if !shared.iter().any(|s| s.ip.eq_ignore_ascii_case(&server.ip)) {
            shared.push(server);
        }
    }
}

fn pull_servers(id: &str) -> AppResult<()> {
    let shared: Vec<SyncedServer> =
        store::read_json(&paths::sync_servers_file())?.unwrap_or_default();
    if shared.is_empty() {
        return Ok(());
    }
    write_instance_servers(id, &shared)
}

fn push_servers(id: &str) -> AppResult<()> {
    let mut shared: Vec<SyncedServer> =
        store::read_json(&paths::sync_servers_file())?.unwrap_or_default();
    merge_servers(&mut shared, read_instance_servers(id));
    store::write_json(&paths::sync_servers_file(), &shared)
}

const HISTORY_LIMIT: usize = 1000;

fn merge_history(shared: &str, local: &str) -> String {
    let mut lines: Vec<&str> = shared.lines().collect();
    for line in local.lines() {
        if !lines.contains(&line) {
            lines.push(line);
        }
    }
    if lines.len() > HISTORY_LIMIT {
        lines.drain(0..lines.len() - HISTORY_LIMIT);
    }
    let mut out = lines.join("\n");
    if !out.is_empty() {
        out.push('\n');
    }
    out
}

fn pull_history(id: &str) -> AppResult<()> {
    let Ok(shared) = std::fs::read_to_string(paths::sync_history_file()) else { return Ok(()) };
    let game_dir = paths::instance_game_dir(id);
    std::fs::create_dir_all(&game_dir).map_err(|e| format!("game dir: {e}"))?;
    std::fs::write(game_dir.join("command_history.txt"), shared)
        .map_err(|e| format!("write command history: {e}").into())
}

fn push_history(id: &str) -> AppResult<()> {
    let local = std::fs::read_to_string(
        paths::instance_game_dir(id).join("command_history.txt"),
    )
    .unwrap_or_default();
    if local.is_empty() {
        return Ok(());
    }
    let shared = std::fs::read_to_string(paths::sync_history_file()).unwrap_or_default();
    let merged = merge_history(&shared, &local);
    std::fs::create_dir_all(paths::shared_sync_dir()).map_err(|e| format!("shared dir: {e}"))?;
    std::fs::write(paths::sync_history_file(), merged)
        .map_err(|e| format!("write shared history: {e}").into())
}

fn pull_hotbar(id: &str) -> AppResult<()> {
    let shared = paths::sync_hotbar_file();
    if !shared.is_file() {
        return Ok(());
    }
    let game_dir = paths::instance_game_dir(id);
    std::fs::create_dir_all(&game_dir).map_err(|e| format!("game dir: {e}"))?;
    std::fs::copy(&shared, game_dir.join("hotbar.nbt"))
        .map(|_| ())
        .map_err(|e| format!("write hotbar: {e}").into())
}

fn push_hotbar(id: &str) -> AppResult<()> {
    let local = paths::instance_game_dir(id).join("hotbar.nbt");
    if !local.is_file() {
        return Ok(());
    }
    std::fs::create_dir_all(paths::shared_sync_dir()).map_err(|e| format!("shared dir: {e}"))?;
    std::fs::copy(&local, paths::sync_hotbar_file())
        .map(|_| ())
        .map_err(|e| format!("store hotbar: {e}").into())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncedPack {
    pub id: String,
    pub filename: String,
    #[serde(default)]
    pub pack_format: Option<i64>,
    #[serde(default = "yes")]
    pub enabled: bool,
}

fn yes() -> bool {
    true
}

const PACK_FORMAT_BY_VERSION: [(u32, i64); 17] = [
    (8, 1),
    (10, 2),
    (12, 3),
    (14, 4),
    (16, 6),
    (17, 7),
    (18, 8),
    (19, 13),
    (20, 34),
    (21, 63),
    (22, 99),
    (23, 99),
    (24, 99),
    (25, 99),
    (26, 99),
    (27, 99),
    (28, 99),
];

fn max_pack_format(mc_version: &str) -> i64 {
    let mut parts = mc_version.split('.');
    let Some("1") = parts.next() else { return i64::MAX };
    let Some(minor) = parts.next().and_then(|m| m.parse::<u32>().ok()) else {
        return i64::MAX;
    };
    PACK_FORMAT_BY_VERSION
        .iter()
        .find(|(v, _)| minor <= *v)
        .map(|(_, f)| *f)
        .unwrap_or(i64::MAX)
}

fn pack_fits(pack: &SyncedPack, mc_version: &str) -> bool {
    match pack.pack_format {
        Some(format) => format <= max_pack_format(mc_version),
        None => true,
    }
}

fn read_pack_format(path: &Path) -> Option<i64> {
    let file = std::fs::File::open(path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;
    let mut text = String::new();
    archive.by_name("pack.mcmeta").ok()?.read_to_string(&mut text).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    value["pack"]["pack_format"].as_i64()
}

fn file_sha1(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let mut hash = Sha1::new();
    hash.update(&bytes);
    Some(format!("{:x}", hash.finalize()))
}

pub fn load_packs() -> AppResult<Vec<SyncedPack>> {
    Ok(store::read_json::<Vec<SyncedPack>>(&paths::sync_packs_file())?.unwrap_or_default())
}

fn save_packs(packs: &[SyncedPack]) -> AppResult<()> {
    store::write_json(&paths::sync_packs_file(), &packs.to_vec())
}

fn instance_pack_dir(id: &str) -> PathBuf {
    paths::instance_game_dir(id).join("resourcepacks")
}

fn push_packs(id: &str) -> AppResult<()> {
    let dir = instance_pack_dir(id);
    let Ok(entries) = std::fs::read_dir(&dir) else { return Ok(()) };
    let mut packs = load_packs()?;
    let store_dir = paths::sync_packs_dir();

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let filename = entry.file_name().to_string_lossy().into_owned();
        if !filename.to_lowercase().ends_with(".zip") {
            continue;
        }
        let Some(hash) = file_sha1(&path) else { continue };
        if packs.iter().any(|p| p.id == hash) {
            continue;
        }
        std::fs::create_dir_all(&store_dir).map_err(|e| format!("pack store: {e}"))?;
        let target = store_dir.join(format!("{hash}.zip"));
        if !target.is_file() && std::fs::copy(&path, &target).is_err() {
            continue;
        }
        packs.push(SyncedPack {
            id: hash,
            pack_format: read_pack_format(&path),
            filename,
            enabled: true,
        });
    }
    save_packs(&packs)
}

fn pull_packs(id: &str, mc_version: &str) -> AppResult<()> {
    let packs = load_packs()?;
    if packs.is_empty() {
        return Ok(());
    }
    let dir = instance_pack_dir(id);
    std::fs::create_dir_all(&dir).map_err(|e| format!("resourcepacks dir: {e}"))?;

    let era = flattening_era(mc_version);
    let mut selected = Vec::new();
    for pack in packs.iter().filter(|p| p.enabled && pack_fits(p, mc_version)) {
        let source = paths::sync_packs_dir().join(format!("{}.zip", pack.id));
        if !source.is_file() {
            continue;
        }
        let target = dir.join(&pack.filename);
        if !target.is_file() && std::fs::copy(&source, &target).is_err() {
            continue;
        }
        selected.push(if era {
            format!("\"file/{}\"", pack.filename)
        } else {
            format!("\"{}\"", pack.filename)
        });
    }

    let path = paths::instance_game_dir(id).join("options.txt");
    let mut options = read_options(&path);
    selected.insert(0, "\"vanilla\"".to_string());
    options.insert("resourcePacks".to_string(), format!("[{}]", selected.join(",")));
    write_options(&path, &options)
}

fn instance_of(id: &str) -> AppResult<Instance> {
    store::read_json::<Instance>(&paths::instance_config_file(id))?
        .ok_or_else(|| AppError::not_found(format!("instance '{id}' not found")))
}

fn apply(id: &str, mc_version: &str, option: SyncOption, out: bool) -> AppResult<()> {
    match (option, out) {
        (SyncOption::GameOptions, false) => pull_options(id, mc_version),
        (SyncOption::GameOptions, true) => push_options(id, mc_version),
        (SyncOption::MultiplayerServers, false) => pull_servers(id),
        (SyncOption::MultiplayerServers, true) => push_servers(id),
        (SyncOption::CommandHistory, false) => pull_history(id),
        (SyncOption::CommandHistory, true) => push_history(id),
        (SyncOption::CreativeHotbars, false) => pull_hotbar(id),
        (SyncOption::CreativeHotbars, true) => push_hotbar(id),
        (SyncOption::ResourcePacks, false) => pull_packs(id, mc_version),
        (SyncOption::ResourcePacks, true) => push_packs(id),
    }
}

pub fn pull(id: &str, mc_version: &str) -> AppResult<()> {
    let state = load_state()?;
    if !state.global.any() {
        return Ok(());
    }
    for option in SyncOption::ALL {
        if state.participates(id, option) && era_matches(&state, option, mc_version) {
            if let Err(e) = apply(id, mc_version, option, false) {
                log::warn!("sync pull {} for {id}: {e}", option.key());
            }
        }
    }
    Ok(())
}

pub fn push(id: &str, mc_version: &str) -> AppResult<()> {
    let state = load_state()?;
    if !state.global.any() {
        return Ok(());
    }
    for option in SyncOption::ALL {
        if state.participates(id, option) && era_matches(&state, option, mc_version) {
            if let Err(e) = apply(id, mc_version, option, true) {
                log::warn!("sync push {} for {id}: {e}", option.key());
            }
        }
    }
    Ok(())
}

fn shared_has_content(option: SyncOption) -> bool {
    match option {
        SyncOption::GameOptions => paths::sync_options_file().is_file(),
        SyncOption::MultiplayerServers => paths::sync_servers_file().is_file(),
        SyncOption::CommandHistory => paths::sync_history_file().is_file(),
        SyncOption::CreativeHotbars => paths::sync_hotbar_file().is_file(),
        SyncOption::ResourcePacks => paths::sync_packs_file().is_file(),
    }
}

fn instance_has_content(id: &str, option: SyncOption) -> bool {
    let game = paths::instance_game_dir(id);
    match option {
        SyncOption::GameOptions => game.join("options.txt").is_file(),
        SyncOption::MultiplayerServers => game.join("servers.dat").is_file(),
        SyncOption::CommandHistory => game.join("command_history.txt").is_file(),
        SyncOption::CreativeHotbars => game.join("hotbar.nbt").is_file(),
        SyncOption::ResourcePacks => std::fs::read_dir(instance_pack_dir(id))
            .map(|mut d| d.any(|e| e.is_ok()))
            .unwrap_or(false),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JoinAction {
    SeedShared,
    Attach,
    Merge,
    RequiresResolution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JoinResolution {
    UseSynced,
    UseInstance,
}

pub fn join_action(state: &SyncState, id: &str, option: SyncOption) -> JoinAction {
    let shared = shared_has_content(option);
    let local = instance_has_content(id, option);
    match (shared, local) {
        (false, _) => JoinAction::SeedShared,
        (true, false) => JoinAction::Attach,
        (true, true) if matches!(option, SyncOption::MultiplayerServers | SyncOption::CommandHistory | SyncOption::ResourcePacks) => {
            JoinAction::Merge
        }
        (true, true) => {
            let _ = state;
            JoinAction::RequiresResolution
        }
    }
}

#[derive(Serialize)]
pub struct SyncSource {
    pub id: String,
    pub name: String,
    pub mc_version: String,
    pub has_icon: bool,
}

pub fn resolve_announcement() -> bool {
    let config = paths::launcher_config_file();
    let upgraded = config.is_file() || has_instances();

    let mut settings = crate::commands::settings::load().unwrap_or_default();
    if settings.sync_announced {
        return false;
    }

    settings.sync_announced = true;
    if let Err(e) = store::write_json(&config, &settings) {
        log::warn!("could not record the sync announcement: {e}");
        return false;
    }

    upgraded && !load_state().map(|s| s.global.any()).unwrap_or(false)
}

fn has_instances() -> bool {
    std::fs::read_dir(paths::instances_dir())
        .map(|entries| {
            entries.flatten().any(|e| {
                paths::instance_config_file(&e.file_name().to_string_lossy()).is_file()
            })
        })
        .unwrap_or(false)
}

#[tauri::command]
pub fn take_sync_announcement(state: tauri::State<'_, crate::AppState>) -> bool {
    state
        .announce_sync
        .lock()
        .map(|mut flag| std::mem::replace(&mut *flag, false))
        .unwrap_or(false)
}

#[tauri::command]
pub async fn sync_get_state() -> AppResult<SyncState> {
    crate::blocking(load_state).await
}

#[tauri::command]
pub async fn sync_sources() -> AppResult<Vec<SyncSource>> {
    crate::blocking(|| {
        let mut out = Vec::new();
        let Ok(entries) = std::fs::read_dir(paths::instances_dir()) else { return Ok(out) };
        for entry in entries.flatten() {
            let id = entry.file_name().to_string_lossy().into_owned();
            if let Ok(Some(instance)) =
                store::read_json::<Instance>(&paths::instance_config_file(&id))
            {
                out.push(SyncSource {
                    has_icon: instance.icon.is_some(),
                    name: instance.name,
                    mc_version: instance.mc_version,
                    id: instance.id,
                });
            }
        }
        out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(out)
    })
    .await
}

#[tauri::command]
pub async fn sync_join_preview(instance_id: String, option: SyncOption) -> AppResult<JoinAction> {
    crate::blocking(move || {
        let state = load_state()?;
        Ok(join_action(&state, &instance_id, option))
    })
    .await
}

#[tauri::command]
pub async fn sync_set_global(
    option: SyncOption,
    enabled: bool,
    base_instance_id: Option<String>,
) -> AppResult<SyncState> {
    crate::blocking(move || {
        let mut state = load_state()?;
        state.global.set(option, enabled);

        if enabled {
            if let Some(base) = base_instance_id {
                let instance = instance_of(&base)?;
                apply(&base, &instance.mc_version, option, true)?;
                state.seeded_from.insert(option.key().to_string(), instance.mc_version);
            }
        } else {
            state.seeded_from.remove(option.key());
        }

        save_state(&state)?;
        Ok(state)
    })
    .await
}

#[tauri::command]
pub async fn sync_set_instance(
    instance_id: String,
    option: SyncOption,
    enabled: bool,
    resolution: Option<JoinResolution>,
) -> AppResult<SyncState> {
    crate::blocking(move || {
        let mut state = load_state()?;
        let instance = instance_of(&instance_id)?;

        if enabled {
            match resolution {
                Some(JoinResolution::UseInstance) => {
                    apply(&instance_id, &instance.mc_version, option, true)?;
                }
                Some(JoinResolution::UseSynced) | None => {}
            }
            state.excluded.entry(instance_id.clone()).or_default().set(option, false);
        } else {
            state.excluded.entry(instance_id.clone()).or_default().set(option, true);
        }

        save_state(&state)?;
        if enabled && state.global.get(option) {
            apply(&instance_id, &instance.mc_version, option, false)?;
        }
        Ok(state)
    })
    .await
}

#[tauri::command]
pub async fn sync_list_packs() -> AppResult<Vec<SyncedPack>> {
    crate::blocking(load_packs).await
}

#[tauri::command]
pub async fn sync_set_pack_enabled(pack_id: String, enabled: bool) -> AppResult<Vec<SyncedPack>> {
    crate::blocking(move || {
        let mut packs = load_packs()?;
        if let Some(pack) = packs.iter_mut().find(|p| p.id == pack_id) {
            pack.enabled = enabled;
        }
        save_packs(&packs)?;
        Ok(packs)
    })
    .await
}

#[tauri::command]
pub async fn sync_remove_pack(pack_id: String) -> AppResult<Vec<SyncedPack>> {
    crate::blocking(move || {
        let mut packs = load_packs()?;
        packs.retain(|p| p.id != pack_id);
        let _ = std::fs::remove_file(paths::sync_packs_dir().join(format!("{pack_id}.zip")));
        save_packs(&packs)?;
        Ok(packs)
    })
    .await
}

#[tauri::command]
pub async fn sync_open_folder() -> AppResult<String> {
    crate::blocking(|| {
        let dir = paths::shared_sync_dir();
        std::fs::create_dir_all(&dir).map_err(|e| format!("shared dir: {e}"))?;
        Ok(dir.to_string_lossy().into_owned())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flattening_era_splits_at_1_13() {
        assert!(!flattening_era("1.12.2"));
        assert!(!flattening_era("1.7.10"));
        assert!(flattening_era("1.13"));
        assert!(flattening_era("1.21.4"));
        assert!(flattening_era("24w14a"), "snapshots count as modern");
    }

    #[test]
    fn only_whitelisted_option_keys_travel_between_instances() {
        assert!(is_synced_option_key("fov", true));
        assert!(is_synced_option_key("soundCategory_master", false));
        assert!(!is_synced_option_key("version", true));
        assert!(!is_synced_option_key("resourcePacks", true));
        assert!(!is_synced_option_key("lastServer", true));

        assert!(is_synced_option_key("key_key.attack", true));
        assert!(
            !is_synced_option_key("key_key.attack", false),
            "pre-1.13 keybinds use LWJGL numbers and must not mix with modern ids"
        );
    }

    #[test]
    fn pack_format_ceiling_follows_the_game_version() {
        assert_eq!(max_pack_format("1.12.2"), 3);
        assert_eq!(max_pack_format("1.20.1"), 34);
        assert_eq!(max_pack_format("1.99"), i64::MAX);

        let modern = SyncedPack {
            id: "a".into(),
            filename: "a.zip".into(),
            pack_format: Some(34),
            enabled: true,
        };
        assert!(pack_fits(&modern, "1.20.1"));
        assert!(!pack_fits(&modern, "1.12.2"));

        let unknown = SyncedPack { pack_format: None, ..modern };
        assert!(pack_fits(&unknown, "1.12.2"), "a pack we cannot read is left to the game");
    }

    #[test]
    fn servers_merge_by_address_without_duplicating() {
        let mut shared = vec![SyncedServer {
            name: "Hub".into(),
            ip: "play.example.com".into(),
            accept_textures: None,
            hidden: None,
        }];
        merge_servers(
            &mut shared,
            vec![
                SyncedServer {
                    name: "Hub renamed".into(),
                    ip: "PLAY.EXAMPLE.COM".into(),
                    accept_textures: None,
                    hidden: None,
                },
                SyncedServer {
                    name: "Second".into(),
                    ip: "second.example.com".into(),
                    accept_textures: Some(1),
                    hidden: None,
                },
            ],
        );
        assert_eq!(shared.len(), 2);
        assert_eq!(shared[0].name, "Hub");
        assert_eq!(shared[1].name, "Second");
    }

    #[test]
    fn history_merges_new_lines_and_stays_bounded() {
        let merged = merge_history("/tp 0 0 0\n/gamemode creative\n", "/gamemode creative\n/time set day\n");
        assert_eq!(merged, "/tp 0 0 0\n/gamemode creative\n/time set day\n");

        let long: String = (0..HISTORY_LIMIT + 50).map(|i| format!("/cmd{i}\n")).collect();
        let capped = merge_history("", &long);
        assert_eq!(capped.lines().count(), HISTORY_LIMIT);
        assert_eq!(capped.lines().next().unwrap(), "/cmd50");
    }

    #[test]
    fn joining_picks_the_right_action_for_what_already_exists() {
        let guard = paths::lock_data_dir();
        let root = std::env::temp_dir().join(format!("spectra-sync-{}", uuid::Uuid::new_v4()));
        std::env::set_var("SPECTRA_DATA_DIR", &root);
        let state = SyncState::default();

        let game = paths::instance_game_dir("i1");
        std::fs::create_dir_all(&game).unwrap();

        assert_eq!(join_action(&state, "i1", SyncOption::GameOptions), JoinAction::SeedShared);

        std::fs::create_dir_all(paths::shared_sync_dir()).unwrap();
        std::fs::write(paths::sync_options_file(), "{}").unwrap();
        assert_eq!(join_action(&state, "i1", SyncOption::GameOptions), JoinAction::Attach);

        std::fs::write(game.join("options.txt"), "fov:0.5\n").unwrap();
        assert_eq!(
            join_action(&state, "i1", SyncOption::GameOptions),
            JoinAction::RequiresResolution
        );

        std::fs::write(paths::sync_servers_file(), "[]").unwrap();
        std::fs::write(game.join("servers.dat"), [0u8]).unwrap();
        assert_eq!(
            join_action(&state, "i1", SyncOption::MultiplayerServers),
            JoinAction::Merge
        );

        std::fs::remove_dir_all(&root).unwrap();
        std::env::remove_var("SPECTRA_DATA_DIR");
        drop(guard);
    }

    #[test]
    fn the_announcement_fires_after_an_upgrade_but_never_after_a_fresh_install() {
        let guard = paths::lock_data_dir();

        let fresh = std::env::temp_dir().join(format!("spectra-fresh-{}", uuid::Uuid::new_v4()));
        std::env::set_var("SPECTRA_DATA_DIR", &fresh);
        std::fs::create_dir_all(&fresh).unwrap();
        assert!(
            !resolve_announcement(),
            "a launcher with no config file has never run before"
        );
        assert!(paths::launcher_config_file().is_file());
        assert!(!resolve_announcement(), "and it stays quiet on every later start");
        std::fs::remove_dir_all(&fresh).unwrap();

        let upgraded = std::env::temp_dir().join(format!("spectra-upg-{}", uuid::Uuid::new_v4()));
        std::env::set_var("SPECTRA_DATA_DIR", &upgraded);
        std::fs::create_dir_all(&upgraded).unwrap();
        std::fs::write(paths::launcher_config_file(), "{\"default_memory_mb\":4096}").unwrap();
        assert!(resolve_announcement(), "an existing install upgrading sees it once");
        assert!(!resolve_announcement(), "but only once");
        std::fs::remove_dir_all(&upgraded).unwrap();

        let no_config = std::env::temp_dir().join(format!("spectra-inst-{}", uuid::Uuid::new_v4()));
        std::env::set_var("SPECTRA_DATA_DIR", &no_config);
        std::fs::create_dir_all(paths::instance_dir("old")).unwrap();
        std::fs::write(paths::instance_config_file("old"), "{}").unwrap();
        assert!(
            resolve_announcement(),
            "instances prove the launcher was used before, even with no settings file"
        );
        std::fs::remove_dir_all(&no_config).unwrap();

        let already = std::env::temp_dir().join(format!("spectra-on-{}", uuid::Uuid::new_v4()));
        std::env::set_var("SPECTRA_DATA_DIR", &already);
        std::fs::create_dir_all(paths::shared_sync_dir()).unwrap();
        std::fs::write(paths::launcher_config_file(), "{\"default_memory_mb\":4096}").unwrap();
        let mut state = SyncState::default();
        state.global.set(SyncOption::GameOptions, true);
        save_state(&state).unwrap();
        assert!(
            !resolve_announcement(),
            "someone who already turned syncing on is not told it is new"
        );
        std::fs::remove_dir_all(&already).unwrap();

        std::env::remove_var("SPECTRA_DATA_DIR");
        drop(guard);
    }

    #[test]
    fn options_round_trip_keeps_local_keys_and_takes_shared_ones() {
        let guard = paths::lock_data_dir();
        let root = std::env::temp_dir().join(format!("spectra-opt-{}", uuid::Uuid::new_v4()));
        std::env::set_var("SPECTRA_DATA_DIR", &root);

        let game = paths::instance_game_dir("src");
        std::fs::create_dir_all(&game).unwrap();
        std::fs::write(game.join("options.txt"), "fov:1.0\nversion:3465\nlastServer:a.b\n").unwrap();
        push_options("src", "1.20.1").unwrap();

        let target = paths::instance_game_dir("dst");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("options.txt"), "fov:0.2\nversion:1234\n").unwrap();
        pull_options("dst", "1.20.1").unwrap();

        let result = read_options(&target.join("options.txt"));
        assert_eq!(result.get("fov").unwrap(), "1.0", "whitelisted key travels");
        assert_eq!(result.get("version").unwrap(), "1234", "instance keeps its own version line");
        assert!(!result.contains_key("lastServer"), "non-whitelisted keys do not travel");

        std::fs::remove_dir_all(&root).unwrap();
        std::env::remove_var("SPECTRA_DATA_DIR");
        drop(guard);
    }
}
