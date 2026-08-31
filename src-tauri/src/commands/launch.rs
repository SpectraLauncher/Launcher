use lyceris::auth::AuthMethod;
use lyceris::minecraft::config::{ConfigBuilder, Memory};
use lyceris::minecraft::emitter::{Emitter, Event};
use lyceris::minecraft::install::install;
use lyceris::minecraft::launch::launch;
use lyceris::minecraft::loader::{
    fabric::Fabric, forge::Forge, neoforge::NeoForge, quilt::Quilt, Loader as LyLoader,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter as _, Manager, State};

use crate::commands::auth::refresh_active_account;
use crate::commands::instances;
use crate::commands::settings::get_settings;
use crate::models::{AccountKind, Instance, Loader};
use crate::{paths, store, AppState};

#[derive(Debug, Deserialize)]
#[serde(tag = "kind")]
pub enum QuickPlay {
    Singleplayer { world: String },
    Multiplayer { host: String, port: Option<u16> },
}

#[derive(Clone, Serialize)]
struct MultiProgress {
    instance_id: String,
    current: u64,
    total: u64,
}

#[derive(Clone, Serialize)]
struct ConsoleLine {
    instance_id: String,
    line: String,
}

#[derive(Clone, Serialize)]
struct ExitInfo {
    instance_id: String,
    code: Option<i32>,
}

#[derive(Clone, Serialize)]
struct CrashInfo {
    instance_id: String,
    code: Option<i32>,
    crash_report_rel: Option<String>,
}

#[cfg(unix)]
fn symlink_dir(target: &std::path::Path, link: &std::path::Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn symlink_dir(target: &std::path::Path, link: &std::path::Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(target, link).or_else(|_| {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .creation_flags(CREATE_NO_WINDOW)
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(std::io::Error::other("mklink /J failed"))
        }
    })
}

fn merge_move(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let dest = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            merge_move(&entry.path(), &dest)?;
        } else if !dest.exists() {
            if std::fs::rename(entry.path(), &dest).is_err() {
                std::fs::copy(entry.path(), &dest)?;
            }
        }
    }
    Ok(())
}

fn link_shared_dirs(id: &str) {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = LOCK.lock();
    let game_dir = paths::instance_game_dir(id);
    for (name, shared) in [
        ("assets", paths::shared_assets_dir()),
        ("libraries", paths::shared_libraries_dir()),
    ] {
        let link = game_dir.join(name);
        let is_link = link.symlink_metadata().map(|m| m.is_symlink()).unwrap_or(false);
        if is_link {
            if let Err(e) = std::fs::create_dir_all(&shared) {
                log::warn!("shared {name} dir: {e}");
            }
            continue;
        }
        if link.is_dir() {
            if let Err(e) = merge_move(&link, &shared) {
                log::warn!("migrating {name} of instance {id}: {e}");
                continue;
            }
            if let Err(e) = std::fs::remove_dir_all(&link) {
                log::warn!("removing migrated {name} of instance {id}: {e}");
                continue;
            }
        } else if let Err(e) = std::fs::create_dir_all(&shared).and_then(|_| std::fs::create_dir_all(&game_dir)) {
            log::warn!("shared {name} dir: {e}");
            continue;
        }
        if let Err(e) = symlink_dir(&shared, &link) {
            log::warn!("linking shared {name} into instance {id}: {e}");
        }
    }
}

#[tauri::command]
pub fn migrate_shared_dirs() {
    let Ok(entries) = std::fs::read_dir(paths::instances_dir()) else { return };
    for entry in entries.flatten() {
        let id = entry.file_name().to_string_lossy().into_owned();
        if paths::instance_config_file(&id).is_file() {
            link_shared_dirs(&id);
        }
    }
}

fn to_lyceris_loader(loader: &Loader, mc: &str) -> Option<Box<dyn LyLoader>> {
    match loader {
        Loader::Vanilla => None,
        Loader::Fabric(v) => Some(Fabric(v.clone()).into()),
        Loader::Quilt(v) => Some(Quilt(v.clone()).into()),
        Loader::Forge(v) => Some(Forge(v.strip_prefix(&format!("{mc}-")).unwrap_or(v).to_string()).into()),
        Loader::NeoForge(v) => Some(NeoForge(v.clone()).into()),
    }
}

