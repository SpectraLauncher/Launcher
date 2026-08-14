//! Spectra Launcher data layout.
//!
//! Everything the launcher owns lives under a single, relocatable **data root**.
//! Each Minecraft instance is fully self-contained (its own `minecraft/` game dir),
//! so instances can be copied, backed up or shared independently — that's the
//! "modular" part. Only the Java runtimes are shared between instances to avoid
//! re-downloading multi-hundred-MB JDKs.
//!
//! ```text
//! <data root>/                      ($SPECTRA_DATA_DIR or <OS data dir>/SpectraLauncher)
//! ├── launcher.json                 global settings (see models::Settings)
//! ├── accounts.json                 saved Microsoft accounts (see models::AccountsFile)
//! ├── instances/
//! │   └── <instance-id>/
//! │       ├── instance.json         instance metadata (see models::Instance)
//! │       └── minecraft/            Lyceris game_dir: versions, libraries, assets,
//! │                                 mods, saves, config, resourcepacks, options.txt …
//! ├── runtimes/                     shared Lyceris runtime_dir (managed Java per major)
//! ├── skins/                        saved skins: <id>.png + skins.json index
//! ├── cache/                        Modrinth manifests, downloaded icons, temp files
//! └── logs/                         the launcher's own logs (not the game's)
//! ```

use std::path::PathBuf;

const APP_DIR_NAME: &str = "SpectraLauncher";

/// Resolves the data root.
///
/// Priority: `SPECTRA_DATA_DIR` env override → OS data dir → current dir fallback.
/// The override makes portable installs and tests trivial.
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

/// Session token for the Spectra account (friends, shared instances). Separate
/// from `launcher.json`, which is handed to the frontend in one piece.
pub fn spectra_account_file() -> PathBuf {
    data_root().join("spectra-account.json")
}

pub fn instances_dir() -> PathBuf {
    data_root().join("instances")
}

/// Directory holding a single instance's metadata + game dir.
pub fn instance_dir(id: &str) -> PathBuf {
    instances_dir().join(id)
}

pub fn instance_config_file(id: &str) -> PathBuf {
    instance_dir(id).join("instance.json")
}

/// The Minecraft `game_dir` handed to Lyceris for an instance.
pub fn instance_game_dir(id: &str) -> PathBuf {
    instance_dir(id).join("minecraft")
}

/// Index of installed Modrinth content for an instance (`content.json`).
pub fn instance_content_index(id: &str) -> PathBuf {
    instance_dir(id).join("content.json")
}

/// Optional custom icon for an instance (`instances/<id>/icon.png`).
pub fn instance_icon_file(id: &str) -> PathBuf {
    instance_dir(id).join("icon.png")
}

/// Run lock for an instance (`instances/<id>/instance.lock`). Holds the game's
/// PID so a relaunch — even after the launcher itself restarts — can tell the
/// instance is already running and avoid two JVMs sharing one game dir.
pub fn instance_lock_file(id: &str) -> PathBuf {
    instance_dir(id).join("instance.lock")
}

/// Shared Java runtime directory (Lyceris `runtime_dir`), reused across instances.
pub fn runtimes_dir() -> PathBuf {
    data_root().join("runtimes")
}

pub fn skins_dir() -> PathBuf {
    data_root().join("skins")
}

pub fn cache_dir() -> PathBuf {
    data_root().join("cache")
}

pub fn logs_dir() -> PathBuf {
    data_root().join("logs")
}

/// Creates the top-level directories if they don't exist. Cheap and idempotent;
/// call once on startup.
pub fn ensure_base_dirs() -> std::io::Result<()> {
    for dir in [
        data_root(),
        instances_dir(),
        runtimes_dir(),
        skins_dir(),
        cache_dir(),
        logs_dir(),
    ] {
        std::fs::create_dir_all(dir)?;
    }
    Ok(())
}

/// Frontend-facing snapshot of where everything lives. Lets the UI show
/// "open folder" buttons without hard-coding paths.
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
