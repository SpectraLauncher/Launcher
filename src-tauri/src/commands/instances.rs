//! CRUD for Minecraft instances. Each instance is a folder under `instances/`
//! containing `instance.json` and a `minecraft/` game dir.

use crate::models::{Instance, Loader};
use crate::{paths, store};

/// Lists all instances, newest-created first. Skips folders without a valid
/// `instance.json` instead of failing the whole call.
#[tauri::command]
pub fn list_instances() -> Result<Vec<Instance>, String> {
    let root = paths::instances_dir();
    let mut instances = Vec::new();

    let entries = match std::fs::read_dir(&root) {
        Ok(e) => e,
        Err(_) => return Ok(instances), // dir not created yet
    };

    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().into_owned();
        if let Ok(Some(instance)) = store::read_json::<Instance>(&paths::instance_config_file(&id)) {
            instances.push(instance);
        }
    }

    instances.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(instances)
}

#[tauri::command]
pub fn get_instance(id: String) -> Result<Instance, String> {
    store::read_json::<Instance>(&paths::instance_config_file(&id))?
        .ok_or_else(|| format!("instance '{id}' not found"))
}

/// Creates a new instance folder + `instance.json`. Returns the created record.
/// When `icon_source_path` is given, the image is copied into the instance folder
/// as `icon.png` and `instance.icon` is set to the marker `"icon.png"`.
#[tauri::command]
pub fn create_instance(
    name: String,
    mc_version: String,
    loader: Loader,
    memory_mb: Option<u32>,
    icon_source_path: Option<String>,
) -> Result<Instance, String> {
    let id = uuid::Uuid::new_v4().to_string();

    // Pre-create the game dir (also creates the instance dir) so it's launch-ready.
    let game_dir = paths::instance_game_dir(&id);
    std::fs::create_dir_all(&game_dir).map_err(|e| format!("create game dir: {e}"))?;

    // Pre-create the standard content folders so they're visible/usable right away.
    for sub in [
        "crash-reports",
        "datapacks",
        "mods",
        "resourcepacks",
        "saves",
        "shaderpacks",
    ] {
        std::fs::create_dir_all(game_dir.join(sub))
            .map_err(|e| format!("create {sub} dir: {e}"))?;
    }

    // Copy a chosen icon into the instance folder as icon.png.
    let icon = match icon_source_path {
        Some(src) if !src.trim().is_empty() => {
            std::fs::copy(&src, paths::instance_icon_file(&id))
                .map_err(|e| format!("copy icon: {e}"))?;
            Some("icon.png".to_string())
        }
        _ => None,
    };

    let instance = Instance {
        id: id.clone(),
        name,
        mc_version,
        loader,
        memory_mb,
        icon,
        created_at: chrono::Utc::now().to_rfc3339(),
        ..Default::default()
    };

    store::write_json(&paths::instance_config_file(&id), &instance)?;
    Ok(instance)
}

/// Absolute path to an instance's `icon.png`, or `None` if it has no icon.
/// The UI feeds this to `convertFileSrc` to display it via the asset protocol.
#[tauri::command]
pub fn get_instance_icon_path(id: String) -> Option<String> {
    let path = paths::instance_icon_file(&id);
    if path.exists() {
        Some(path.to_string_lossy().into_owned())
    } else {
        None
    }
}

/// Writes `bytes` as the instance's `icon.png` and records it in `instance.json`.
fn store_icon(id: &str, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > 5 * 1024 * 1024 {
        return Err("image too large (max 5 MB)".into());
    }
    let mut instance = store::read_json::<Instance>(&paths::instance_config_file(id))?
        .ok_or_else(|| format!("instance '{id}' not found"))?;
    std::fs::write(paths::instance_icon_file(id), bytes).map_err(|e| format!("write icon: {e}"))?;
    instance.icon = Some("icon.png".to_string());
    store::write_json(&paths::instance_config_file(id), &instance)
}

/// Sets an instance's icon from a chosen image file (copied to `icon.png`).
#[tauri::command]
pub fn set_instance_icon(id: String, source_path: String) -> Result<(), String> {
    let bytes = std::fs::read(&source_path).map_err(|e| format!("read icon: {e}"))?;
    store_icon(&id, &bytes)
}