async fn build_emitter(app: &AppHandle, id: &str) -> Emitter {
    let emitter = Emitter::default();

    let app_multi = app.clone();
    let id_multi = id.to_string();
    emitter
        .on(
            Event::MultipleDownloadProgress,
            move |(_, current, total, _): (String, u64, u64, String)| {
                let _ = app_multi.emit(
                    "mc://multi-progress",
                    MultiProgress {
                        instance_id: id_multi.clone(),
                        current,
                        total,
                    },
                );
            },
        )
        .await;

    let app_console = app.clone();
    let id_console = id.to_string();
    emitter
        .on(Event::Console, move |line: String| {
            let _ = app_console.emit(
                "mc://console",
                ConsoleLine {
                    instance_id: id_console.clone(),
                    line,
                },
            );
        })
        .await;

    emitter
}

#[tauri::command]
pub async fn launch_instance(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    quick_play: Option<QuickPlay>,
) -> Result<(), String> {
    {
        let mut running = state.running.lock().map_err(|e| e.to_string())?;
        if !running.insert(id.clone()) {
            return Err("instance is already running".into());
        }
    }

    if let Some(pid) = read_lock_pid(&id) {
        if is_game_pid_alive(pid) {
            if let Ok(mut pids) = state.pids.lock() {
                pids.insert(id.clone(), pid);
            }
            if let Ok(mut adopted) = state.adopted.lock() {
                adopted.insert(id.clone());
            }
            return Err("instance is already running".into());
        }
        remove_lock(&id);
    }

    let result = launch_inner(&app, &id, quick_play).await;
    if result.is_err() {
        if let Ok(mut running) = state.running.lock() {
            running.remove(&id);
        }
    }
    result
}

