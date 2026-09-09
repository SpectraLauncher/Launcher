use base64::Engine;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::commands::auth::refresh_active_account;
use crate::models::{AccountKind, SavedSkin};
use crate::{paths, store};
use crate::error::{AppError, AppResult};

fn http() -> AppResult<&'static reqwest::Client> {
    Ok(crate::http())
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

fn load_index() -> AppResult<Vec<SavedSkin>> {
    Ok(store::read_json::<Vec<SavedSkin>>(&skins_index_file())?.unwrap_or_default())
}

fn save_index(skins: &[SavedSkin]) -> AppResult<()> {
    store::write_json(&skins_index_file(), &skins.to_vec())
}

#[tauri::command]
pub async fn list_skins() -> AppResult<Vec<SavedSkin>> {
    crate::blocking(load_index).await
}

#[tauri::command]
pub async fn save_skin(
    name: String,
    model: String,
    source_path: String,
) -> AppResult<SavedSkin> {
    crate::blocking(move || store_skin(name, model, source_path)).await
}

fn store_skin(name: String, model: String, source_path: String) -> AppResult<SavedSkin> {
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
pub async fn set_skin_model(id: String, model: String) -> AppResult<()> {
    crate::blocking(move || {
        let m = if model == "slim" { "slim" } else { "classic" };
        let mut index = load_index()?;
        if let Some(skin) = index.iter_mut().find(|s| s.id == id) {
            skin.model = m.to_string();
            save_index(&index)?;
        }
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn delete_skin(id: String) -> AppResult<()> {
    crate::blocking(move || {
        let _ = std::fs::remove_file(skin_png_file(&id));
        let mut index = load_index()?;
        index.retain(|s| s.id != id);
        save_index(&index)
    })
    .await
}

#[tauri::command]
pub fn get_skin_path(id: String) -> AppResult<String> {
    let path = skin_png_file(&id);
    if !path.exists() {
        return Err(AppError::not_found(format!("skin '{id}' not found")));
    }
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub async fn get_skin_data_url(id: String) -> AppResult<String> {
    crate::blocking(move || {
        let bytes =
            std::fs::read(skin_png_file(&id)).map_err(|_| format!("skin '{id}' not found"))?;
        Ok(png_data_url(&bytes))
    })
    .await
}

#[tauri::command]
pub async fn fetch_skin_data_url(url: String) -> AppResult<String> {
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

async fn fetch_player_skin(uuid: &str) -> AppResult<(Vec<u8>, bool)> {
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
pub async fn get_player_skin(uuid: String) -> AppResult<PlayerSkin> {
    let (bytes, slim) = fetch_player_skin(&uuid).await?;
    Ok(PlayerSkin { skin: png_data_url(&bytes), slim })
}

#[tauri::command]
pub async fn import_player_skin(uuid: String, name: String) -> AppResult<SavedSkin> {
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
pub async fn get_player_capes() -> AppResult<Vec<Cape>> {
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
        return Err(AppError::network(format!("profile failed: {}", resp.status())));
    }
    let p: ProfileCapesResponse = resp.json().await.map_err(|e| e.to_string())?;
    Ok(p.capes
        .into_iter()
        .map(|c| Cape { id: c.id, url: c.url, alias: c.alias, active: c.state.eq_ignore_ascii_case("ACTIVE") })
        .collect())
}

#[tauri::command]
pub async fn set_active_cape(cape_id: Option<String>) -> AppResult<()> {
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
        return Err(AppError::network(format!("set cape failed: {}", resp.status())));
    }
    Ok(())
}

#[tauri::command]
pub async fn apply_skin(id: String) -> AppResult<()> {
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
        return Err(AppError::network(format!("skin upload failed: {}", resp.status())));
    }

    let mut index = load_index()?;
    mark_active(&mut index, &id);
    save_index(&index)?;
    Ok(())
}

const DEFAULT_SKINS: [(&str, &str); 9] = [
    ("Steve", "wide"),
    ("Alex", "slim"),
    ("Zuri", "wide"),
    ("Sunny", "wide"),
    ("Noor", "slim"),
    ("Makena", "slim"),
    ("Kai", "wide"),
    ("Efe", "slim"),
    ("Ari", "wide"),
];

#[derive(Serialize)]
pub struct DefaultSkin {
    pub name: String,
    pub model: String,
    pub path: Option<String>,
    pub url: Option<String>,
}

fn default_skins_dir() -> PathBuf {
    paths::cache_dir().join("default-skins")
}

fn client_jars() -> Vec<PathBuf> {
    let mut jars = Vec::new();
    let Ok(instances) = std::fs::read_dir(paths::instances_dir()) else { return jars };
    for instance in instances.flatten() {
        let versions = instance.path().join("minecraft").join("versions");
        let Ok(dirs) = std::fs::read_dir(&versions) else { continue };
        for dir in dirs.flatten() {
            let name = dir.file_name().to_string_lossy().into_owned();
            let jar = dir.path().join(format!("{name}.jar"));
            if jar.is_file() {
                jars.push(jar);
            }
        }
    }
    jars.sort_by_key(|p| {
        std::cmp::Reverse(
            std::fs::metadata(p)
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH),
        )
    });
    jars
}

fn extract_defaults_from(jar: &std::path::Path, dir: &std::path::Path) -> usize {
    use std::io::Read;

    let Ok(file) = std::fs::File::open(jar) else { return 0 };
    let Ok(mut archive) = zip::ZipArchive::new(file) else { return 0 };
    if std::fs::create_dir_all(dir).is_err() {
        return 0;
    }

    let mut written = 0;
    for (name, variant) in DEFAULT_SKINS {
        let lower = name.to_lowercase();
        let entry =
            format!("assets/minecraft/textures/entity/player/{variant}/{lower}.png");
        let Ok(mut file) = archive.by_name(&entry) else { continue };
        let mut bytes = Vec::new();
        if file.read_to_end(&mut bytes).is_err() || bytes.is_empty() {
            continue;
        }
        if std::fs::write(dir.join(format!("{lower}.png")), &bytes).is_ok() {
            written += 1;
        }
    }
    written
}

#[tauri::command]
pub async fn list_default_skins() -> AppResult<Vec<DefaultSkin>> {
    crate::blocking(|| {
        let dir = default_skins_dir();
        let complete = DEFAULT_SKINS
            .iter()
            .all(|(name, _)| dir.join(format!("{}.png", name.to_lowercase())).is_file());

        if !complete {
            for jar in client_jars() {
                if extract_defaults_from(&jar, &dir) == DEFAULT_SKINS.len() {
                    break;
                }
            }
        }

        let mut out = Vec::new();
        for (name, variant) in DEFAULT_SKINS {
            let model = if variant == "slim" { "slim" } else { "classic" };
            let file = dir.join(format!("{}.png", name.to_lowercase()));
            if file.is_file() {
                out.push(DefaultSkin {
                    name: name.to_string(),
                    model: model.to_string(),
                    path: Some(file.to_string_lossy().into_owned()),
                    url: None,
                });
            } else if let Some(url) = fallback_url(name) {
                out.push(DefaultSkin {
                    name: name.to_string(),
                    model: model.to_string(),
                    path: None,
                    url: Some(url.to_string()),
                });
            }
        }
        Ok(out)
    })
    .await
}

fn fallback_url(name: &str) -> Option<&'static str> {
    match name {
        "Steve" => Some("https://assets.mojang.com/SkinTemplates/steve.png"),
        "Alex" => Some("https://assets.mojang.com/SkinTemplates/alex.png"),
        _ => None,
    }
}

#[cfg(test)]
mod default_skin_tests {
    use super::{extract_defaults_from, DEFAULT_SKINS};
    use std::io::Write;

    #[test]
    fn pulls_every_default_skin_out_of_a_client_jar() {
        let root = std::env::temp_dir().join(format!("spectra-skins-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let jar_path = root.join("client.jar");

        let file = std::fs::File::create(&jar_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        for (name, variant) in DEFAULT_SKINS {
            let entry = format!(
                "assets/minecraft/textures/entity/player/{variant}/{}.png",
                name.to_lowercase()
            );
            zip.start_file(entry, zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(format!("png-{name}").as_bytes()).unwrap();
        }
        zip.start_file("assets/other.png", zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(b"ignored").unwrap();
        zip.finish().unwrap();

        let out = root.join("out");
        assert_eq!(extract_defaults_from(&jar_path, &out), DEFAULT_SKINS.len());
        assert_eq!(std::fs::read_to_string(out.join("zuri.png")).unwrap(), "png-Zuri");
        assert!(!out.join("other.png").exists());

        let empty = root.join("empty.jar");
        std::fs::write(&empty, b"not a zip").unwrap();
        assert_eq!(extract_defaults_from(&empty, &out), 0);

        std::fs::remove_dir_all(&root).unwrap();
    }
}
