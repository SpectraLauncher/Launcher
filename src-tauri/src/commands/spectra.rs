use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter};

use crate::{paths, store};

pub const SITE: &str = "https://spectra.makoto.com.pl";

pub const ORIGIN: &str = SITE;

#[derive(Default, Serialize, Deserialize)]
struct AccountFile {
    #[serde(default)]
    token: Option<String>,
}

pub fn stored_token() -> Option<String> {
    store::read_json_private::<AccountFile>(&paths::spectra_account_file())
        .ok()
        .flatten()
        .and_then(|f| f.token)
        .filter(|t| !t.is_empty())
}

fn save_token(token: Option<String>) -> Result<(), String> {
    store::write_json_private(&paths::spectra_account_file(), &AccountFile { token })
}

fn client() -> &'static reqwest::Client {
    crate::http()
}

/// Every endpoint the launcher is allowed to reach with the stored session.
///
/// `call()` is reachable from the webview through the `spectra_api` command, so
/// without this list any script running in the window — a bypass of the
/// markdown sanitiser in the Modrinth browser, say — could drive the whole
/// account API. Adding a screen means adding its route here on purpose.
pub fn allowed(method: &str, path: &str) -> bool {
    let path = path.split('?').next().unwrap_or("");
    let segments: Vec<&str> = path.trim_matches('/').split('/').collect();

    matches!(
        (method, segments.as_slice()),
        ("GET" | "POST", ["api", "friends"])
            | ("PATCH" | "DELETE", ["api", "friends", _])
            | ("GET", ["api", "notifications"])
            | ("DELETE", ["api", "notifications", _])
            | ("POST", ["api", "notifications", "read"])
            | ("POST", ["api", "presence"])
            | ("GET", ["api", "shares"])
            | ("DELETE", ["api", "share", _])
            | ("POST", ["api", "share", _, "extend"])
            | ("POST" | "DELETE", ["api", "share", _, "invite"])
            | ("GET", ["api", "users"])
            | ("POST", ["api", "me", "activity" | "minecraft"])
            | ("GET", ["api", "auth", "get-session"])
            | ("POST", ["api", "auth", "sign-out"])
            | ("POST", ["api", "auth", "one-time-token", "verify"])
    )
}