async fn launch_inner(app: &AppHandle, id: &str, quick_play: Option<QuickPlay>) -> Result<(), String> {
    let instance: Instance =
        store::read_json(&paths::instance_config_file(id))?.ok_or("instance not found")?;
    let _ = instances::touch_last_played(id);
    let settings = get_settings()?;
    let account = refresh_active_account().await?;

    let auth = match account.kind {
        AccountKind::Offline => AuthMethod::Offline {
            username: account.username,
            uuid: Some(account.uuid),
        },
        AccountKind::Microsoft => AuthMethod::Microsoft {
            username: account.username,
            xuid: account.xuid,
            uuid: account.uuid,
            access_token: account.access_token,
            refresh_token: account.refresh_token,
        },
    };

    let memory_mb = (if instance.override_memory { instance.memory_mb } else { None })
        .unwrap_or(settings.default_memory_mb) as u64;
    let java_args = if instance.override_java_args {
        instance.java_args.clone()
    } else {
        settings.default_java_args.clone()
    };
    let (fullscreen, width, height) = if instance.override_window {
        (instance.fullscreen, instance.width, instance.height)
    } else {
        (settings.default_fullscreen, settings.default_width, settings.default_height)
    };
    let (pre_launch, post_exit) = if instance.override_hooks {
        (instance.pre_launch.clone(), instance.post_exit.clone())
    } else {
        (settings.default_pre_launch.clone(), settings.default_post_exit.clone())
    };

    let mut game_args: Vec<String> = Vec::new();
    if fullscreen {
        game_args.push("--fullscreen".into());
    }
    if let Some(w) = width {
        game_args.push("--width".into());
        game_args.push(w.to_string());
    }
    if let Some(h) = height {
        game_args.push("--height".into());
        game_args.push(h.to_string());
    }
    match quick_play {
        Some(QuickPlay::Singleplayer { world }) => {
            game_args.push("--quickPlaySingleplayer".into());
            game_args.push(world);
        }
        Some(QuickPlay::Multiplayer { host, port }) => {
            game_args.push("--quickPlayMultiplayer".into());
            let addr = match port {
                Some(p) if p != 25565 => format!("{host}:{p}"),
                _ => host,
            };
            game_args.push(addr);
        }
        None => {}
    }

    if let Some(cmd) = pre_launch.as_deref().filter(|s| !s.trim().is_empty()) {
        run_hook(cmd, true);
    }

    link_shared_dirs(id);
    let builder = ConfigBuilder::new(paths::instance_game_dir(id), instance.mc_version.clone(), auth)
        .memory(Memory::Megabyte(memory_mb))
        .runtime_dir(paths::runtimes_dir())
        .client(crate::http().clone())
        .custom_java_args(java_args)
        .custom_args(game_args);

    let emitter = build_emitter(app, id).await;
    let app_state = app.state::<AppState>();

    let mut child = match to_lyceris_loader(&instance.loader, &instance.mc_version) {
        None => {
            let config = builder.build();
            {
                let _install_guard = app_state.install_lock.lock().await;
                install(&config, Some(&emitter))
                    .await
                    .map_err(|e| format!("install failed: {e}"))?;
            }
            launch(&config, Some(&emitter))
                .await
                .map_err(|e| format!("launch failed: {e}"))?
        }
        Some(loader) => {
            let config = builder.loader(loader).build();
            {
                let _install_guard = app_state.install_lock.lock().await;
                install(&config, Some(&emitter))
                    .await
                    .map_err(|e| format!("install failed: {e}"))?;
            }
            launch(&config, Some(&emitter))
                .await
                .map_err(|e| format!("launch failed: {e}"))?
        }
    };

    let track_playtime = settings.track_playtime;
    let discord_rpc = settings.discord_rpc;
    let share_activity = settings.share_activity;
    let started = std::time::Instant::now();

    if let Some(pid) = child.id() {
        if let Ok(mut pids) = app_state.pids.lock() {
            pids.insert(id.to_string(), pid);
        }
        write_lock(id, pid);
    }

    if discord_rpc {
        if let Ok(mut map) = app_state.discord_playing.lock() {
            map.insert(id.to_string(), (instance.name.clone(), instance.mc_version.clone()));
        }
        crate::discord::update_presence(&app_state);
    }

    if share_activity {
        let app_hb = app.clone();
        let id_hb = id.to_string();
        tauri::async_runtime::spawn(async move {
            crate::commands::spectra::report_activity(true, 0).await;

            let mut reported = 0u64;
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;

                let running = app_hb
                    .try_state::<AppState>()
                    .and_then(|state| state.running.lock().ok().map(|set| set.contains(&id_hb)))
                    .unwrap_or(false);

                if running {
                    let due = started.elapsed().as_secs().saturating_sub(reported);
                    if due >= 300 {
                        crate::commands::spectra::report_activity(false, due).await;
                        reported += due.min(crate::commands::spectra::ACTIVITY_MAX_SECONDS);
                    }
                    continue;
                }

                for chunk in crate::commands::spectra::activity_chunks(started.elapsed().as_secs(), reported) {
                    crate::commands::spectra::report_activity(false, chunk).await;
                }
                break;
            }
        });
    }

    let app_bg = app.clone();
    let id_bg = id.to_string();
    let game_start = std::time::SystemTime::now();
    tauri::async_runtime::spawn(async move {
        let _keep_emitter = emitter;
        let code = match child.wait().await {
            Ok(status) => status.code(),
            Err(_) => None,
        };
        let mut stopped_by_user = false;
        if let Some(state) = app_bg.try_state::<AppState>() {
            if let Ok(mut stopping) = state.stopping.lock() {
                stopped_by_user = stopping.remove(&id_bg);
            }
            if let Ok(mut running) = state.running.lock() {
                running.remove(&id_bg);
            }
            if let Ok(mut pids) = state.pids.lock() {
                pids.remove(&id_bg);
            }
            if discord_rpc {
                if let Ok(mut map) = state.discord_playing.lock() {
                    map.remove(&id_bg);
                }
                crate::discord::update_presence(&state);
            }
        }
        remove_lock(&id_bg);
        if track_playtime {
            let _ = instances::add_playtime(&id_bg, started.elapsed().as_secs());
        }
        if let Some(cmd) = post_exit.as_deref().filter(|s| !s.trim().is_empty()) {
            run_hook(cmd, false);
        }

        let is_crash = !stopped_by_user && code.map(|c| c != 0).unwrap_or(false);
        if is_crash {
            let crash_rel = find_latest_crash_report(&id_bg, game_start);
            let _ = app_bg.emit_to(
                "main",
                "mc://crashed",
                CrashInfo {
                    instance_id: id_bg.clone(),
                    code,
                    crash_report_rel: crash_rel,
                },
            );
        }
        let _ = app_bg.emit(
            "mc://exited",
            ExitInfo {
                instance_id: id_bg,
                code,
            },
        );
    });

    Ok(())
}

