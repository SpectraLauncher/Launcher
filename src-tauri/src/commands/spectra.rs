//! Spectra account — the launcher half of the website's login.
//!
//! Signing in happens in the browser: the launcher opens `/launcher/auth`, the
//! site mints a one-time token and hands it back through the `spectra://` deep
//! link the launcher already registers for share codes. That token is exchanged
//! here for a session token, which every later call sends as a bearer.
//!
//! The token lives in its own file rather than in `launcher.json`, because the
//! settings blob is handed to the frontend wholesale and a session token has no
//! business being there.
//!
//! Everything the account UI needs (friends, notifications, invites) goes
//! through one authenticated passthrough instead of a command per endpoint —
//! the server already validates each route, so a second copy of that logic in
//! Rust would only be a second place to get it wrong.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter};

use crate::{paths, store};

pub const SITE: &str = "https://spectra.makoto.com.pl";

/// better-auth refuses requests with no `Origin` (CSRF); a desktop client has
/// to name the origin it is talking to.
pub const ORIGIN: &str = SITE;

#[derive(Default, Serialize, Deserialize)]
struct AccountFile {
    #[serde(default)]
    token: Option<String>,
}

/// The stored session token, if the user has signed in.
pub fn stored_token() -> Option<String> {
    store::read_json::<AccountFile>(&paths::spectra_account_file())
        .ok()
        .flatten()
        .and_then(|f| f.token)
        .filter(|t| !t.is_empty())
}

fn save_token(token: Option<String>) -> Result<(), String> {
    store::write_json(&paths::spectra_account_file(), &AccountFile { token })
}

fn client() -> reqwest::Client {
    reqwest::Client::new()
}

/// One authenticated call against the Spectra API. `path` is server-relative.
async fn call(method: &str, path: &str, body: Option<Value>) -> Result<Value, String> {
    if !path.starts_with("/api/") {
        return Err("refusing to call a non-API path".into());
    }
    let mut req = match method {
        "GET" => client().get(format!("{SITE}{path}")),
        "POST" => client().post(format!("{SITE}{path}")),
        "PATCH" => client().patch(format!("{SITE}{path}")),
        "DELETE" => client().delete(format!("{SITE}{path}")),
        other => return Err(format!("unsupported method {other}")),
    }
    .header("origin", ORIGIN);

    if let Some(token) = stored_token() {
        req = req.bearer_auth(token);
    }
    if let Some(body) = body {
        req = req.json(&body);
    }

    let resp = req.send().await.map_err(|e| format!("network error: {e}"))?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();

    if !status.is_success() {
        // Signed out on the server (token revoked, password changed): drop the
        // dead token so the UI goes back to a sign-in button instead of looping.
        if status == reqwest::StatusCode::UNAUTHORIZED {
            let _ = save_token(None);
        }
        return Err(message_of(&text).unwrap_or_else(|| format!("request failed ({status})")));
    }
    if text.is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(&text).map_err(|e| format!("bad server reply: {e}"))
}

/// Pulls the human-readable part out of an h3/better-auth error body.
fn message_of(body: &str) -> Option<String> {
    let v: Value = serde_json::from_str(body).ok()?;
    v.get("statusMessage")
        .or_else(|| v.get("message"))
        .and_then(|m| m.as_str())
        .map(|s| s.to_string())
}

/// Where the browser sends the user to sign in.
#[tauri::command]
pub fn spectra_login_url() -> String {
    format!("{SITE}/launcher/auth")
}

/// The signed-in user, or `None`. Also the liveness check for a stored token.
#[tauri::command]
pub async fn spectra_session() -> Option<Value> {
    if stored_token().is_none() {
        return None;
    }
    match call("GET", "/api/auth/get-session", None).await {
        Ok(Value::Null) => None,
        Ok(v) => v.get("user").cloned(),
        Err(_) => None,
    }
}

/// Forgets the session locally and on the server.
#[tauri::command]
pub async fn spectra_logout() -> Result<(), String> {
    let _ = call("POST", "/api/auth/sign-out", Some(serde_json::json!({}))).await;
    save_token(None)
}

/// Authenticated passthrough for the account UI (friends, notifications, …).
#[tauri::command]
pub async fn spectra_api(method: String, path: String, body: Option<Value>) -> Result<Value, String> {
    call(&method, &path, body).await
}

/// Tells the site which Minecraft profile this account plays as.
///
/// The launcher is the only client that can prove it: it holds a Minecraft
/// access token from the Xbox chain, and the site asks Mojang whose token it is
/// rather than believing a name typed by the client. The token is sent once and
/// never stored anywhere but the account file it already lives in.
///
/// Quietly does nothing when there is no Spectra session, no Minecraft account,
/// or the active one is offline — none of those are errors worth a dialog.
#[tauri::command]
pub async fn spectra_link_minecraft() -> Result<Option<Value>, String> {
    if stored_token().is_none() {
        return Ok(None);
    }
    let account = match crate::commands::auth::refresh_active_account().await {
        Ok(account) => account,
        Err(_) => return Ok(None), // nobody signed in to the game
    };
    if account.kind == crate::models::AccountKind::Offline || account.access_token.is_empty() {
        return Ok(None);
    }

    call(
        "POST",
        "/api/me/minecraft",
        Some(serde_json::json!({ "token": account.access_token })),
    )
    .await
    .map(Some)
}

/// Turns the one-time token from `spectra://auth/<token>` into a session.
pub async fn redeem_login(app: AppHandle, token: String) {
    let result = call(
        "POST",
        "/api/auth/one-time-token/verify",
        Some(serde_json::json!({ "token": token })),
    )
    .await
    .and_then(|v| {
        v.get("session")
            .and_then(|s| s.get("token"))
            .and_then(|t| t.as_str())
            .map(|t| t.to_string())
            .ok_or_else(|| "the site did not return a session".to_string())
    });

    match result {
        Ok(session_token) => {
            if let Err(e) = save_token(Some(session_token)) {
                log::error!("could not store the Spectra session: {e}");
            }
            let _ = app.emit("spectra://account", ());
        }
        Err(e) => {
            log::error!("Spectra sign-in failed: {e}");
            let _ = app.emit("spectra://auth-failed", e);
        }
    }
}

/// `spectra://auth/<token>` → the one-time token, if that is what this URL is.
pub fn login_token_from_url(url: &str) -> Option<String> {
    let rest = url.strip_prefix("spectra://")?.trim_start_matches('/');
    let token = rest.strip_prefix("auth/")?.trim_matches('/');
    let ok = !token.is_empty()
        && token.len() <= 128
        && token.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    ok.then(|| token.to_string())
}

#[cfg(test)]
mod tests {
    use super::login_token_from_url;

    #[test]
    fn parses_login_links() {
        assert_eq!(login_token_from_url("spectra://auth/abc123"), Some("abc123".into()));
        assert_eq!(login_token_from_url("spectra://auth/abc-123_x/"), Some("abc-123_x".into()));
        // Not a login link, and not something to feed the API.
        assert_eq!(login_token_from_url("spectra://share/ABC123"), None);
        assert_eq!(login_token_from_url("spectra://auth/"), None);
        assert_eq!(login_token_from_url("spectra://auth/tok en"), None);
    }
}
