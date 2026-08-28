use crate::models::Settings;
use crate::{paths, store};

#[tauri::command]
pub fn get_settings() -> Result<Settings, String> {
    Ok(store::read_json::<Settings>(&paths::launcher_config_file())?.unwrap_or_default())
}

#[tauri::command]
pub fn save_settings(settings: Settings) -> Result<(), String> {
    store::write_json(&paths::launcher_config_file(), &settings)
}

#[tauri::command]
pub fn get_system_memory_mb() -> u64 {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    sys.total_memory() / (1024 * 1024)
}