fn run_hook(cmd: &str, wait: bool) {
    let mut command;
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command = std::process::Command::new("cmd");
        command.arg("/C").arg(cmd).creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    {
        command = std::process::Command::new("sh");
        command.arg("-c").arg(cmd);
    }
    let result = if wait { command.status().map(|_| ()) } else { command.spawn().map(|_| ()) };
    if let Err(e) = result {
        log::warn!("launch hook failed: {e}");
    }
}

#[tauri::command]
pub async fn repair_instance(app: AppHandle, id: String) -> Result<(), String> {
    let instance: Instance =
        store::read_json(&paths::instance_config_file(&id))?.ok_or("instance not found")?;
    let settings = get_settings()?;
    let auth = AuthMethod::Offline { username: "Player".into(), uuid: None };
    let memory_mb = instance.memory_mb.unwrap_or(settings.default_memory_mb) as u64;

    link_shared_dirs(&id);
    let builder = ConfigBuilder::new(paths::instance_game_dir(&id), instance.mc_version.clone(), auth)
        .memory(Memory::Megabyte(memory_mb))
        .runtime_dir(paths::runtimes_dir())
        .client(crate::http().clone());
    let emitter = build_emitter(&app, &id).await;

    let result = match to_lyceris_loader(&instance.loader, &instance.mc_version) {
        None => install(&builder.build(), Some(&emitter)).await,
        Some(loader) => install(&builder.loader(loader).build(), Some(&emitter)).await,
    };
    let _ = app.emit("mc://exited", ExitInfo { instance_id: id, code: Some(0) });
    result.map_err(|e| format!("repair failed: {e}"))
}

#[tauri::command]
pub fn is_instance_running(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    let running = state.running.lock().map_err(|e| e.to_string())?;
    Ok(running.contains(&id))
}

#[tauri::command]
pub fn stop_instance(state: State<'_, AppState>, id: String, force: bool) -> Result<(), String> {
    let pid = {
        let pids = state.pids.lock().map_err(|e| e.to_string())?;
        pids.get(&id).copied().ok_or("instance is not running")?
    };
    if let Ok(mut stopping) = state.stopping.lock() {
        stopping.insert(id.clone());
    }
    kill_process_tree(pid, force)?;

    let is_adopted = state.adopted.lock().map(|a| a.contains(&id)).unwrap_or(false);
    if is_adopted {
        if let Ok(mut running) = state.running.lock() {
            running.remove(&id);
        }
        if let Ok(mut pids) = state.pids.lock() {
            pids.remove(&id);
        }
        if let Ok(mut adopted) = state.adopted.lock() {
            adopted.remove(&id);
        }
        if let Ok(mut stopping) = state.stopping.lock() {
            stopping.remove(&id);
        }
        remove_lock(&id);
    }
    Ok(())
}

fn find_latest_crash_report(id: &str, since: std::time::SystemTime) -> Option<String> {
    let dir = paths::instance_game_dir(id).join("crash-reports");
    let entries = std::fs::read_dir(&dir).ok()?;
    let mut best: Option<(std::time::SystemTime, String)> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext != "txt" && ext != "log" {
            continue;
        }
        let Some(modified) = entry.metadata().ok().and_then(|m| m.modified().ok()) else {
            continue;
        };
        if modified < since {
            continue;
        }
        if best.as_ref().map(|(t, _)| modified > *t).unwrap_or(true) {
            let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
                continue;
            };
            best = Some((modified, format!("crash-reports/{name}")));
        }
    }
    best.map(|(_, rel)| rel)
}

