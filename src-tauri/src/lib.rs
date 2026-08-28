mod commands;
mod discord;
mod models;
mod paths;
mod store;

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

#[derive(Default)]
pub struct AppState {
    pub running: Mutex<HashSet<String>>,
    pub pids: Mutex<HashMap<String, u32>>,
    pub adopted: Mutex<HashSet<String>>,
    pub stopping: Mutex<HashSet<String>>,
    pub discord: Mutex<Option<discord_rich_presence::DiscordIpcClient>>,
    pub discord_playing: Mutex<HashMap<String, (String, String)>>,
    pub install_lock: tokio::sync::Mutex<()>,
    pub pending_share: Mutex<Option<String>>,
    pub pending_launch: Mutex<Option<String>>,
}

#[cfg(desktop)]
fn handle_deep_link(app: &tauri::AppHandle, url: &str) {
    use tauri::{Emitter, Manager};

    if let Some(token) = commands::spectra::login_token_from_url(url) {
        let handle = app.clone();
        tauri::async_runtime::spawn(async move {
            commands::spectra::redeem_login(handle, token).await;
        });
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
        return;
    }

    if let Some(id) = commands::launch::instance_id_from_url(url) {
        if let Some(state) = app.try_state::<AppState>() {
            if let Ok(mut pending) = state.pending_launch.lock() {
                *pending = Some(id.clone());
            }
        }
        let _ = app.emit("launch://open", &id);
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
        return;
    }

    let Some(code) = commands::share::code_from_url(url) else { return };
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut pending) = state.pending_share.lock() {
            *pending = Some(code.clone());
        }
    }
    let _ = app.emit("share://open", &code);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();

    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
        use tauri::Manager;
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
    }));

    builder
        .manage(AppState::default())
        .plugin(tauri_plugin_deep_link::init())
        .setup(|app| {
            if let Err(e) = paths::ensure_base_dirs() {
                log::error!("failed to create data directories: {e}");
            }

            commands::launch::reconcile_running(app.handle());

            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            #[cfg(desktop)]
            {
                let _ = app.handle().plugin(tauri_plugin_updater::Builder::new().build());
            }

            #[cfg(desktop)]
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                let _ = app.deep_link().register_all();

                #[cfg(windows)]
                if let Ok(key) = windows_registry::CURRENT_USER.create("Software\\Classes\\spectra") {
                    let _ = key.set_string("", "URL:Spectra Launcher protocol");
                }

                let handle = app.handle().clone();
                app.deep_link().on_open_url(move |event| {
                    for url in event.urls() {
                        handle_deep_link(&handle, url.as_str());
                    }
                });
                if let Ok(Some(urls)) = app.deep_link().get_current() {
                    for url in urls {
                        handle_deep_link(app.handle(), url.as_str());
                    }
                }
            }
            Ok(())
        })
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .invoke_handler(tauri::generate_handler![
            paths::get_launcher_paths,
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::get_system_memory_mb,
            commands::instances::list_instances,
            commands::instances::get_instance,
            commands::instances::create_instance,
            commands::instances::update_instance,
            commands::instances::set_instance_icon,
            commands::instances::set_instance_icon_data,
            commands::instances::list_custom_symbols,
            commands::instances::add_custom_symbol,
            commands::instances::delete_custom_symbol,
            commands::instances::create_desktop_shortcut,
            commands::launch::take_pending_launch,
            commands::instances::delete_instance,
            commands::instances::read_image_data_url,
            commands::instances::get_instance_icon_path,
            commands::instances::get_instance_path,
            commands::instances::open_instance_folder,
            commands::instances::open_instance_game_folder,
            commands::instances::reveal_in_explorer,
            commands::instances::open_external,
            commands::instances::copy_file,
            commands::instances::duplicate_instance,
            commands::content::list_screenshots,
            commands::content::list_worlds,
            commands::content::list_resource_packs,
            commands::content::list_data_packs,
            commands::content::list_shaders,
            commands::content::list_servers,
            commands::content::add_server,
            commands::content::delete_server,
            commands::content::delete_content,
            commands::content::set_content_enabled,
            commands::content::delete_world,
            commands::content::backup_world,
            commands::content::delete_screenshot,
            commands::content::list_log_files,
            commands::content::read_log_file,
            commands::content::upload_log_to_mclogs,
            commands::auth::auth_login,
            commands::auth::auth_login_offline,
            commands::auth::auth_get_login_url,
            commands::auth::auth_login_with_code,
            commands::auth::auth_refresh_active,
            commands::auth::list_accounts,
            commands::auth::set_active_account,
            commands::auth::remove_account,
            commands::launch::launch_instance,
            commands::launch::repair_instance,
            commands::launch::is_instance_running,
            commands::launch::stop_instance,
            commands::ping::ping_server,
            commands::meta::get_minecraft_versions,
            commands::meta::get_loader_versions,
            commands::java::detect_java_installations,
            commands::java::validate_java_path,
            commands::modrinth::modrinth_search,
            commands::modrinth::modrinth_versions,
            commands::modrinth::check_mod_updates,
            commands::modrinth::update_all_mods,
            commands::modrinth::match_local_mods,
            commands::modrinth::modrinth_match_file,
            commands::modrinth::modrinth_project,
            commands::modrinth::modrinth_categories,
            commands::modrinth::modrinth_install_with_deps,
            commands::modrinth::get_installed_content,
            commands::modrinth::get_removable_dependencies,
            commands::modrinth::check_conflicts,
            commands::modrinth::modrinth_install_modpack,
            commands::modrinth::import_file,
            commands::modrinth::export_mrpack,
            commands::modrinth::check_modpack_update,
            commands::modrinth::update_modpack,
            commands::curseforge::cf_enabled,
            commands::curseforge::curseforge_search,
            commands::curseforge::curseforge_versions,
            commands::curseforge::curseforge_project,
            commands::curseforge::curseforge_categories,
            commands::curseforge::curseforge_install_with_deps,
            commands::curseforge::curseforge_match_local,
            commands::curseforge::curseforge_match_file,
            commands::curseforge::curseforge_update_all,
            commands::curseforge::curseforge_install_modpack,
            commands::curseforge::curseforge_import_modpack_file,
            commands::curseforge::export_curseforge,
            commands::curseforge::get_blocked_mods,
            commands::curseforge::resolve_blocked_mods,
            commands::curseforge::default_downloads_dir,
            commands::import::detect_external_instances,
            commands::import::import_external_instance,
            commands::import::list_dir,
            commands::import::export_instance,
            commands::import::import_dropped,
            commands::import::write_text_file,
            commands::share::share_preview,
            commands::share::share_instance,
            commands::share::import_share,
            commands::share::sync_share,
            commands::snapshots::list_snapshots,
            commands::snapshots::create_snapshot,
            commands::snapshots::restore_snapshot,
            commands::snapshots::delete_snapshot,
            commands::share::take_pending_share,
            commands::content_window::open_content_window,
            commands::content_window::content_window_config,
            commands::content_window::close_content_window,
            commands::content_window::content_installed,
            commands::spectra::spectra_login_url,
            commands::spectra::spectra_profile_url,
            commands::spectra::spectra_session,
            commands::spectra::spectra_logout,
            commands::spectra::spectra_api,
            commands::spectra::spectra_link_minecraft,
            commands::mods::list_mods,
            commands::mods::list_content,
            commands::mods::set_mod_enabled,
            commands::mods::delete_mod,
            commands::skins::list_skins,
            commands::skins::save_skin,
            commands::skins::set_skin_model,
            commands::skins::delete_skin,
            commands::skins::get_skin_path,
            commands::skins::get_skin_data_url,
            commands::skins::fetch_skin_data_url,
            commands::skins::get_player_skin,
            commands::skins::import_player_skin,
            commands::skins::apply_skin,
            commands::skins::get_player_capes,
            commands::skins::set_active_cape,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