/// The user's own icon-editor symbols, newest first, as absolute paths. The UI
/// feeds them to `convertFileSrc` to show them.
#[tauri::command]
pub fn list_custom_symbols() -> Result<Vec<String>, String> {
    let dir = paths::symbols_dir();
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Ok(Vec::new()),
    };
    let mut out: Vec<(std::time::SystemTime, String)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let added = entry
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
        out.push((added, path.to_string_lossy().into_owned()));
    }
    out.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(out.into_iter().map(|(_, p)| p).collect())
}

/// Copies a picked image into the symbols folder and returns its new path.
#[tauri::command]
pub fn add_custom_symbol(source_path: String) -> Result<String, String> {
    let src = std::path::Path::new(&source_path);
    let bytes = std::fs::read(src).map_err(|e| format!("read symbol: {e}"))?;
    if bytes.len() > 5 * 1024 * 1024 {
        return Err("image too large (max 5 MB)".into());
    }
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_lowercase();
    let dir = paths::symbols_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("symbols dir: {e}"))?;
    // Own name, own namespace: whatever the file was called, two picks with the
    // same name never fight, and the name can't escape the folder.
    let dest = dir.join(format!("{}.{ext}", uuid::Uuid::new_v4()));
    std::fs::write(&dest, &bytes).map_err(|e| format!("write symbol: {e}"))?;
    Ok(dest.to_string_lossy().into_owned())
}

/// Deletes one of the user's symbols. Refuses anything outside the folder.
#[tauri::command]
pub fn delete_custom_symbol(path: String) -> Result<(), String> {
    let dir = paths::symbols_dir();
    let target = std::path::Path::new(&path);
    if target.parent() != Some(dir.as_path()) {
        return Err("not a custom symbol".into());
    }
    if target.exists() {
        std::fs::remove_file(target).map_err(|e| format!("delete symbol: {e}"))?;
    }
    Ok(())
}

/// Saves an icon drawn by the in-app editor: a `data:image/png;base64,...` URL
/// straight off a canvas.
#[tauri::command]
pub fn set_instance_icon_data(id: String, data_url: String) -> Result<(), String> {
    use base64::Engine;

    let b64 = data_url.rsplit(',').next().unwrap_or_default();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| format!("decode icon: {e}"))?;
    store_icon(&id, &bytes)
}

/// Overwrites an existing instance's metadata. The `id` must already exist.
#[tauri::command]
pub fn update_instance(instance: Instance) -> Result<(), String> {
    let path = paths::instance_config_file(&instance.id);
    if !path.exists() {
        return Err(format!("instance '{}' not found", instance.id));
    }
    store::write_json(&path, &instance)
}

/// Deletes an instance and all of its data. Irreversible.
#[tauri::command]
pub fn delete_instance(id: String) -> Result<(), String> {
    let dir = paths::instance_dir(&id);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| format!("delete instance: {e}"))?;
    }
    Ok(())
}

/// Reads an image file and returns it as a `data:` URL, for use as an instance
/// icon. Kept small (≤ 5 MB) since it's embedded in `instance.json`.
#[tauri::command]
pub fn read_image_data_url(path: String) -> Result<String, String> {
    use base64::Engine;

    let p = std::path::Path::new(&path);
    let bytes = std::fs::read(p).map_err(|e| format!("read image: {e}"))?;
    if bytes.len() > 5 * 1024 * 1024 {
        return Err("image too large (max 5 MB)".into());
    }
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_lowercase();
    let mime = match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        _ => "image/png",
    };
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:{mime};base64,{b64}"))
}

/// Absolute path to an instance's folder (for "copy path" / "open folder").
#[tauri::command]
pub fn get_instance_path(id: String) -> String {
    paths::instance_dir(&id).to_string_lossy().into_owned()
}

/// Opens the instance folder in the OS file manager.
#[tauri::command]
pub fn open_instance_folder(id: String) -> Result<(), String> {
    let dir = paths::instance_dir(&id);
    if !dir.exists() {
        return Err("instance folder not found".into());
    }
    open_in_file_manager(&dir)
}

/// Opens an instance's Minecraft game folder (`minecraft/`) in the file manager.
#[tauri::command]
pub fn open_instance_game_folder(id: String) -> Result<(), String> {
    let dir = paths::instance_game_dir(&id);
    std::fs::create_dir_all(&dir).map_err(|e| format!("create game dir: {e}"))?;
    open_in_file_manager(&dir)
}

// ===== desktop shortcut =====

/// Trims a name down to something every filesystem accepts as a file name.
fn shortcut_file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if r#"\/:*?"<>|"#.contains(c) || c.is_control() { '_' } else { c })
        .collect();
    let trimmed = cleaned.trim().trim_end_matches('.');
    if trimmed.is_empty() {
        "Instance".to_string()
    } else {
        trimmed.chars().take(80).collect()
    }
}