fn write_lock(id: &str, pid: u32) {
    let _ = std::fs::write(paths::instance_lock_file(id), pid.to_string());
}

fn read_lock_pid(id: &str) -> Option<u32> {
    std::fs::read_to_string(paths::instance_lock_file(id))
        .ok()?
        .trim()
        .parse()
        .ok()
}

fn remove_lock(id: &str) {
    let _ = std::fs::remove_file(paths::instance_lock_file(id));
}

fn is_game_pid_alive(pid: u32) -> bool {
    use sysinfo::{Pid, ProcessesToUpdate, System};
    let p = Pid::from_u32(pid);
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::Some(&[p]), true);
    match sys.process(p) {
        Some(proc_) => proc_.name().to_string_lossy().to_lowercase().contains("java"),
        None => false,
    }
}

pub fn reconcile_running(app: &AppHandle) {
    let state = app.state::<AppState>();
    let Ok(entries) = std::fs::read_dir(paths::instances_dir()) else { return };
    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().into_owned();
        let Some(pid) = read_lock_pid(&id) else { continue };
        if is_game_pid_alive(pid) {
            if let Ok(mut running) = state.running.lock() {
                running.insert(id.clone());
            }
            if let Ok(mut pids) = state.pids.lock() {
                pids.insert(id.clone(), pid);
            }
            if let Ok(mut adopted) = state.adopted.lock() {
                adopted.insert(id.clone());
            }
            spawn_adopted_watcher(app.clone(), id, pid);
        } else {
            remove_lock(&id);
        }
    }
}

fn spawn_adopted_watcher(app: AppHandle, id: String, pid: u32) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(4)).await;
            if !is_game_pid_alive(pid) {
                if let Some(state) = app.try_state::<AppState>() {
                    if let Ok(mut r) = state.running.lock() { r.remove(&id); }
                    if let Ok(mut p) = state.pids.lock()    { p.remove(&id); }
                    if let Ok(mut a) = state.adopted.lock() { a.remove(&id); }
                    if let Ok(mut map) = state.discord_playing.lock() { map.remove(&id); }
                    crate::discord::update_presence(&state);
                }
                remove_lock(&id);
                let _ = app.emit("mc://exited", ExitInfo { instance_id: id, code: None });
                break;
            }
        }
    });
}

fn kill_process_tree(pid: u32, force: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let mut cmd = std::process::Command::new("taskkill");
        cmd.args(["/PID", &pid.to_string(), "/T"]);
        if force {
            cmd.arg("/F");
        }
        cmd.creation_flags(CREATE_NO_WINDOW)
            .status()
            .map(|_| ())
            .map_err(|e| format!("taskkill: {e}"))
    }
    #[cfg(not(windows))]
    {
        let sig = if force { "-KILL" } else { "-TERM" };
        let pgid = unsafe { libc::getpgid(pid as libc::pid_t) };
        let own = unsafe { libc::getpgid(0) };
        let target = if pgid > 0 && pgid != own {
            format!("-{pgid}")
        } else {
            pid.to_string()
        };
        std::process::Command::new("kill")
            .args([sig, &target])
            .status()
            .map(|_| ())
            .map_err(|e| format!("kill: {e}"))
    }
}

pub fn instance_id_from_url(url: &str) -> Option<String> {
    let rest = url.strip_prefix("spectra://")?.trim_start_matches('/');
    let id = rest.strip_prefix("launch/")?.trim_end_matches('/');
    let plain = !id.is_empty()
        && id.len() <= 64
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    plain.then(|| id.to_string())
}

#[tauri::command]
pub fn take_pending_launch(state: tauri::State<'_, crate::AppState>) -> Option<String> {
    state.pending_launch.lock().ok()?.take()
}

#[cfg(test)]
mod deep_link_tests {
    use super::instance_id_from_url;

