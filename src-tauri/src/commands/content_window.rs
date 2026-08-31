use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

pub const LABEL: &str = "content-browser";

fn pending() -> &'static std::sync::Mutex<Option<Value>> {
    static PENDING: std::sync::OnceLock<std::sync::Mutex<Option<Value>>> = std::sync::OnceLock::new();
    PENDING.get_or_init(|| std::sync::Mutex::new(None))
}

fn remember(config: Value) {
    if let Ok(mut slot) = pending().lock() {
        *slot = Some(config);
    }
}

#[tauri::command]
pub async fn open_content_window(app: AppHandle, config: Value) -> Result<(), String> {
    remember(config.clone());

    if let Some(window) = app.get_webview_window(LABEL) {
        window.emit("content://config", config).map_err(|e| e.to_string())?;
        let _ = window.unminimize();
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    let builder = WebviewWindowBuilder::new(&app, LABEL, WebviewUrl::App("browser".into()))
        .title("Spectra")
        .inner_size(1320.0, 860.0)
        .min_inner_size(960.0, 600.0);

    #[cfg(target_os = "macos")]
    let builder = builder
        .decorations(true)
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true)
        .traffic_light_position(tauri::LogicalPosition::new(12.0, 18.0));

    #[cfg(not(target_os = "macos"))]
    let builder = builder.decorations(false);

    builder.build().map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn content_window_config() -> Option<Value> {
    pending().lock().ok().and_then(|slot| slot.clone())
}

#[tauri::command]
pub fn close_content_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(LABEL) {
        window.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn content_installed(app: AppHandle, payload: Value) -> Result<(), String> {
    app.emit_to("main", "content://installed", payload)
        .map_err(|e| e.to_string())
}
