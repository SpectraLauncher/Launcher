use crate::models::{Instance, Loader};
use crate::{paths, store};

#[tauri::command]
pub fn list_instances() -> Result<Vec<Instance>, String> {
    let root = paths::instances_dir();
    let mut instances = Vec::new();

    let entries = match std::fs::read_dir(&root) {
        Ok(e) => e,
        Err(_) => return Ok(instances),
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

#[tauri::command]
pub fn create_instance(
    name: String,
    mc_version: String,
    loader: Loader,
    memory_mb: Option<u32>,
    icon_source_path: Option<String>,
) -> Result<Instance, String> {
    let id = uuid::Uuid::new_v4().to_string();

    let game_dir = paths::instance_game_dir(&id);
    std::fs::create_dir_all(&game_dir).map_err(|e| format!("create game dir: {e}"))?;

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

#[tauri::command]
pub fn get_instance_icon_path(id: String) -> Option<String> {
    let path = paths::instance_icon_file(&id);
    if path.exists() {
        Some(path.to_string_lossy().into_owned())
    } else {
        None
    }
}

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

#[tauri::command]
pub fn set_instance_icon(id: String, source_path: String) -> Result<(), String> {
    let bytes = std::fs::read(&source_path).map_err(|e| format!("read icon: {e}"))?;
    store_icon(&id, &bytes)
}

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
    let dest = dir.join(format!("{}.{ext}", uuid::Uuid::new_v4()));
    std::fs::write(&dest, &bytes).map_err(|e| format!("write symbol: {e}"))?;
    Ok(dest.to_string_lossy().into_owned())
}

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

#[tauri::command]
pub fn set_instance_icon_data(id: String, data_url: String) -> Result<(), String> {
    use base64::Engine;

    let b64 = data_url.rsplit(',').next().unwrap_or_default();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| format!("decode icon: {e}"))?;
    store_icon(&id, &bytes)
}

#[tauri::command]
pub fn update_instance(instance: Instance) -> Result<(), String> {
    let path = paths::instance_config_file(&instance.id);
    if !path.exists() {
        return Err(format!("instance '{}' not found", instance.id));
    }
    store::write_json(&path, &instance)
}

#[tauri::command]
pub fn delete_instance(id: String) -> Result<(), String> {
    let dir = paths::instance_dir(&id);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| format!("delete instance: {e}"))?;
    }
    Ok(())
}

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

#[tauri::command]
pub fn get_instance_path(id: String) -> String {
    paths::instance_dir(&id).to_string_lossy().into_owned()
}

#[tauri::command]
pub fn open_instance_folder(id: String) -> Result<(), String> {
    let dir = paths::instance_dir(&id);
    if !dir.exists() {
        return Err("instance folder not found".into());
    }
    open_in_file_manager(&dir)
}

#[tauri::command]
pub fn open_instance_game_folder(id: String) -> Result<(), String> {
    let dir = paths::instance_game_dir(&id);
    std::fs::create_dir_all(&dir).map_err(|e| format!("create game dir: {e}"))?;
    open_in_file_manager(&dir)
}

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

#[cfg(target_os = "windows")]
fn png_to_ico(png: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(png.len() + 22);
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.push(0);
    out.push(0);
    out.push(0);
    out.push(0);
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(&(png.len() as u32).to_le_bytes());
    out.extend_from_slice(&22u32.to_le_bytes());
    out.extend_from_slice(png);
    out
}

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
        let mut body = format!("[InternetShortcut]\r\nURL={url}\r\n");
        let ico = paths::instance_dir(&id).join("icon.ico");
        let png = std::fs::read(&icon).unwrap_or_default();
        if png.starts_with(b"\x89PNG\r\n\x1a\n") {
            std::fs::write(&ico, png_to_ico(&png)).map_err(|e| format!("write icon: {e}"))?;
            body.push_str(&format!("IconFile={}\r\nIconIndex=0\r\n", ico.display()));
        } else if let Ok(exe) = std::env::current_exe() {
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
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755));
        }
        return Ok(path.to_string_lossy().into_owned());
    }

    #[cfg(target_os = "macos")]
    {
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
        desktop_open(p.parent().unwrap_or(p).as_os_str())?;
    }
    Ok(())
}

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

pub fn touch_last_played(id: &str) -> Result<(), String> {
    if let Some(mut instance) = store::read_json::<Instance>(&paths::instance_config_file(id))? {
        instance.last_played = Some(chrono::Utc::now().to_rfc3339());
        store::write_json(&paths::instance_config_file(id), &instance)?;
    }
    Ok(())
}

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

    #[cfg(target_os = "windows")]
    #[test]
    fn ico_wraps_the_png_at_the_declared_offset() {
        let png = b"\x89PNG\r\n\x1a\n-pretend-this-is-an-image";
        let ico = super::png_to_ico(png);

        assert_eq!(&ico[0..2], &0u16.to_le_bytes());
        assert_eq!(&ico[2..4], &1u16.to_le_bytes());
        assert_eq!(&ico[4..6], &1u16.to_le_bytes());
        let len = u32::from_le_bytes(ico[14..18].try_into().unwrap()) as usize;
        let offset = u32::from_le_bytes(ico[18..22].try_into().unwrap()) as usize;
        assert_eq!(len, png.len());
        assert_eq!(offset, 22);
        assert_eq!(&ico[offset..offset + len], png);
    }
}