async fn call(method: &str, path: &str, body: Option<Value>) -> Result<Value, String> {
    if !allowed(method, path) {
        return Err(format!("refusing to call {method} {path}"));
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

fn message_of(body: &str) -> Option<String> {
    let v: Value = serde_json::from_str(body).ok()?;
    v.get("statusMessage")
        .or_else(|| v.get("message"))
        .and_then(|m| m.as_str())
        .map(|s| s.to_string())
}

pub const ACTIVITY_MAX_SECONDS: u64 = 3600;

pub fn activity_chunks(elapsed: u64, reported: u64) -> Vec<u64> {
    let mut left = elapsed.saturating_sub(reported);
    let mut out = Vec::new();
    while left > 0 {
        let chunk = left.min(ACTIVITY_MAX_SECONDS);
        out.push(chunk);
        left -= chunk;
    }
    out
}

pub async fn report_activity(launched: bool, seconds: u64) {
    if stored_token().is_none() {
        return;
    }

    let mut body = serde_json::Map::new();
    if launched {
        body.insert("launched".into(), Value::Bool(true));
    }
    if seconds > 0 {
        body.insert("seconds".into(), Value::from(seconds.min(ACTIVITY_MAX_SECONDS)));
    }
    if body.is_empty() {
        return;
    }

    if let Err(e) = call("POST", "/api/me/activity", Some(Value::Object(body))).await {
        log::warn!("could not report activity: {e}");
    }
}

#[tauri::command]
pub fn spectra_login_url() -> String {
    format!("{SITE}/launcher/auth")
}

#[tauri::command]
pub fn spectra_profile_url(username: String) -> String {
    let clean: String = username
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .collect();
    format!("{SITE}/u/{clean}")
}

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

#[tauri::command]
pub async fn spectra_logout() -> Result<(), String> {
    let _ = call("POST", "/api/auth/sign-out", Some(serde_json::json!({}))).await;
    save_token(None)
}

#[tauri::command]
pub async fn spectra_api(method: String, path: String, body: Option<Value>) -> Result<Value, String> {
    call(&method, &path, body).await
}

#[tauri::command]
pub async fn spectra_link_minecraft() -> Result<Option<Value>, String> {
    if stored_token().is_none() {
        return Ok(None);
    }
    let account = match crate::commands::auth::refresh_active_account().await {
        Ok(account) => account,
        Err(_) => return Ok(None),
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
    use super::{activity_chunks, allowed, login_token_from_url, ACTIVITY_MAX_SECONDS};

    #[test]
    fn only_the_routes_the_launcher_uses_are_reachable() {
        for (method, path) in [
            ("GET", "/api/friends"),
            ("POST", "/api/friends"),
            ("PATCH", "/api/friends/12"),
            ("DELETE", "/api/friends/12"),
            ("GET", "/api/notifications?playing=1"),
            ("POST", "/api/notifications/read"),
            ("DELETE", "/api/notifications/9"),
            ("POST", "/api/presence"),
            ("GET", "/api/shares"),
            ("DELETE", "/api/share/ABC123"),
            ("POST", "/api/share/ABC123/extend"),
            ("POST", "/api/share/ABC123/invite"),
            ("DELETE", "/api/share/ABC123/invite"),
            ("GET", "/api/users?q=mako"),
            ("POST", "/api/me/activity"),
            ("POST", "/api/me/minecraft"),
            ("GET", "/api/auth/get-session"),
            ("POST", "/api/auth/sign-out"),
            ("POST", "/api/auth/one-time-token/verify"),
        ] {
            assert!(allowed(method, path), "should be allowed: {method} {path}");
        }
    }

    #[test]
    fn everything_else_is_refused() {
        for (method, path) in [
            // the admin surface is not the launcher's business
            ("GET", "/api/admin/users"),
            ("POST", "/api/admin/discord/send"),
            ("GET", "/api/admin/stats"),
            // account-altering better-auth routes
            ("POST", "/api/auth/sign-in/email"),
            ("POST", "/api/auth/update-user"),
            ("POST", "/api/auth/change-password"),
            ("POST", "/api/auth/one-time-token/generate"),
            // right shape, wrong method
            ("DELETE", "/api/presence"),
            ("GET", "/api/me/activity"),
            // traversal and near misses
            ("GET", "/api/friends/../admin/users"),
            ("GET", "/api/friendsx"),
            ("GET", "/api/share/ABC123/complete"),
            ("GET", "/"),
            ("GET", "/render/full/mako/full.png"),
        ] {
            assert!(!allowed(method, path), "should be refused: {method} {path}");
        }
    }

    #[test]
    fn splits_activity_into_chunks_the_server_accepts() {
        assert_eq!(activity_chunks(0, 0), Vec::<u64>::new());
        assert_eq!(activity_chunks(120, 120), Vec::<u64>::new());
        assert_eq!(activity_chunks(300, 0), vec![300]);
        assert_eq!(activity_chunks(900, 600), vec![300]);
        assert_eq!(activity_chunks(3600, 0), vec![3600]);
        assert_eq!(activity_chunks(3601, 0), vec![3600, 1]);
        assert_eq!(activity_chunks(10_000, 0), vec![3600, 3600, 2800]);


        assert_eq!(activity_chunks(100, 500), Vec::<u64>::new());

        for chunk in activity_chunks(10_000, 0) {
            assert!(chunk <= ACTIVITY_MAX_SECONDS);
        }
        assert_eq!(activity_chunks(10_000, 0).iter().sum::<u64>(), 10_000);
    }

    #[test]
    fn parses_login_links() {
        assert_eq!(login_token_from_url("spectra://auth/abc123"), Some("abc123".into()));
        assert_eq!(login_token_from_url("spectra://auth/abc-123_x/"), Some("abc-123_x".into()));
        assert_eq!(login_token_from_url("spectra://share/ABC123"), None);
        assert_eq!(login_token_from_url("spectra://auth/"), None);
        assert_eq!(login_token_from_url("spectra://auth/tok en"), None);
    }
}