/// Wraps a PNG in a single-image `.ico` container. Windows has read PNG-backed
/// icon entries since Vista, so no re-encoding is needed — just the 22-byte
/// header the format wants in front.
///
/// ponytail: declares the entry as 256x256 (the `0` size byte) whatever the PNG
/// really is, and Windows scales. Fine for icons; if a source ever needs exact
/// per-size entries, that is where a real encoder comes in.
#[cfg(target_os = "windows")]
fn png_to_ico(png: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(png.len() + 22);
    out.extend_from_slice(&0u16.to_le_bytes()); // reserved
    out.extend_from_slice(&1u16.to_le_bytes()); // type: icon
    out.extend_from_slice(&1u16.to_le_bytes()); // one image
    out.push(0); // width, 0 = 256
    out.push(0); // height, 0 = 256
    out.push(0); // palette size
    out.push(0); // reserved
    out.extend_from_slice(&1u16.to_le_bytes()); // colour planes
    out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
    out.extend_from_slice(&(png.len() as u32).to_le_bytes());
    out.extend_from_slice(&22u32.to_le_bytes()); // offset of the image data
    out.extend_from_slice(png);
    out
}

/// Puts a shortcut to `spectra://launch/<id>` on the desktop, named after the
/// instance and wearing its icon. Returns the file it wrote.
#[tauri::command]
pub fn create_desktop_shortcut(id: String) -> Result<String, String> {
    let instance = store::read_json::<Instance>(&paths::instance_config_file(&id))?
        .ok_or_else(|| format!("instance '{id}' not found"))?;
    let desktop = dirs::desktop_dir().ok_or("no desktop folder")?;
    let name = shortcut_file_name(&instance.name);
    let url = format!("spectra://launch/{id}");
    let icon = paths::instance_icon_file(&id);

    #[cfg(target_os = "windows")]
    {
        // An .url file is the one shortcut kind that can point at a protocol
        // without a helper process in between; its icon has to be an .ico, so
        // the instance icon is re-wrapped next to icon.png.
        let mut body = format!("[InternetShortcut]\r\nURL={url}\r\n");
        let ico = paths::instance_dir(&id).join("icon.ico");
        let png = std::fs::read(&icon).unwrap_or_default();
        if png.starts_with(b"\x89PNG\r\n\x1a\n") {
            std::fs::write(&ico, png_to_ico(&png)).map_err(|e| format!("write icon: {e}"))?;
            body.push_str(&format!("IconFile={}\r\nIconIndex=0\r\n", ico.display()));
        } else if let Ok(exe) = std::env::current_exe() {
            // No usable icon (none set, or an uploaded JPEG): the launcher's own
            // beats a blank page.
            body.push_str(&format!("IconFile={}\r\nIconIndex=0\r\n", exe.display()));
        }
        let path = desktop.join(format!("{name}.url"));
        std::fs::write(&path, body).map_err(|e| format!("write shortcut: {e}"))?;
        return Ok(path.to_string_lossy().into_owned());
    }

    #[cfg(target_os = "linux")]
    {
        let mut body = String::from("[Desktop Entry]\nType=Application\nTerminal=false\n");
        body.push_str(&format!("Name={}\n", instance.name));
        body.push_str(&format!("Exec=xdg-open {url}\n"));
        if icon.exists() {
            body.push_str(&format!("Icon={}\n", icon.display()));
        }
        let path = desktop.join(format!("{name}.desktop"));
        std::fs::write(&path, body).map_err(|e| format!("write shortcut: {e}"))?;
        // A .desktop file only counts as launchable once it is executable.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755));
        }
        return Ok(path.to_string_lossy().into_owned());
    }

    #[cfg(target_os = "macos")]
    {
        // ponytail: a .webloc carries no custom icon (that needs a resource fork
        // written through AppKit); it opens the right instance, wearing Safari's
        // document icon.
        let body = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
             <plist version=\"1.0\"><dict><key>URL</key><string>{url}</string></dict></plist>\n"
        );
        let path = desktop.join(format!("{name}.webloc"));
        std::fs::write(&path, body).map_err(|e| format!("write shortcut: {e}"))?;
        return Ok(path.to_string_lossy().into_owned());
    }

    #[allow(unreachable_code)]
    Err("unsupported platform".into())
}

