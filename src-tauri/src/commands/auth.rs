use std::sync::{Arc, Mutex};

use lyceris::auth::microsoft;
use reqwest::Client;
use tauri::{AppHandle, WebviewUrl, WebviewWindowBuilder, WindowEvent};

use crate::models::{Account, AccountKind, AccountsFile};
use crate::{paths, store};

const REDIRECT_PREFIX: &str = "https://login.live.com/oauth20_desktop.srf";

impl From<microsoft::MinecraftAccount> for Account {
    fn from(a: microsoft::MinecraftAccount) -> Self {
        Account {
            kind: AccountKind::Microsoft,
            uuid: a.uuid,
            username: a.username,
            xuid: a.xuid,
            access_token: a.access_token,
            refresh_token: a.refresh_token,
            exp: a.exp,
            client_id: a.client_id,
        }
    }
}

fn load_accounts() -> Result<AccountsFile, String> {
    Ok(store::read_json_private::<AccountsFile>(&paths::accounts_file())?.unwrap_or_default())
}

fn save_accounts(file: &AccountsFile) -> Result<(), String> {
    store::write_json_private(&paths::accounts_file(), file)
}

fn upsert_account(file: &mut AccountsFile, account: Account) {
    if let Some(existing) = file.accounts.iter_mut().find(|a| a.uuid == account.uuid) {
        *existing = account.clone();
    } else {
        file.accounts.push(account.clone());
    }
    file.active_uuid = Some(account.uuid);
}