    #[test]
    fn parses_launch_links() {
        let id = "2f2a1ad4-0bb4-4e03-8251-ecfc8a5b8fd7";
        assert_eq!(instance_id_from_url(&format!("spectra://launch/{id}")), Some(id.into()));
        assert_eq!(instance_id_from_url(&format!("spectra://launch/{id}/")), Some(id.into()));
        assert_eq!(instance_id_from_url("spectra://share/ABC123"), None);
        assert_eq!(instance_id_from_url("spectra://launch/"), None);
        assert_eq!(instance_id_from_url("spectra://launch/../../etc/passwd"), None);
        assert_eq!(instance_id_from_url("spectra://launch/a b"), None);
    }
}

#[cfg(test)]
mod shared_dir_tests {
    use super::{link_shared_dirs, migrate_shared_dirs};
    use crate::paths;
    use std::fs;

    fn seed(id: &str, name: &str, file: &str, body: &str) {
        let dir = paths::instance_game_dir(id).join(name).join("sub");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(file), body).unwrap();
    }

    #[test]
    fn migrates_existing_instances_and_links_new_ones() {
        let root = std::env::temp_dir().join(format!("spectra-{}", uuid::Uuid::new_v4()));
        std::env::set_var("SPECTRA_DATA_DIR", &root);

        seed("old", "assets", "a.bin", "one");
        seed("old", "libraries", "l.jar", "lib");
        link_shared_dirs("old");

        let link = paths::instance_game_dir("old").join("assets");
        assert!(link.symlink_metadata().unwrap().is_symlink());
        assert_eq!(fs::read_to_string(link.join("sub/a.bin")).unwrap(), "one");
        assert_eq!(fs::read_to_string(paths::shared_assets_dir().join("sub/a.bin")).unwrap(), "one");
        assert!(paths::shared_libraries_dir().join("sub/l.jar").is_file());

        seed("second", "assets", "a.bin", "clobbered");
        seed("second", "assets", "b.bin", "two");
        link_shared_dirs("second");

        let shared = paths::shared_assets_dir();
        assert_eq!(fs::read_to_string(shared.join("sub/a.bin")).unwrap(), "one");
        assert_eq!(fs::read_to_string(shared.join("sub/b.bin")).unwrap(), "two");
        assert!(paths::instance_game_dir("second").join("assets").symlink_metadata().unwrap().is_symlink());

        link_shared_dirs("second");
        assert_eq!(fs::read_to_string(shared.join("sub/b.bin")).unwrap(), "two");

        seed("fresh", "mods", "m.jar", "mod");
        link_shared_dirs("fresh");
        assert_eq!(
            fs::read_to_string(paths::instance_game_dir("fresh").join("assets/sub/a.bin")).unwrap(),
            "one"
        );

        seed("third", "assets", "c.bin", "three");
        fs::write(paths::instance_config_file("third"), "{}").unwrap();
        seed("stray", "assets", "d.bin", "four");
        migrate_shared_dirs();

        assert_eq!(fs::read_to_string(shared.join("sub/c.bin")).unwrap(), "three");
        assert!(paths::instance_game_dir("third").join("assets").symlink_metadata().unwrap().is_symlink());
        assert!(!shared.join("sub/d.bin").exists());
        assert!(!paths::instance_game_dir("stray").join("assets").symlink_metadata().unwrap().is_symlink());

        fs::remove_dir_all(&root).unwrap();
        std::env::remove_var("SPECTRA_DATA_DIR");
    }
}

#[cfg(all(test, unix))]
mod kill_tests {
    use super::kill_process_tree;
    use std::time::{Duration, Instant};

    #[test]
    fn kills_the_game_and_not_the_launcher() {
        let mut child = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .unwrap();
        let pid = child.id();

        assert_eq!(
            unsafe { libc::getpgid(pid as libc::pid_t) },
            unsafe { libc::getpgid(0) },
            "child is expected to share our process group, as the JVM does"
        );

        kill_process_tree(pid, true).unwrap();

        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            assert!(Instant::now() < deadline, "child survived the kill");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}
