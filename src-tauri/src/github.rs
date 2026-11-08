//! GitHub integration: OAuth device flow, profile lookup and repository listing.
//! Every request is made from Rust; the token never enters the webview.

use crate::db::{Db, now};
use crate::secrets;
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::State;

const API: &str = "https://api.github.com";
const DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const CLIENT_ID_KEY: &str = "github.clientId";
const PROFILE_KEY: &str = "github.profile";
const USER_AGENT: &str = "GRAFT/0.1.0 (Linux; Tauri)";
const API_VERSION: &str = "2022-11-28";
const SCOPE: &str = "repo read:user";
const PER_PAGE: u32 = 100;
const TIMEOUT: Duration = Duration::from_secs(25);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct GithubUser {
    pub login: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub avatar_url: String,
    #[serde(default)]
    pub html_url: String,
    #[serde(default)]
    pub public_repos: u32,
    #[serde(default)]
    pub total_private_repos: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct RepoOwner {
    pub login: String,
}

fn default_branch() -> String {
    "main".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct Repo {
    pub id: u64,
    pub name: String,
    pub full_name: String,
    pub private: bool,
    #[serde(default)]
    pub fork: bool,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub stargazers_count: u32,
    #[serde(default)]
    pub forks_count: u32,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub pushed_at: Option<String>,
    #[serde(default = "default_branch")]
    pub default_branch: String,
    pub html_url: String,
    #[serde(default)]
    pub owner: Option<RepoOwner>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
    pub client_id: Option<String>,
    pub signed_in: bool,
    pub user: Option<GithubUser>,
    pub cached_user: Option<GithubUser>,
    pub token_error: Option<String>,
    pub keyring_error: Option<String>,
    pub scopes: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceStart {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum DevicePoll {
    Pending,
    SlowDown { interval: u64 },
    Complete { user: GithubUser, scopes: Option<String> },
    Failed { message: String },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoPage {
    pub page: u32,
    pub repos: Vec<Repo>,
    pub has_more: bool,
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(TIMEOUT)
        .build()
        .map_err(|e| format!("Cannot create the HTTP client: {e}"))
}

fn truncate(value: &str, limit: usize) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= limit {
        trimmed.to_string()
    } else {
        let cut: String = trimmed.chars().take(limit).collect();
        format!("{cut}…")
    }
}

async fn describe_error(response: reqwest::Response) -> String {
    let status = response.status();
    let remaining = response
        .headers()
        .get("x-ratelimit-remaining")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let reset = response
        .headers()
        .get("x-ratelimit-reset")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());
    let body = response.text().await.unwrap_or_default();
    let detail = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|value| {
            value
                .get("message")
                .and_then(|message| message.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| truncate(&body, 160));

    match status.as_u16() {
        401 => "GitHub rejected the stored token (401). Sign in again.".to_string(),
        403 if remaining.as_deref() == Some("0") => match reset {
            Some(epoch) => {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|duration| duration.as_secs())
                    .unwrap_or(0);
                let minutes = epoch.saturating_sub(now) / 60 + 1;
                format!("GitHub rate limit reached. It resets in about {minutes} minutes.")
            }
            None => "GitHub rate limit reached. Try again later.".to_string(),
        },
        404 => format!("GitHub returned 404 — {detail}"),
        _ => format!("GitHub returned {status}: {detail}"),
    }
}

async fn api_get(token: &str, url: &str) -> Result<reqwest::Response, String> {
    let response = client()?
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", API_VERSION)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| format!("Cannot reach api.github.com: {e}"))?;

    if response.status().is_success() {
        Ok(response)
    } else {
        Err(describe_error(response).await)
    }
}

async fn fetch_user(token: &str) -> Result<(GithubUser, Option<String>), String> {
    let response = api_get(token, &format!("{API}/user")).await?;
    let scopes = response
        .headers()
        .get("x-oauth-scopes")
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let user: GithubUser = response
        .json()
        .await
        .map_err(|e| format!("GitHub returned an unreadable profile: {e}"))?;
    Ok((user, scopes))
}

fn remember(db: &Db, user: &GithubUser) -> Result<(), String> {
    let encoded = serde_json::to_string(user).map_err(|e| format!("cannot cache profile: {e}"))?;
    db.set_setting(PROFILE_KEY, &encoded)
}

fn cached_user(db: &Db) -> Option<GithubUser> {
    db.get_setting(PROFILE_KEY)
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str::<GithubUser>(&raw).ok())
}

async fn stored_token() -> Result<Option<String>, String> {
    secrets::load()
}

#[tauri::command]
pub async fn github_status(db: State<'_, Db>) -> Result<AuthStatus, String> {
    let client_id = db.get_setting(CLIENT_ID_KEY)?;
    let cached = cached_user(&db);

    let token = match stored_token().await {
        Ok(token) => token,
        Err(error) => {
            return Ok(AuthStatus {
                client_id,
                signed_in: false,
                user: None,
                cached_user: cached,
                token_error: None,
                keyring_error: Some(error),
                scopes: None,
            })
        }
    };

    let Some(token) = token else {
        return Ok(AuthStatus {
            client_id,
            signed_in: false,
            user: None,
            cached_user: cached,
            token_error: None,
            keyring_error: None,
            scopes: None,
        });
    };

    match fetch_user(&token).await {
        Ok((user, scopes)) => {
            remember(&db, &user)?;
            Ok(AuthStatus {
                client_id,
                signed_in: true,
                user: Some(user),
                cached_user: cached,
                token_error: None,
                keyring_error: None,
                scopes,
            })
        }
        Err(error) => Ok(AuthStatus {
            client_id,
            signed_in: false,
            user: None,
            cached_user: cached,
            token_error: Some(error),
            keyring_error: None,
            scopes: None,
        }),
    }
}

#[tauri::command]
pub fn github_set_client_id(db: State<'_, Db>, client_id: String) -> Result<(), String> {
    let value = client_id.trim();
    if value.is_empty() {
        return db.delete_setting(CLIENT_ID_KEY);
    }
    let valid = (8..=64).contains(&value.chars().count())
        && value.chars().all(|c| c.is_ascii_alphanumeric());
    if !valid {
        return Err(
            "A GitHub OAuth client ID is the 20-character identifier from the OAuth app page."
                .to_string(),
        );
    }
    db.set_setting(CLIENT_ID_KEY, value)
}

#[tauri::command]
pub async fn github_start_device(db: State<'_, Db>) -> Result<DeviceStart, String> {
    let client_id = db
        .get_setting(CLIENT_ID_KEY)?
        .ok_or_else(|| "Add your GitHub OAuth app client ID first.".to_string())?;

    #[derive(Deserialize)]
    struct DeviceCode {
        device_code: String,
        user_code: String,
        verification_uri: String,
        expires_in: u64,
        interval: u64,
    }

    let response = client()?
        .post(DEVICE_CODE_URL)
        .header("Accept", "application/json")
        .form(&[("client_id", client_id.as_str()), ("scope", SCOPE)])
        .send()
        .await
        .map_err(|e| format!("Cannot reach github.com: {e}"))?;

    if !response.status().is_success() {
        return Err(describe_error(response).await);
    }

    let body: DeviceCode = response
        .json()
        .await
        .map_err(|e| format!("GitHub returned an unexpected device code response: {e}"))?;

    Ok(DeviceStart {
        device_code: body.device_code,
        user_code: body.user_code,
        verification_uri: body.verification_uri,
        expires_in: body.expires_in,
        interval: body.interval.max(5),
    })
}

#[derive(Debug, PartialEq)]
enum TokenOutcome {
    Pending,
    SlowDown,
    Token(String),
    Failed(String),
}

fn parse_token_response(body: &str) -> TokenOutcome {
    #[derive(Deserialize)]
    struct Payload {
        access_token: Option<String>,
        error: Option<String>,
        error_description: Option<String>,
    }

    let payload = match serde_json::from_str::<Payload>(body) {
        Ok(payload) => payload,
        Err(_) => {
            return TokenOutcome::Failed(format!(
                "GitHub returned an unreadable token response: {}",
                truncate(body, 120)
            ))
        }
    };

    if let Some(token) = payload.access_token.filter(|token| !token.trim().is_empty()) {
        return TokenOutcome::Token(token);
    }

    match payload.error.as_deref() {
        Some("authorization_pending") => TokenOutcome::Pending,
        Some("slow_down") => TokenOutcome::SlowDown,
        Some("expired_token") => {
            TokenOutcome::Failed("The device code expired. Start the sign-in again.".to_string())
        }
        Some("access_denied") => {
            TokenOutcome::Failed("The request was denied on GitHub.".to_string())
        }
        Some("incorrect_client_credentials") => TokenOutcome::Failed(
            "GitHub rejected the client ID. Check the OAuth app configuration.".to_string(),
        ),
        Some(other) => TokenOutcome::Failed(
            payload
                .error_description
                .unwrap_or_else(|| format!("GitHub returned '{other}'.")),
        ),
        None => TokenOutcome::Failed("GitHub did not return a token.".to_string()),
    }
}

#[tauri::command]
pub async fn github_poll_device(
    db: State<'_, Db>,
    device_code: String,
) -> Result<DevicePoll, String> {
    let client_id = db
        .get_setting(CLIENT_ID_KEY)?
        .ok_or_else(|| "Add your GitHub OAuth app client ID first.".to_string())?;

    let response = client()?
        .post(TOKEN_URL)
        .header("Accept", "application/json")
        .form(&[
            ("client_id", client_id.as_str()),
            ("device_code", device_code.as_str()),
            (
                "grant_type",
                "urn:ietf:params:oauth:grant-type:device_code",
            ),
        ])
        .send()
        .await
        .map_err(|e| format!("Cannot reach github.com: {e}"))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| format!("Cannot read GitHub's response: {e}"))?;

    if status.is_server_error() {
        return Err(format!("GitHub returned {status} during sign-in."));
    }

    match parse_token_response(&body) {
        TokenOutcome::Pending => Ok(DevicePoll::Pending),
        TokenOutcome::SlowDown => Ok(DevicePoll::SlowDown { interval: 10 }),
        TokenOutcome::Failed(message) => Ok(DevicePoll::Failed { message }),
        TokenOutcome::Token(token) => {
            let (user, scopes) = fetch_user(&token).await?;
            secrets::save(&token)?;
            remember(&db, &user)?;
            Ok(DevicePoll::Complete { user, scopes })
        }
    }
}

#[tauri::command]
pub async fn github_save_token(db: State<'_, Db>, token: String) -> Result<GithubUser, String> {
    let value = token.trim();
    if value.is_empty() {
        return Err("Paste a GitHub token first.".to_string());
    }
    let (user, _) = fetch_user(value).await?;
    secrets::save(value)?;
    remember(&db, &user)?;
    Ok(user)
}

#[tauri::command]
pub async fn github_sign_out(db: State<'_, Db>) -> Result<bool, String> {
    let removed = secrets::clear()?;
    db.delete_setting(PROFILE_KEY)?;
    Ok(removed)
}

#[tauri::command]
pub async fn github_list_repos(db: State<'_, Db>, page: u32) -> Result<RepoPage, String> {
    let _ = &db;
    let token = secrets::load()?
        .ok_or_else(|| "Sign in to GitHub to load repositories.".to_string())?;
    let page = page.clamp(1, 20);
    let url = format!(
        "{API}/user/repos?per_page={PER_PAGE}&page={page}&sort=updated&direction=desc&affiliation=owner,collaborator,organization_member"
    );

    let response = api_get(&token, &url).await?;
    let linked_next = response
        .headers()
        .get("link")
        .and_then(|value| value.to_str().ok())
        .map(|value| value.split(',').any(|part| part.contains("rel=\"next\"")))
        .unwrap_or(false);

    let repos: Vec<Repo> = response
        .json()
        .await
        .map_err(|e| format!("GitHub returned an unreadable repository list: {e}"))?;

    let has_more = linked_next || repos.len() as u32 == PER_PAGE;
    Ok(RepoPage {
        page,
        repos,
        has_more,
    })
}

/// Updates the cached profile timestamp so Settings can show when it was verified.
#[tauri::command]
pub fn github_touch_profile(db: State<'_, Db>) -> Result<(), String> {
    db.set_setting("github.profileCheckedAt", &now().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_device_flow_token_responses() {
        assert_eq!(
            parse_token_response(r#"{"access_token":"gho_abc","token_type":"bearer"}"#),
            TokenOutcome::Token("gho_abc".to_string())
        );
        assert_eq!(
            parse_token_response(r#"{"error":"authorization_pending"}"#),
            TokenOutcome::Pending
        );
        assert_eq!(
            parse_token_response(r#"{"error":"slow_down"}"#),
            TokenOutcome::SlowDown
        );
        assert!(matches!(
            parse_token_response(r#"{"error":"expired_token"}"#),
            TokenOutcome::Failed(_)
        ));
        assert!(matches!(
            parse_token_response("not json"),
            TokenOutcome::Failed(_)
        ));
    }
}
