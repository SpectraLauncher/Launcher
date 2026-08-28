use base64::Engine;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::commands::auth::refresh_active_account;
use crate::models::{AccountKind, SavedSkin};
use crate::{paths, store};

const USER_AGENT: &str = concat!("Spectra-Launcher/", env!("CARGO_PKG_VERSION"));

fn http() -> Result<reqwest::Client, String> {
    reqwest::Client::builder().user_agent(USER_AGENT).build().map_err(|e| e.to_string())
}

fn png_data_url(bytes: &[u8]) -> String {
    format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))
}

fn skins_index_file() -> PathBuf {
    paths::skins_dir().join("skins.json")
}

fn skin_png_file(id: &str) -> PathBuf {
    paths::skins_dir().join(format!("{id}.png"))
}

fn load_index() -> Result<Vec<SavedSkin>, String> {
    Ok(store::read_json::<Vec<SavedSkin>>(&skins_index_file())?.unwrap_or_default())
}

fn save_index(skins: &[SavedSkin]) -> Result<(), String> {
    store::write_json(&skins_index_file(), &skins.to_vec())
}

#[tauri::command]
pub fn list_skins() -> Result<Vec<SavedSkin>, String> {
    load_index()
}

#[tauri::command]
pub fn save_skin(name: String, model: String, source_path: String) -> Result<SavedSkin, String> {
    std::fs::create_dir_all(paths::skins_dir()).map_err(|e| e.to_string())?;

    let id = uuid::Uuid::new_v4().to_string();
    std::fs::copy(&source_path, skin_png_file(&id))
        .map_err(|e| format!("copy skin: {e}"))?;

    let skin = SavedSkin {
        id,
        name,
        model: if model == "slim" { "slim".into() } else { "classic".into() },
        active: false,
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    let mut index = load_index()?;
    index.push(skin.clone());
    save_index(&index)?;
    Ok(skin)
}

fn mark_active(index: &mut [SavedSkin], id: &str) {
    for s in index.iter_mut() {
        s.active = s.id == id;
    }
}

#[tauri::command]
pub fn set_skin_model(id: String, model: String) -> Result<(), String> {
    let m = if model == "slim" { "slim" } else { "classic" };
    let mut index = load_index()?;
    if let Some(skin) = index.iter_mut().find(|s| s.id == id) {
        skin.model = m.to_string();
        save_index(&index)?;
    }
    Ok(())
}

#[tauri::command]
pub fn delete_skin(id: String) -> Result<(), String> {
    let _ = std::fs::remove_file(skin_png_file(&id));
    let mut index = load_index()?;
    index.retain(|s| s.id != id);
    save_index(&index)
}

#[tauri::command]
pub fn get_skin_path(id: String) -> Result<String, String> {
    let path = skin_png_file(&id);
    if !path.exists() {
        return Err(format!("skin '{id}' not found"));
    }
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn get_skin_data_url(id: String) -> Result<String, String> {
    let bytes = std::fs::read(skin_png_file(&id)).map_err(|_| format!("skin '{id}' not found"))?;
    Ok(png_data_url(&bytes))
}

#[tauri::command]
pub async fn fetch_skin_data_url(url: String) -> Result<String, String> {
    let bytes = http()?.get(&url).send().await.map_err(|e| e.to_string())?
        .bytes().await.map_err(|e| e.to_string())?;
    Ok(png_data_url(&bytes))
}

#[derive(Serialize)]
pub struct PlayerSkin {
    skin: String,
    slim: bool,
}

#[derive(Deserialize)]
struct SessionProfile {
    properties: Vec<SessionProperty>,
}
#[derive(Deserialize)]
struct SessionProperty {
    name: String,
    value: String,
}

async fn fetch_player_skin(uuid: &str) -> Result<(Vec<u8>, bool), String> {
    let id = uuid.replace('-', "");
    let client = http()?;
    let profile: SessionProfile = client
        .get(format!("https://sessionserver.mojang.com/session/minecraft/profile/{id}"))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let textures = profile
        .properties
        .iter()
        .find(|p| p.name == "textures")
        .ok_or("profile has no textures")?;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(&textures.value)
        .map_err(|e| e.to_string())?;
    let json: serde_json::Value = serde_json::from_slice(&decoded).map_err(|e| e.to_string())?;

    let skin_node = &json["textures"]["SKIN"];
    let slim = skin_node["metadata"]["model"].as_str() == Some("slim");
    let url = skin_node["url"]
        .as_str()
        .unwrap_or("https://assets.mojang.com/SkinTemplates/steve.png");

    let bytes = client.get(url).send().await.map_err(|e| e.to_string())?
        .bytes().await.map_err(|e| e.to_string())?.to_vec();
    Ok((bytes, slim))
}

#[tauri::command]
pub async fn get_player_skin(uuid: String) -> Result<PlayerSkin, String> {
    let (bytes, slim) = fetch_player_skin(&uuid).await?;
    Ok(PlayerSkin { skin: png_data_url(&bytes), slim })
}

#[tauri::command]
pub async fn import_player_skin(uuid: String, name: String) -> Result<SavedSkin, String> {
    let (bytes, slim) = fetch_player_skin(&uuid).await?;
    let model = if slim { "slim" } else { "classic" };

    let mut index = load_index()?;

    if let Some(existing) = index.iter().find(|s| {
        std::fs::read(skin_png_file(&s.id)).map(|b| b == bytes).unwrap_or(false)
    }) {
        let id = existing.id.clone();
        mark_active(&mut index, &id);
        save_index(&index)?;
        return index.into_iter().find(|s| s.id == id).ok_or("skin vanished".into());
    }

    std::fs::create_dir_all(paths::skins_dir()).map_err(|e| e.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();
    std::fs::write(skin_png_file(&id), &bytes).map_err(|e| format!("write skin: {e}"))?;

    let skin = SavedSkin {
        id: id.clone(),
        name: if name.trim().is_empty() { "Skin".into() } else { name },
        model: model.into(),
        active: true,
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    index.push(skin.clone());
    mark_active(&mut index, &id);
    save_index(&index)?;
    Ok(skin)
}

#[derive(Serialize)]
pub struct Cape {
    pub id: String,
    pub url: String,
    pub alias: String,
    pub active: bool,
}

#[derive(Deserialize)]
struct ProfileCape {
    id: String,
    #[serde(default)]
    state: String,
    url: String,
    #[serde(default)]
    alias: String,
}

#[derive(Deserialize)]
struct ProfileCapesResponse {
    #[serde(default)]
    capes: Vec<ProfileCape>,
}

#[tauri::command]
pub async fn get_player_capes() -> Result<Vec<Cape>, String> {
    let account = refresh_active_account().await?;
    if account.kind != AccountKind::Microsoft {
        return Ok(vec![]);
    }
    let resp = http()?
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(&account.access_token)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("profile failed: {}", resp.status()));
    }
    let p: ProfileCapesResponse = resp.json().await.map_err(|e| e.to_string())?;
    Ok(p.capes
        .into_iter()
        .map(|c| Cape { id: c.id, url: c.url, alias: c.alias, active: c.state.eq_ignore_ascii_case("ACTIVE") })
        .collect())
}

#[tauri::command]
pub async fn set_active_cape(cape_id: Option<String>) -> Result<(), String> {
    let account = refresh_active_account().await?;
    if account.kind != AccountKind::Microsoft {
        return Err("only Microsoft accounts can change capes".into());
    }
    let client = http()?;
    let url = "https://api.minecraftservices.com/minecraft/profile/capes/active";
    let resp = match cape_id {
        Some(id) => {
            client.put(url).bearer_auth(&account.access_token).json(&serde_json::json!({ "capeId": id })).send().await
        }
        None => client.delete(url).bearer_auth(&account.access_token).send().await,
    }
    .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("set cape failed: {}", resp.status()));
    }
    Ok(())
}

#[tauri::command]
pub async fn apply_skin(id: String) -> Result<(), String> {
    let account = refresh_active_account().await?;
    if account.kind != AccountKind::Microsoft {
        return Err("only Microsoft accounts can change their skin".into());
    }

    let skin = load_index()?
        .into_iter()
        .find(|s| s.id == id)
        .ok_or_else(|| format!("skin '{id}' not found"))?;
    let bytes = std::fs::read(skin_png_file(&id)).map_err(|e| format!("read skin: {e}"))?;
    let variant = if skin.model == "slim" { "slim" } else { "classic" };

    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name("skin.png")
        .mime_str("image/png")
        .map_err(|e| e.to_string())?;
    let form = reqwest::multipart::Form::new().text("variant", variant).part("file", part);

    let resp = http()?
        .post("https://api.minecraftservices.com/minecraft/profile/skins")
        .bearer_auth(&account.access_token)
        .multipart(form)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("skin upload failed: {}", resp.status()));
    }

    let mut index = load_index()?;
    mark_active(&mut index, &id);
    save_index(&index)?;
    Ok(())
}
