//! Microsoft (online) authentication, wrapping `lyceris::auth::microsoft`.
//!
//! Flow (driven by the frontend):
//!   1. `auth_get_login_url()` -> open it in a Tauri WebviewWindow.
//!   2. Watch that window's navigation; when it reaches the redirect URI
//!      (`https://login.live.com/oauth20_desktop.srf?code=...`), grab `code`.
//!   3. `auth_login_with_code(code)` -> exchanges the code, stores the account,
//!      makes it active, and returns it.
//!
//! Tokens are persisted in `accounts.json`; `auth_refresh_active()` renews the
//! access token before launch when it's near expiry.

use std::sync::{Arc, Mutex};

use lyceris::auth::microsoft;
use reqwest::Client;
use tauri::{AppHandle, WebviewUrl, WebviewWindowBuilder, WindowEvent};

use crate::models::{Account, AccountKind, AccountsFile};
use crate::{paths, store};

/// Microsoft's fixed desktop redirect for this client id; the auth code arrives
/// as a `?code=` query param on a page under this URL.
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
    Ok(store::read_json::<AccountsFile>(&paths::accounts_file())?.unwrap_or_default())
}

fn save_accounts(file: &AccountsFile) -> Result<(), String> {
    store::write_json(&paths::accounts_file(), file)
}

/// Inserts or replaces an account (matched by uuid) and marks it active.
fn upsert_account(file: &mut AccountsFile, account: Account) {
    if let Some(existing) = file.accounts.iter_mut().find(|a| a.uuid == account.uuid) {
        *existing = account.clone();
    } else {
        file.accounts.push(account.clone());
    }
    file.active_uuid = Some(account.uuid);
}

/// Step 1: the Microsoft OAuth URL to present to the user.
#[tauri::command]
pub fn auth_get_login_url() -> Result<String, String> {
    microsoft::create_link().map_err(|e| e.to_string())
}

/// One-shot Microsoft login: opens an embedded webview at the OAuth URL,
/// intercepts navigation to the redirect to grab the `code`, then exchanges it
/// and persists the account. This is the flow the UI should call.
///
/// Note: building a webview from a command thread is reliable on Windows; if we
/// later support macOS/Linux this may need to move onto the main thread.
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
                    return false; // stop here; we have what we need
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

    // If the user closes the window before finishing, treat it as cancellation.
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

/// Lower-level: exchange an already-obtained authorization `code`. Kept for
/// custom auth flows / testing.
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

/// Minecraft usernames: 3–16 chars, letters/digits/underscore.
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

/// Creates (or re-activates) an offline account. No Microsoft login required;
/// usable for singleplayer and offline-mode servers only. The uuid is generated
/// once and persisted so the account keeps its identity (and worlds) across runs.
#[tauri::command]
pub fn auth_login_offline(username: String) -> Result<Account, String> {
    validate_username(&username)?;

    let mut file = load_accounts()?;

    // Reuse an existing offline account with the same name instead of duplicating.
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

/// Returns the active account, refreshing its token first. Used internally by
/// launch; also exposed so the UI can validate the session on startup.
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

    // Offline accounts have nothing to refresh.
    if current.kind == AccountKind::Offline {
        return Ok(current);
    }

    let client = Client::new();

    // One retry: a refresh that fails because the network blinked is worth
    // trying again before telling somebody to sign in, and it costs a second.
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

/// Whether a failed refresh is worth retrying — the connection dropped or the
/// request timed out, rather than Microsoft saying no.
fn is_transient(e: &lyceris::error::Error) -> bool {
    match e {
        lyceris::error::Error::Reqwest(re) => re.is_connect() || re.is_timeout(),
        lyceris::error::Error::Timeout(_) => true,
        _ => false,
    }
}

/// Turns a refresh failure into something a player can act on.
///
/// `lyceris` deserializes Microsoft's reply without looking at the status code,
/// so a rejected refresh token — expired after a long break, or invalidated by
/// a password change — surfaces as reqwest's "error decoding response body".
/// That tells nobody anything; what it means, in practice, is that the saved
/// session is dead and the account has to sign in again.
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
        // XSTS says things like "this account has no Xbox profile" in plain words.
        lyceris::error::Error::Authentication(msg) => {
            format!("Microsoft refused the session for {who}: {msg}")
        }
        other => format!("Could not renew the session for {who}: {other}"),
    }
}