#[tauri::command]
pub fn auth_get_login_url() -> Result<String, String> {
    microsoft::create_link().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn auth_login(app: AppHandle) -> Result<Account, String> {
    let url = microsoft::create_link().map_err(|e| e.to_string())?;
    let parsed: tauri::Url = url.parse().map_err(|e| format!("invalid auth url: {e}"))?;

    let (tx, rx) = tokio::sync::oneshot::channel::<Result<String, String>>();
    let sender = Arc::new(Mutex::new(Some(tx)));

    let take_send = {
        let sender = sender.clone();
        move |result: Result<String, String>| {
            if let Ok(mut guard) = sender.lock() {
                if let Some(tx) = guard.take() {
                    let _ = tx.send(result);
                }
            }
        }
    };

    let nav_send = take_send.clone();
    let window = WebviewWindowBuilder::new(&app, "msa-login", WebviewUrl::External(parsed))
        .title("Sign in with Microsoft")
        .inner_size(520.0, 720.0)
        .center()
        .on_navigation(move |u| {
            if u.as_str().starts_with(REDIRECT_PREFIX) {
                let mut code = None;
                let mut error = None;
                for (k, v) in u.query_pairs() {
                    match k.as_ref() {
                        "code" => code = Some(v.into_owned()),
                        "error" => error = Some(v.into_owned()),
                        _ => {}
                    }
                }
                if let Some(code) = code {
                    nav_send(Ok(code));
                    return false;
                }
                if let Some(error) = error {
                    nav_send(Err(error));
                    return false;
                }
            }
            true
        })
        .build()
        .map_err(|e| format!("failed to open login window: {e}"))?;

    let close_send = take_send.clone();
    window.on_window_event(move |event| {
        if matches!(event, WindowEvent::Destroyed) {
            close_send(Err("login cancelled".to_string()));
        }
    });

    let code = rx.await.map_err(|_| "login cancelled".to_string())??;
    let _ = window.close();

    let client = Client::new();
    let account: Account = microsoft::authenticate(code, &client)
        .await
        .map_err(|e| e.to_string())?
        .into();

    let mut file = load_accounts()?;
    upsert_account(&mut file, account.clone());
    save_accounts(&file)?;
    Ok(account)
}

#[tauri::command]
pub async fn auth_login_with_code(code: String) -> Result<Account, String> {
    let client = Client::new();
    let account: Account = microsoft::authenticate(code, &client)
        .await
        .map_err(|e| e.to_string())?
        .into();

    let mut file = load_accounts()?;
    upsert_account(&mut file, account.clone());
    save_accounts(&file)?;
    Ok(account)
}

fn validate_username(name: &str) -> Result<(), String> {
    let len = name.chars().count();
    if !(3..=16).contains(&len) {
        return Err("username must be 3–16 characters".into());
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err("username may only contain letters, digits and underscore".into());
    }
    Ok(())
}

#[tauri::command]
pub fn auth_login_offline(username: String) -> Result<Account, String> {
    validate_username(&username)?;

    let mut file = load_accounts()?;

    let account = match file
        .accounts
        .iter()
        .find(|a| a.kind == AccountKind::Offline && a.username == username)
    {
        Some(existing) => existing.clone(),
        None => Account {
            kind: AccountKind::Offline,
            uuid: uuid::Uuid::new_v4().to_string(),
            username,
            ..Default::default()
        },
    };

    upsert_account(&mut file, account.clone());
    save_accounts(&file)?;
    Ok(account)
}

#[tauri::command]
pub fn list_accounts() -> Result<AccountsFile, String> {
    load_accounts()
}

#[tauri::command]
pub fn set_active_account(uuid: String) -> Result<(), String> {
    let mut file = load_accounts()?;
    if !file.accounts.iter().any(|a| a.uuid == uuid) {
        return Err(format!("account '{uuid}' not found"));
    }
    file.active_uuid = Some(uuid);
    save_accounts(&file)
}

#[tauri::command]
pub fn remove_account(uuid: String) -> Result<(), String> {
    let mut file = load_accounts()?;
    file.accounts.retain(|a| a.uuid != uuid);
    if file.active_uuid.as_deref() == Some(uuid.as_str()) {
        file.active_uuid = file.accounts.first().map(|a| a.uuid.clone());
    }
    save_accounts(&file)
}

pub async fn refresh_active_account() -> Result<Account, String> {
    let mut file = load_accounts()?;
    let active_uuid = file
        .active_uuid
        .clone()
        .ok_or_else(|| "no active account".to_string())?;

    let current = file
        .accounts
        .iter()
        .find(|a| a.uuid == active_uuid)
        .cloned()
        .ok_or_else(|| "active account missing".to_string())?;

    if current.kind == AccountKind::Offline {
        return Ok(current);
    }

    let client = Client::new();

    let mut attempt = microsoft::refresh(current.refresh_token.clone(), &client).await;
    if matches!(&attempt, Err(e) if is_transient(e)) {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        attempt = microsoft::refresh(current.refresh_token.clone(), &client).await;
    }

    let refreshed: Account = attempt.map_err(|e| explain_refresh_failure(&current, e))?.into();

    upsert_account(&mut file, refreshed.clone());
    save_accounts(&file)?;
    Ok(refreshed)
}

#[tauri::command]
pub async fn auth_refresh_active() -> Result<Account, String> {
    refresh_active_account().await
}

fn is_transient(e: &lyceris::error::Error) -> bool {
    match e {
        lyceris::error::Error::Reqwest(re) => re.is_connect() || re.is_timeout(),
        lyceris::error::Error::Timeout(_) => true,
        _ => false,
    }
}

fn explain_refresh_failure(account: &Account, e: lyceris::error::Error) -> String {
    let who = &account.username;
    match &e {
        lyceris::error::Error::Reqwest(re) if re.is_connect() || re.is_timeout() => {
            format!("Could not reach Microsoft to renew the session for {who}. Check your connection and try again.")
        }
        lyceris::error::Error::Reqwest(re) if re.is_decode() || re.is_status() => {
            format!(
                "Microsoft would not renew the session for {who} — usually because it \
has been too long since the last sign-in, or the password changed. \
Remove the account and add it again."
            )
        }
        lyceris::error::Error::Authentication(msg) => {
            format!("Microsoft refused the session for {who}: {msg}")
        }
        other => format!("Could not renew the session for {who}: {other}"),
    }
}