/// Reveals a file in the OS file manager (selects it where supported).
#[tauri::command]
pub fn reveal_in_explorer(path: String) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    if !p.exists() {
        return Err("file not found".into());
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        // Explorer only understands one exact spelling: `/select,"<path>"` as a
        // single argument, backslashes throughout. Split over two arguments (so
        // a space lands after the comma), or handed a path with the forward
        // slashes the UI joins with, it silently drops the selection and opens
        // the user home folder instead. `raw_arg` keeps std from re-quoting it.
        let arg = format!("/select,\"{}\"", path.replace('/', "\\"));
        std::process::Command::new("explorer")
            .raw_arg(arg)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    std::process::Command::new("open")
        .arg("-R")
        .arg(p)
        .spawn()
        .map_err(|e| e.to_string())?;
    #[cfg(target_os = "linux")]
    {
        // No "select the file" equivalent that every file manager honours.
        desktop_open(p.parent().unwrap_or(p).as_os_str())?;
    }
    Ok(())
}

/// Copies a file (used to "download"/export a screenshot to a chosen location).
#[tauri::command]
pub fn copy_file(from: String, to: String) -> Result<(), String> {
    std::fs::copy(&from, &to).map(|_| ()).map_err(|e| format!("copy file: {e}"))
}

fn open_in_file_manager(path: &std::path::Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        std::process::Command::new("explorer")
            .arg(path)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    std::process::Command::new("open")
        .arg(path)
        .spawn()
        .map_err(|e| e.to_string())?;
    #[cfg(target_os = "linux")]
    desktop_open(path.as_os_str())?;
    Ok(())
}

/// Builds a command for a helper that must run against the *system*, not against
/// this AppImage.
///
/// The AppImage runtime repoints LD_LIBRARY_PATH, GIO_MODULE_DIR, XDG_DATA_DIRS …
/// into the mounted image so the bundled GTK/WebKit load. Children inherit those,
/// so xdg-open — and the browser or file manager it goes on to launch — starts
/// against the image's libraries and dies without a word: the button appears to
/// do nothing. Strip the overrides, keeping whatever the user set themselves.
#[cfg(target_os = "linux")]
fn system_command(program: &str) -> std::process::Command {
    let mut cmd = std::process::Command::new(program);
    let Ok(appdir) = std::env::var("APPDIR") else { return cmd };

    for var in [
        "GDK_PIXBUF_MODULE_FILE", "GIO_MODULE_DIR", "GSETTINGS_SCHEMA_DIR",
        "GST_PLUGIN_SYSTEM_PATH", "GST_PLUGIN_SYSTEM_PATH_1_0", "GTK_DATA_PREFIX",
        "GTK_EXE_PREFIX", "GTK_IM_MODULE_FILE", "GTK_PATH", "LD_PRELOAD", "PERLLIB",
        "PYTHONHOME", "PYTHONPATH", "QT_PLUGIN_PATH",
    ] {
        cmd.env_remove(var);
    }
    // These carry real system entries too — drop only what points into the image.
    for var in ["LD_LIBRARY_PATH", "XDG_DATA_DIRS", "XDG_CONFIG_DIRS", "PATH"] {
        let Ok(value) = std::env::var(var) else { continue };
        let kept: Vec<&str> = value.split(':').filter(|e| !e.starts_with(&appdir)).collect();
        if kept.is_empty() {
            cmd.env_remove(var);
        } else {
            cmd.env(var, kept.join(":"));
        }
    }
    cmd
}

/// Hands a path or URL to the desktop's opener, falling back to `gio` on systems
/// without xdg-utils.
#[cfg(target_os = "linux")]
fn desktop_open(target: &std::ffi::OsStr) -> Result<(), String> {
    match system_command("xdg-open").arg(target).spawn() {
        Ok(_) => Ok(()),
        Err(e) => system_command("gio")
            .arg("open")
            .arg(target)
            .spawn()
            .map(|_| ())
            .map_err(|_| format!("could not open it — is xdg-utils installed? ({e})")),
    }
}

