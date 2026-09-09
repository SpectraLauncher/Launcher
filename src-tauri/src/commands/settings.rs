use crate::models::Settings;
use crate::{paths, store};
use crate::error::AppResult;

pub fn load() -> AppResult<Settings> {
    Ok(store::read_json::<Settings>(&paths::launcher_config_file())?.unwrap_or_default())
}

#[tauri::command]
pub async fn get_settings() -> AppResult<Settings> {
    crate::blocking(load).await
}

#[tauri::command]
pub async fn save_settings(settings: Settings) -> AppResult<()> {
    crate::blocking(move || store::write_json(&paths::launcher_config_file(), &settings)).await
}

#[tauri::command]
pub fn get_system_memory_mb() -> u64 {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    sys.total_memory() / (1024 * 1024)
}
