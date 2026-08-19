//! Installing + launching an instance through Lyceris.
//!
//! `launch_instance` installs (downloading/verifying the game files) then
//! launches, resolving once the game process has started. Progress and console
//! output stream to the frontend as Tauri events, all carrying the `instance_id`
//! they belong to:
//!   - `mc://multi-progress`  `{ instance_id, current, total }`  (libraries/assets/java)
//!   - `mc://file-progress`   `{ instance_id, path, current, total }`
//!   - `mc://console`         `{ instance_id, line }`
//!   - `mc://exited`          `{ instance_id, code }`

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

/// Describes which game session to jump into immediately on launch.
/// Requires Minecraft 1.20+ (the `--quickPlay*` flags were added then).
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
struct FileProgress {
    instance_id: String,
    path: String,
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

/// Emitted when the game exits with a non-zero code and a crash report is found.
#[derive(Clone, Serialize)]
struct CrashInfo {
    instance_id: String,
    code: Option<i32>,
    /// Relative path from the game dir (e.g. "crash-reports/crash-2026-06-22_13.21.01-client.txt").
    crash_report_rel: Option<String>,
}

fn to_lyceris_loader(loader: &Loader, mc: &str) -> Option<Box<dyn LyLoader>> {
    match loader {
        Loader::Vanilla => None,
        Loader::Fabric(v) => Some(Fabric(v.clone()).into()),
        Loader::Quilt(v) => Some(Quilt(v.clone()).into()),
        // Lyceris rebuilds the installer version as `{mc}-{v}`, so it wants only
        // the Forge build (e.g. "47.4.0"), not the full maven id "1.20.1-47.4.0"
        // we store — otherwise the MC version doubles and the download 404s.
        Loader::Forge(v) => Some(Forge(v.strip_prefix(&format!("{mc}-")).unwrap_or(v).to_string()).into()),
        Loader::NeoForge(v) => Some(NeoForge(v.clone()).into()),
    }
}

/// Builds an emitter that forwards Lyceris events for `id` to the webview.
async fn build_emitter(app: &AppHandle, id: &str) -> Emitter {
    let emitter = Emitter::default();

    let app_multi = app.clone();
    let id_multi = id.to_string();
    emitter
        .on(
            Event::MultipleDownloadProgress,
            // 4th field is the file type as a string ("Asset"/"Library"/"Java"/…).
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

    let app_single = app.clone();
    let id_single = id.to_string();
    emitter
        .on(
            Event::SingleDownloadProgress,
            move |(path, current, total): (String, u64, u64)| {
                let _ = app_single.emit(
                    "mc://file-progress",
                    FileProgress {
                        instance_id: id_single.clone(),
                        path,
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

/// Installs (verifying/repairing files) then launches the instance. Returns once
/// the game process is running; it's then awaited in the background so we can
/// emit `mc://exited` and clear the running flag.
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

    // Cross-restart guard: a live lock file means another launcher session (or a
    // leftover game) still owns this game dir. Reconcile state so the UI shows it
    // as running and stop/kill works, then refuse the duplicate launch. A stale
    // lock (process gone) is cleared so the launch proceeds.
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

    // Any early failure must release the running flag.
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
    // Stamp "last played" up front (at launch attempt), so the library reflects
    // it immediately even while the game is still installing/starting.
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

    // Resolve effective launch options: per-instance override, else global default.
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

    // Translate window prefs into Minecraft game arguments.
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
    // Quick Play: jump directly into a world or server (Minecraft 1.20+).
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

    // Pre-launch hook (waited on so it can prepare things before the game starts).
    if let Some(cmd) = pre_launch.as_deref().filter(|s| !s.trim().is_empty()) {
        run_hook(cmd, true);
    }

    let builder = ConfigBuilder::new(paths::instance_game_dir(id), instance.mc_version.clone(), auth)
        .memory(Memory::Megabyte(memory_mb))
        .runtime_dir(paths::runtimes_dir())
        .custom_java_args(java_args)
        .custom_args(game_args);

    let emitter = build_emitter(app, id).await;
    let app_state = app.state::<AppState>();

    // Vanilla and modded produce different Config<T> types, so the install/launch
    // pair has to live inside each branch. The install phase is serialized through
    // a global lock so two instances can't provision the shared Java runtime at
    // the same time (which would corrupt it); launch itself stays concurrent.
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

    // Privacy-gated extras.
    let track_playtime = settings.track_playtime;
    let discord_rpc = settings.discord_rpc;
    let started = std::time::Instant::now();

    // Remember the game PID so the UI can stop/kill it, and write a run lock so a
    // relaunch (even after the launcher restarts) sees the instance is busy.
    if let Some(pid) = child.id() {
        if let Ok(mut pids) = app_state.pids.lock() {
            pids.insert(id.to_string(), pid);
        }
        write_lock(id, pid);
    }

    // Discord presence is recomputed from all running instances, so several at
    // once stay correct and the first to exit doesn't wipe the others' presence.
    if discord_rpc {
        if let Ok(mut map) = app_state.discord_playing.lock() {
            map.insert(id.to_string(), (instance.name.clone(), instance.mc_version.clone()));
        }
        crate::discord::update_presence(&app_state);
    }

    // Wait for exit off the command path. Move `emitter` in so its console reader
    // stays alive for the whole session, and clear the running flag at the end.
    let app_bg = app.clone();
    let id_bg = id.to_string();
    let game_start = std::time::SystemTime::now();
    tauri::async_runtime::spawn(async move {
        let _keep_emitter = emitter;
        let code = match child.wait().await {
            Ok(status) => status.code(),
            Err(_) => None,
        };
        if let Some(state) = app_bg.try_state::<AppState>() {
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
        // Post-exit hook (fire-and-forget).
        if let Some(cmd) = post_exit.as_deref().filter(|s| !s.trim().is_empty()) {
            run_hook(cmd, false);
        }

        // Crash detection: non-zero exit code → look for a crash report created
        // after the game started. If found, emit mc://crashed instead of mc://exited.
        let is_crash = code.map(|c| c != 0).unwrap_or(false);
        if is_crash {
            let crash_rel = find_latest_crash_report(&id_bg, game_start);
            let _ = app_bg.emit(
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

/// Runs a user launch hook through the OS shell. `wait` blocks until it finishes.
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

/// Re-installs (verifies/repairs) an instance's files without launching it.
#[tauri::command]
pub async fn repair_instance(app: AppHandle, id: String) -> Result<(), String> {
    let instance: Instance =
        store::read_json(&paths::instance_config_file(&id))?.ok_or("instance not found")?;
    let settings = get_settings()?;
    // Install doesn't authenticate; a placeholder identity is enough.
    let auth = AuthMethod::Offline { username: "Player".into(), uuid: None };
    let memory_mb = instance.memory_mb.unwrap_or(settings.default_memory_mb) as u64;

    let builder = ConfigBuilder::new(paths::instance_game_dir(&id), instance.mc_version.clone(), auth)
        .memory(Memory::Megabyte(memory_mb))
        .runtime_dir(paths::runtimes_dir());
    let emitter = build_emitter(&app, &id).await;

    let result = match to_lyceris_loader(&instance.loader, &instance.mc_version) {
        None => install(&builder.build(), Some(&emitter)).await,
        Some(loader) => install(&builder.loader(loader).build(), Some(&emitter)).await,
    };
    // Clear the titlebar activity regardless of outcome.
    let _ = app.emit("mc://exited", ExitInfo { instance_id: id, code: Some(0) });
    result.map_err(|e| format!("repair failed: {e}"))
}

/// Whether an instance is currently running (for UI state on reload).
#[tauri::command]
pub fn is_instance_running(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    let running = state.running.lock().map_err(|e| e.to_string())?;
    Ok(running.contains(&id))
}

/// Asks the running game to close (graceful) — `force` hard-kills it instead.
/// The process tree is targeted so the JVM and its children all stop.
#[tauri::command]
pub fn stop_instance(state: State<'_, AppState>, id: String, force: bool) -> Result<(), String> {
    let pid = {
        let pids = state.pids.lock().map_err(|e| e.to_string())?;
        pids.get(&id).copied().ok_or("instance is not running")?
    };
    kill_process_tree(pid, force)?;

    // Instances owned by this session are cleaned up by their wait-task on exit.
    // Adopted ones (from a previous launcher run) have no such task, so once
    // we've killed them, clear their state here.
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
        remove_lock(&id);
    }
    Ok(())
}

// ---------- crash report detection ----------

/// Scans `crash-reports/` for the newest `.txt` or `.log` file that was
/// modified *after* `since`. Returns a relative path like
/// `"crash-reports/crash-2026-06-22_13.21.01-client.txt"`, or `None`.
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
            continue; // older than game start — not caused by this session
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

// ---------- run locks (cross-restart double-launch protection) ----------

/// Writes the instance's run lock holding the game PID.
fn write_lock(id: &str, pid: u32) {
    let _ = std::fs::write(paths::instance_lock_file(id), pid.to_string());
}

/// Reads the PID from an instance's run lock, if present and valid.
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

/// True if a process with `pid` exists and looks like a JVM. The name check
/// guards against the OS reusing a dead game's PID for an unrelated process.
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

/// On startup, adopt any instances still running from a previous launcher session
/// (live lock files) into the in-memory state so the UI shows them as running and
/// stop/kill works; clear locks left behind by games that have since exited.
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
            // Spawn a watcher so the UI updates when the game eventually exits.
            spawn_adopted_watcher(app.clone(), id, pid);
        } else {
            remove_lock(&id);
        }
    }
}

/// Polls the adopted process every 4 seconds. When Java exits, cleans up state
/// and emits `mc://exited` so the frontend stops showing the instance as running.
fn spawn_adopted_watcher(app: AppHandle, id: String, pid: u32) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(4)).await;
            if !is_game_pid_alive(pid) {
                if let Some(state) = app.try_state::<AppState>() {
                    if let Ok(mut r) = state.running.lock() { r.remove(&id); }
                    if let Ok(mut p) = state.pids.lock()    { p.remove(&id); }
                    if let Ok(mut a) = state.adopted.lock() { a.remove(&id); }
                    // Update Discord presence in case it was shown for this instance.
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
        // Use the process group (negative PID) so the whole JVM + child tree is
        // signalled, mirroring `taskkill /T` on Windows. Fallback to bare kill
        // if getpgid fails (shouldn't happen for a live process).
        let pgid = unsafe { libc::getpgid(pid as libc::pid_t) };
        let target = if pgid > 0 {
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

// ===== deep link (`spectra://launch/<instance-id>`) =====

/// `spectra://launch/<id>` → the instance id, if that is what this URL is.
/// Ids are UUIDs we minted, so anything else is rejected rather than passed on.
pub fn instance_id_from_url(url: &str) -> Option<String> {
    let rest = url.strip_prefix("spectra://")?.trim_start_matches('/');
    let id = rest.strip_prefix("launch/")?.trim_end_matches('/');
    let plain = !id.is_empty()
        && id.len() <= 64
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    plain.then(|| id.to_string())
}

/// Hands the frontend the instance a shortcut asked for when the click is what
/// started the launcher. Returns `None` once consumed.
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
        // no path traversal or shell games in something we hand to a launcher
        assert_eq!(instance_id_from_url("spectra://launch/../../etc/passwd"), None);
        assert_eq!(instance_id_from_url("spectra://launch/a b"), None);
    }
}