/// Opens a link in the user's browser.
///
/// Deliberately not `@tauri-apps/plugin-shell`: inside an AppImage its helper
/// inherits the image's library paths and fails silently (see `system_command`).
/// Keeps the plugin's scheme check, because links come from mod descriptions.
#[tauri::command]
pub fn open_external(url: String) -> Result<(), String> {
    let url = url.trim();
    if !["http://", "https://", "mailto:", "tel:"].iter().any(|s| url.starts_with(s)) {
        return Err(format!("refusing to open {url}"));
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        std::process::Command::new("explorer")
            .arg(url)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    std::process::Command::new("open").arg(url).spawn().map_err(|e| e.to_string())?;
    #[cfg(target_os = "linux")]
    desktop_open(std::ffi::OsStr::new(url))?;
    Ok(())
}

/// Duplicates an instance: copies the icon and user content (mods, config,
/// saves, resourcepacks, shaderpacks, options.txt) into a new instance. The
/// re-downloadable game files (versions/libraries/assets) are intentionally not
/// copied — they're restored on first launch.
#[tauri::command]
pub fn duplicate_instance(id: String) -> Result<Instance, String> {
    let src = store::read_json::<Instance>(&paths::instance_config_file(&id))?
        .ok_or_else(|| format!("instance '{id}' not found"))?;

    let new_id = uuid::Uuid::new_v4().to_string();
    let src_game = paths::instance_game_dir(&id);
    let dst_game = paths::instance_game_dir(&new_id);
    std::fs::create_dir_all(&dst_game).map_err(|e| format!("create game dir: {e}"))?;

    let src_icon = paths::instance_icon_file(&id);
    let has_icon = src_icon.exists();
    if has_icon {
        let _ = std::fs::copy(&src_icon, paths::instance_icon_file(&new_id));
    }

    for entry in ["mods", "config", "saves", "resourcepacks", "shaderpacks"] {
        let from = src_game.join(entry);
        if from.is_dir() {
            copy_dir_all(&from, &dst_game.join(entry)).map_err(|e| e.to_string())?;
        }
    }
    let options = src_game.join("options.txt");
    if options.is_file() {
        let _ = std::fs::copy(&options, dst_game.join("options.txt"));
    }

    let instance = Instance {
        id: new_id.clone(),
        name: format!("{} (copy)", src.name),
        icon: if has_icon { Some("icon.png".to_string()) } else { None },
        created_at: chrono::Utc::now().to_rfc3339(),
        last_played: None,
        ..src
    };
    store::write_json(&paths::instance_config_file(&new_id), &instance)?;
    Ok(instance)
}

fn copy_dir_all(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let dest = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_all(&entry.path(), &dest)?;
        } else {
            std::fs::copy(entry.path(), dest)?;
        }
    }
    Ok(())
}

/// Stamps `last_played` to now — call when an instance launches.
pub fn touch_last_played(id: &str) -> Result<(), String> {
    if let Some(mut instance) = store::read_json::<Instance>(&paths::instance_config_file(id))? {
        instance.last_played = Some(chrono::Utc::now().to_rfc3339());
        store::write_json(&paths::instance_config_file(id), &instance)?;
    }
    Ok(())
}

/// Adds `seconds` to an instance's accumulated playtime — call when it exits.
pub fn add_playtime(id: &str, seconds: u64) -> Result<(), String> {
    if let Some(mut instance) = store::read_json::<Instance>(&paths::instance_config_file(id))? {
        instance.playtime_seconds = instance.playtime_seconds.saturating_add(seconds);
        store::write_json(&paths::instance_config_file(id), &instance)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::shortcut_file_name;

    #[test]
    fn shortcut_names_stay_writable() {
        assert_eq!(shortcut_file_name("All the Mods 9"), "All the Mods 9");
        assert_eq!(shortcut_file_name("1.20 / fabric: test?"), "1.20 _ fabric_ test_");
        assert_eq!(shortcut_file_name("  trailing.  "), "trailing");
        assert_eq!(shortcut_file_name(""), "Instance");
    }

    /// The .ico is hand-built, so pin its shape: 22 bytes of header, then the
    /// PNG verbatim at the offset the header advertises.
    #[cfg(target_os = "windows")]
    #[test]
    fn ico_wraps_the_png_at_the_declared_offset() {
        let png = b"\x89PNG\r\n\x1a\n-pretend-this-is-an-image";
        let ico = super::png_to_ico(png);

        assert_eq!(&ico[0..2], &0u16.to_le_bytes()); // reserved
        assert_eq!(&ico[2..4], &1u16.to_le_bytes()); // type: icon
        assert_eq!(&ico[4..6], &1u16.to_le_bytes()); // one image
        let len = u32::from_le_bytes(ico[14..18].try_into().unwrap()) as usize;
        let offset = u32::from_le_bytes(ico[18..22].try_into().unwrap()) as usize;
        assert_eq!(len, png.len());
        assert_eq!(offset, 22);
        assert_eq!(&ico[offset..offset + len], png);
    }
}
