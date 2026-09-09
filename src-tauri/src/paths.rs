use std::path::PathBuf;

const APP_DIR_NAME: &str = "SpectraLauncher";

#[cfg(test)]
pub static DATA_DIR_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
pub fn lock_data_dir() -> std::sync::MutexGuard<'static, ()> {
    DATA_DIR_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn data_root() -> PathBuf {
    if let Ok(custom) = std::env::var("SPECTRA_DATA_DIR") {
        if !custom.trim().is_empty() {
            return PathBuf::from(custom);
        }
    }
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_DIR_NAME)
}

pub fn launcher_config_file() -> PathBuf {
    data_root().join("launcher.json")
}

pub fn accounts_file() -> PathBuf {
    data_root().join("accounts.json")
}

pub fn spectra_account_file() -> PathBuf {
    data_root().join("spectra-account.json")
}

pub fn instances_dir() -> PathBuf {
    data_root().join("instances")
}

pub fn instance_dir(id: &str) -> PathBuf {
    instances_dir().join(id)
}

pub fn instance_config_file(id: &str) -> PathBuf {
    instance_dir(id).join("instance.json")
}

pub fn instance_game_dir(id: &str) -> PathBuf {
    instance_dir(id).join("minecraft")
}

pub fn instance_content_index(id: &str) -> PathBuf {
    instance_dir(id).join("content.json")
}

pub fn instance_icon_file(id: &str) -> PathBuf {
    instance_dir(id).join("icon.png")
}

pub fn instance_lock_file(id: &str) -> PathBuf {
    instance_dir(id).join("instance.lock")
}

pub fn runtimes_dir() -> PathBuf {
    data_root().join("runtimes")
}

pub fn shared_assets_dir() -> PathBuf {
    data_root().join("assets")
}

pub fn shared_libraries_dir() -> PathBuf {
    data_root().join("libraries")
}

pub fn skins_dir() -> PathBuf {
    data_root().join("skins")
}

pub fn cache_dir() -> PathBuf {
    data_root().join("cache")
}

pub fn shared_sync_dir() -> PathBuf {
    data_root().join("shared")
}

pub fn sync_state_file() -> PathBuf {
    shared_sync_dir().join("sync.json")
}

pub fn sync_servers_file() -> PathBuf {
    shared_sync_dir().join("servers.json")
}

pub fn sync_options_file() -> PathBuf {
    shared_sync_dir().join("options.json")
}

pub fn sync_history_file() -> PathBuf {
    shared_sync_dir().join("command_history.txt")
}

pub fn sync_hotbar_file() -> PathBuf {
    shared_sync_dir().join("hotbar.nbt")
}

pub fn sync_packs_dir() -> PathBuf {
    shared_sync_dir().join("resourcepacks")
}

pub fn sync_packs_file() -> PathBuf {
    shared_sync_dir().join("resourcepacks.json")
}

pub fn symbols_dir() -> PathBuf {
    data_root().join("symbols")
}

pub fn logs_dir() -> PathBuf {
    data_root().join("logs")
}

pub fn ensure_base_dirs() -> std::io::Result<()> {
    for dir in [
        data_root(),
        instances_dir(),
        runtimes_dir(),
        skins_dir(),
        symbols_dir(),
        cache_dir(),
        logs_dir(),
    ] {
        std::fs::create_dir_all(dir)?;
    }
    Ok(())
}

#[derive(serde::Serialize)]
pub struct LauncherPaths {
    pub data_root: String,
    pub instances: String,
    pub runtimes: String,
    pub skins: String,
    pub cache: String,
    pub logs: String,
}

#[tauri::command]
pub fn get_launcher_paths() -> LauncherPaths {
    LauncherPaths {
        data_root: data_root().to_string_lossy().into_owned(),
        instances: instances_dir().to_string_lossy().into_owned(),
        runtimes: runtimes_dir().to_string_lossy().into_owned(),
        skins: skins_dir().to_string_lossy().into_owned(),
        cache: cache_dir().to_string_lossy().into_owned(),
        logs: logs_dir().to_string_lossy().into_owned(),
    }
}
