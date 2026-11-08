//! GitHub integration: OAuth device flow, profile lookup and repository listing.
//! Every request is made from Rust; the token never enters the webview.

use crate::db::{Db, now};
use crate::secrets;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::State;

pub(crate) const API: &str = "https://api.github.com";
const DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const CLIENT_ID_KEY: &str = "github.clientId";
const PROFILE_KEY: &str = "github.profile";
const USER_AGENT: &str = "GRAFT/0.1.0 (Linux; Tauri)";
pub(crate) const API_VERSION: &str = "2022-11-28";
const SCOPE: &str = "repo read:user user";
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
    pub topics: Vec<String>,
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

pub(crate) fn client() -> Result<reqwest::Client, String> {
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

pub(crate) async fn describe_error(response: reqwest::Response) -> String {
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoSearchPage {
    pub page: u32,
    pub total_count: u32,
    pub repos: Vec<Repo>,
    pub has_more: bool,
}

fn search_query(
    raw: &str,
    language: Option<&str>,
    topic: Option<&str>,
    author: Option<&str>,
) -> Result<String, String> {
    let raw = raw.trim();
    if raw.chars().count() > 256 {
        return Err("Search terms must stay under 256 characters.".to_string());
    }

    let mut parts: Vec<String> = Vec::new();
    if !raw.is_empty() {
        parts.push(raw.to_string());
    }
    if let Some(language) = language.map(str::trim).filter(|value| !value.is_empty()) {
        let clean = language.replace('"', "").replace(['\n', '\r'], " ");
        if clean.chars().count() > 48 {
            return Err("That language filter is too long.".to_string());
        }
        parts.push(format!("language:\"{clean}\""));
    }
    if let Some(topic) = topic.map(str::trim).filter(|value| !value.is_empty()) {
        let clean = topic.trim_start_matches('#').to_lowercase();
        if clean.is_empty()
            || clean.chars().count() > 48
            || !clean
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        {
            return Err("Topics can use letters, numbers, dashes, underscores and dots.".to_string());
        }
        parts.push(format!("topic:{clean}"));
    }
    if let Some(author) = author.map(str::trim).filter(|value| !value.is_empty()) {
        let clean = author.trim_start_matches('@');
        if clean.is_empty()
            || clean.chars().count() > 39
            || !clean
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            return Err("Author filters must be a GitHub login.".to_string());
        }
        parts.push(format!("user:{clean}"));
    }

    if parts.is_empty() {
        return Err("Enter a search term, language, topic or author.".to_string());
    }
    Ok(parts.join(" "))
}

/// Searches public GitHub repositories. Language, topic and author become
/// GitHub search qualifiers, so the API does the filtering rather than a
/// client-side approximation.
#[tauri::command]
pub async fn github_search_repos(
    query: String,
    language: Option<String>,
    topic: Option<String>,
    author: Option<String>,
    sort: String,
    page: u32,
) -> Result<RepoSearchPage, String> {
    let token = token()?;
    let q = search_query(
        &query,
        language.as_deref(),
        topic.as_deref(),
        author.as_deref(),
    )?;
    let page = page.clamp(1, 10);
    let sort = match sort.as_str() {
        "stars" | "forks" | "updated" => Some(sort.clone()),
        _ => None,
    };

    let mut url = reqwest::Url::parse(&format!("{API}/search/repositories"))
        .map_err(|e| format!("Cannot build the GitHub search URL: {e}"))?;
    {
        let mut pairs = url.query_pairs_mut();
        pairs
            .append_pair("q", &q)
            .append_pair("per_page", "30")
            .append_pair("page", &page.to_string());
        if let Some(sort) = sort {
            pairs.append_pair("sort", &sort).append_pair("order", "desc");
        }
    }

    #[derive(Deserialize)]
    struct SearchResponse {
        #[serde(default)]
        total_count: u32,
        #[serde(default)]
        items: Vec<Repo>,
    }

    let value = get_json(&token, url.as_str()).await?;
    let result: SearchResponse = serde_json::from_value(value)
        .map_err(|e| format!("GitHub returned an unreadable search result: {e}"))?;
    Ok(RepoSearchPage {
        page,
        total_count: result.total_count,
        has_more: page < 10 && page * 30 < result.total_count,
        repos: result.items,
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubNotification {
    pub id: String,
    pub reason: String,
    pub unread: bool,
    pub updated_at: String,
    pub title: String,
    pub subject_type: String,
    pub number: Option<u64>,
    pub repo: Option<Repo>,
}

fn notification_from_value(value: &serde_json::Value) -> Option<GithubNotification> {
    let id = value.get("id")?.as_str()?.to_string();
    let subject = value.get("subject")?;
    let repo = value
        .get("repository")
        .and_then(|repo| serde_json::from_value::<Repo>(repo.clone()).ok())?;
    let subject_url = subject
        .get("url")
        .and_then(|url| url.as_str())
        .unwrap_or_default();
    let number = subject_url
        .rsplit('/')
        .next()
        .and_then(|value| value.parse::<u64>().ok());
    Some(GithubNotification {
        id,
        reason: value
            .get("reason")
            .and_then(|reason| reason.as_str())
            .unwrap_or("subscribed")
            .to_string(),
        unread: value.get("unread").and_then(|unread| unread.as_bool()).unwrap_or(true),
        updated_at: value
            .get("updated_at")
            .and_then(|updated| updated.as_str())
            .unwrap_or_default()
            .to_string(),
        title: subject
            .get("title")
            .and_then(|title| title.as_str())
            .unwrap_or("GitHub notification")
            .to_string(),
        subject_type: subject
            .get("type")
            .and_then(|kind| kind.as_str())
            .unwrap_or("Notification")
            .to_string(),
        number,
        repo: Some(repo),
    })
}

/// Reads the account's unread GitHub notifications. GRAFT only reads them;
/// opening one locally never marks it read on github.com.
#[tauri::command]
pub async fn github_notifications() -> Result<Vec<GithubNotification>, String> {
    let token = token()?;
    let value = get_json(
        &token,
        &format!("{API}/notifications?all=false&participating=false&per_page=30"),
    )
    .await?;
    let list = value
        .as_array()
        .ok_or_else(|| "GitHub returned an unexpected notification list.".to_string())?;
    Ok(list.iter().filter_map(notification_from_value).collect())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoOwnerInfo {
    pub login: String,
    #[serde(default)]
    pub avatar_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoLicense {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub spdx_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoPermissions {
    #[serde(default)]
    pub admin: bool,
    #[serde(default)]
    pub maintain: bool,
    #[serde(default)]
    pub push: bool,
    #[serde(default)]
    pub triage: bool,
    #[serde(default)]
    pub pull: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct RepoDetail {
    pub id: u64,
    pub name: String,
    pub full_name: String,
    #[serde(default)]
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
    pub watchers_count: u32,
    #[serde(default)]
    pub forks_count: u32,
    #[serde(default)]
    pub open_issues_count: u32,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub topics: Vec<String>,
    #[serde(default = "default_branch")]
    pub default_branch: String,
    pub html_url: String,
    #[serde(default)]
    pub clone_url: String,
    #[serde(default)]
    pub ssh_url: String,
    #[serde(default)]
    pub pushed_at: Option<String>,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub license: Option<RepoLicense>,
    #[serde(default)]
    pub permissions: Option<RepoPermissions>,
    #[serde(default)]
    pub owner: Option<RepoOwnerInfo>,
    #[serde(default, skip_deserializing)]
    pub starred: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct RepoEntry {
    pub name: String,
    pub path: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub sha: String,
    #[serde(default)]
    pub html_url: Option<String>,
    #[serde(default)]
    pub download_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoFile {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub sha: String,
    pub html_url: String,
    pub content: Option<String>,
    pub truncated: bool,
    pub binary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct RepoBranch {
    pub name: String,
    #[serde(default)]
    pub protected: bool,
    #[serde(skip_deserializing, default)]
    pub sha: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoCommit {
    pub sha: String,
    pub short: String,
    pub message: String,
    pub author_name: String,
    pub author_login: Option<String>,
    pub author_avatar: Option<String>,
    pub date: Option<String>,
    pub html_url: String,
    pub parents: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct RepoCommitFile {
    pub filename: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub additions: u64,
    #[serde(default)]
    pub deletions: u64,
    #[serde(default)]
    pub changes: u64,
    #[serde(default)]
    pub patch: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoCommitDetail {
    pub sha: String,
    pub message: String,
    pub author_name: String,
    pub author_login: Option<String>,
    pub author_avatar: Option<String>,
    pub date: Option<String>,
    pub additions: u64,
    pub deletions: u64,
    pub html_url: String,
    pub files: Vec<RepoCommitFile>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoPull {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub draft: bool,
    pub merged: bool,
    pub author_login: String,
    pub author_avatar: String,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
    pub comments: u32,
    pub additions: u64,
    pub deletions: u64,
    pub changed_files: u64,
    pub head_ref: String,
    pub base_ref: String,
    pub head_sha: String,
    pub html_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct RepoPullFile {
    pub filename: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub additions: u64,
    #[serde(default)]
    pub deletions: u64,
    #[serde(default)]
    pub patch: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoPullDetail {
    pub pull: RepoPull,
    pub files: Vec<RepoPullFile>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoPullReview {
    pub id: u64,
    pub state: String,
    pub body: String,
    pub author_login: String,
    pub author_avatar: String,
    pub submitted_at: Option<String>,
    pub html_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoPullComment {
    pub id: u64,
    pub body: String,
    pub path: String,
    pub line: Option<u64>,
    pub original_line: Option<u64>,
    pub side: Option<String>,
    pub author_login: String,
    pub author_avatar: String,
    pub created_at: String,
    pub html_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct ForkResult {
    pub full_name: String,
    pub html_url: String,
    #[serde(default)]
    pub clone_url: String,
}

pub(crate) fn token() -> Result<String, String> {
    secrets::load()?.ok_or_else(|| "Sign in to GitHub first.".to_string())
}

pub(crate) fn coords(owner: &str, repo: &str) -> Result<(String, String), String> {
    let valid = |value: &str| {
        (1..=120).contains(&value.len())
            && value
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    if valid(owner.trim()) && valid(repo.trim()) {
        Ok((owner.trim().to_string(), repo.trim().to_string()))
    } else {
        Err("Invalid repository reference.".to_string())
    }
}

fn encode_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn encode_path(path: &str) -> String {
    path.split('/')
        .map(encode_segment)
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

pub(crate) async fn get_json(token: &str, url: &str) -> Result<serde_json::Value, String> {
    let response = api_get(token, url).await?;
    response
        .json()
        .await
        .map_err(|e| format!("GitHub returned an unreadable response: {e}"))
}

async fn get_optional(token: &str, url: &str) -> Result<Option<serde_json::Value>, String> {
    let response = client()?
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", API_VERSION)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| format!("Cannot reach api.github.com: {e}"))?;

    if response.status().as_u16() == 404 {
        return Ok(None);
    }
    if !response.status().is_success() {
        return Err(describe_error(response).await);
    }
    Ok(Some(
        response
            .json()
            .await
            .map_err(|e| format!("GitHub returned an unreadable response: {e}"))?,
    ))
}

fn decode_base64(value: &str) -> Option<Vec<u8>> {
    let compact: String = value.chars().filter(|c| !c.is_whitespace()).collect();
    BASE64.decode(compact).ok()
}

fn file_from_value(value: &serde_json::Value) -> RepoFile {
    let name = value
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let path = value
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let size = value.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
    let sha = value
        .get("sha")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let html_url = value
        .get("html_url")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();

    let mut truncated = false;
    let mut binary = false;
    let mut content = None;

    if let Some(encoded) = value.get("content").and_then(|v| v.as_str()) {
        if size > 512_000 {
            truncated = true;
        } else if let Some(bytes) = decode_base64(encoded) {
            if bytes.iter().take(8000).any(|byte| *byte == 0) {
                binary = true;
            } else {
                match String::from_utf8(bytes) {
                    Ok(text) => content = Some(text),
                    Err(_) => binary = true,
                }
            }
        } else {
            binary = true;
        }
    }

    RepoFile {
        name,
        path,
        size,
        sha,
        html_url,
        content,
        truncated,
        binary,
    }
}

async fn starred(token: &str, owner: &str, repo: &str) -> Result<bool, String> {
    let response = client()?
        .get(format!("{API}/user/starred/{owner}/{repo}"))
        .header("X-GitHub-Api-Version", API_VERSION)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| format!("Cannot reach api.github.com: {e}"))?;
    Ok(response.status().is_success())
}

#[tauri::command]
pub async fn github_repo_detail(owner: String, repo: String) -> Result<RepoDetail, String> {
    let token = token()?;
    let (owner, repo) = coords(&owner, &repo)?;
    let mut detail: RepoDetail =
        serde_json::from_value(get_json(&token, &format!("{API}/repos/{owner}/{repo}")).await?)
            .map_err(|e| format!("GitHub returned an unreadable repository: {e}"))?;
    detail.starred = starred(&token, &owner, &repo).await.unwrap_or(false);
    Ok(detail)
}

#[tauri::command]
pub async fn github_repo_readme(owner: String, repo: String) -> Result<Option<RepoFile>, String> {
    let token = token()?;
    let (owner, repo) = coords(&owner, &repo)?;
    let value = get_optional(&token, &format!("{API}/repos/{owner}/{repo}/readme")).await?;
    Ok(value.as_ref().map(file_from_value))
}

#[tauri::command]
pub async fn github_repo_contents(
    owner: String,
    repo: String,
    path: Option<String>,
    git_ref: Option<String>,
) -> Result<Vec<RepoEntry>, String> {
    let token = token()?;
    let (owner, repo) = coords(&owner, &repo)?;
    let clean = path.unwrap_or_default();
    let clean = clean.trim_matches('/');
    let mut url = format!("{API}/repos/{owner}/{repo}/contents");
    if !clean.is_empty() {
        url.push('/');
        url.push_str(&encode_path(clean));
    }
    if let Some(git_ref) = git_ref.filter(|value| !value.trim().is_empty()) {
        url.push_str(&format!("?ref={}", encode_segment(git_ref.trim())));
    }

    let value = get_json(&token, &url).await?;
    if value.is_array() {
        let mut entries: Vec<RepoEntry> = serde_json::from_value(value)
            .map_err(|e| format!("GitHub returned an unreadable directory: {e}"))?;
        entries.sort_by(|a, b| {
            let kind = |entry: &RepoEntry| entry.kind != "dir";
            kind(a).cmp(&kind(b)).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok(entries)
    } else {
        let entry: RepoEntry = serde_json::from_value(value)
            .map_err(|e| format!("GitHub returned an unreadable entry: {e}"))?;
        Ok(vec![entry])
    }
}

#[tauri::command]
pub async fn github_repo_file(
    owner: String,
    repo: String,
    path: String,
    git_ref: Option<String>,
) -> Result<RepoFile, String> {
    let token = token()?;
    let (owner, repo) = coords(&owner, &repo)?;
    let clean = path.trim().trim_matches('/');
    if clean.is_empty() {
        return Err("Choose a file to read.".to_string());
    }
    let mut url = format!("{API}/repos/{owner}/{repo}/contents/{}", encode_path(clean));
    if let Some(git_ref) = git_ref.filter(|value| !value.trim().is_empty()) {
        url.push_str(&format!("?ref={}", encode_segment(git_ref.trim())));
    }
    let value = get_json(&token, &url).await?;
    Ok(file_from_value(&value))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoIssue {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub body: String,
    pub author_login: String,
    pub author_avatar: String,
    pub labels: Vec<String>,
    pub comments: u32,
    pub created_at: String,
    pub updated_at: String,
    pub html_url: String,
}

fn issue_from_value(value: &serde_json::Value) -> RepoIssue {
    RepoIssue {
        number: value.get("number").and_then(|v| v.as_u64()).unwrap_or(0),
        title: value
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        state: value
            .get("state")
            .and_then(|v| v.as_str())
            .unwrap_or("open")
            .to_string(),
        body: value
            .get("body")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        author_login: value
            .pointer("/user/login")
            .and_then(|v| v.as_str())
            .unwrap_or("ghost")
            .to_string(),
        author_avatar: value
            .pointer("/user/avatar_url")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        labels: value
            .get("labels")
            .and_then(|v| v.as_array())
            .map(|labels| {
                labels
                    .iter()
                    .filter_map(|label| label.get("name").and_then(|v| v.as_str()))
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
        comments: value.get("comments").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        created_at: value
            .get("created_at")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        updated_at: value
            .get("updated_at")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        html_url: value
            .get("html_url")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
    }
}

fn validate_issue(title: &str, body: &str) -> Result<String, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("An issue needs a title.".to_string());
    }
    if title.chars().count() > 256 {
        return Err("Issue titles must stay under 256 characters.".to_string());
    }
    if body.len() > 60_000 {
        return Err("Issue descriptions must stay under 60 KB.".to_string());
    }
    Ok(title.to_string())
}

#[tauri::command]
pub async fn github_repo_issues(
    owner: String,
    repo: String,
    state: String,
) -> Result<Vec<RepoIssue>, String> {
    let token = token()?;
    let (owner, repo) = coords(&owner, &repo)?;
    let state = match state.as_str() {
        "closed" | "all" | "open" => state,
        _ => "open".to_string(),
    };
    let value = get_json(
        &token,
        &format!(
            "{API}/repos/{owner}/{repo}/issues?state={state}&per_page=50&sort=updated&direction=desc"
        ),
    )
    .await?;
    let list = value
        .as_array()
        .ok_or_else(|| "GitHub returned an unexpected issue list.".to_string())?;
    Ok(list
        .iter()
        .filter(|issue| issue.get("pull_request").is_none())
        .map(issue_from_value)
        .collect())
}

#[tauri::command]
pub async fn github_create_issue(
    owner: String,
    repo: String,
    title: String,
    body: String,
) -> Result<RepoIssue, String> {
    let token = token()?;
    let (owner, repo) = coords(&owner, &repo)?;
    let title = validate_issue(&title, &body)?;
    let response = client()?
        .post(format!("{API}/repos/{owner}/{repo}/issues"))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", API_VERSION)
        .bearer_auth(&token)
        .json(&serde_json::json!({ "title": title, "body": body }))
        .send()
        .await
        .map_err(|e| format!("Cannot reach api.github.com: {e}"))?;
    if !response.status().is_success() {
        return Err(describe_error(response).await);
    }
    let value: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("GitHub returned an unreadable issue: {e}"))?;
    Ok(issue_from_value(&value))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileRepoResult {
    pub full_name: String,
    pub html_url: String,
}

/// Creates the special `<login>/<login>` profile repository. This is a live
/// write and the UI must confirm it before calling.
#[tauri::command]
pub async fn github_create_profile_repo(login: String) -> Result<ProfileRepoResult, String> {
    let token = token()?;
    let (login, _) = coords(&login, &login)?;
    let response = client()?
        .post(format!("{API}/user/repos"))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", API_VERSION)
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "name": login,
            "description": "My GitHub profile README",
            "private": false,
            "auto_init": true,
        }))
        .send()
        .await
        .map_err(|e| format!("Cannot reach api.github.com: {e}"))?;

    if response.status().as_u16() == 422 {
        return Err(format!(
            "GitHub refused to create {login}/{login}. A repository with that name may already exist, or the token lacks permission to create repositories."
        ));
    }
    if !response.status().is_success() {
        return Err(describe_error(response).await);
    }
    let value: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("GitHub returned an unreadable repository: {e}"))?;
    Ok(ProfileRepoResult {
        full_name: value
            .get("full_name")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        html_url: value
            .get("html_url")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileWriteResult {
    pub sha: String,
    pub commit_sha: String,
    pub html_url: Option<String>,
    pub path: String,
}

fn valid_file_path(path: &str) -> bool {
    let trimmed = path.trim();
    !trimmed.is_empty()
        && trimmed.len() <= 900
        && !trimmed.starts_with('/')
        && !trimmed.contains('\u{0}')
        && !trimmed
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == ".." || segment == ".git")
}

fn file_write_body(
    content: &str,
    sha: Option<&str>,
    branch: Option<&str>,
    message: &str,
) -> serde_json::Value {
    let mut body = serde_json::json!({
        "message": message,
        "content": BASE64.encode(content.as_bytes()),
    });
    if let Some(sha) = sha.filter(|value| !value.trim().is_empty()) {
        body["sha"] = serde_json::Value::String(sha.trim().to_string());
    }
    if let Some(branch) = branch.filter(|value| !value.trim().is_empty()) {
        body["branch"] = serde_json::Value::String(branch.trim().to_string());
    }
    body
}

fn file_commit_message(raw: &str, path: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        trim_to(&format!("Update {path}"), 240)
    } else {
        trim_to(trimmed, 240)
    }
}

/// Writes one text file to a repository branch. GitHub enforces push access;
/// the UI only offers editing when the repository reports push permission.
#[tauri::command]
pub async fn github_save_file(
    owner: String,
    repo: String,
    path: String,
    content: String,
    sha: Option<String>,
    branch: Option<String>,
    message: String,
) -> Result<FileWriteResult, String> {
    let token = token()?;
    let (owner, repo) = coords(&owner, &repo)?;
    let clean = path.trim().trim_start_matches('/');
    if !valid_file_path(clean) {
        return Err("That file path is not valid for a GitHub repository.".to_string());
    }
    if content.len() > 900_000 {
        return Err("Files saved from GRAFT must stay under 900 KB.".to_string());
    }
    let message = file_commit_message(&message, clean);
    let url = format!(
        "{API}/repos/{owner}/{repo}/contents/{}",
        encode_path(clean)
    );
    let response = client()?
        .put(&url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", API_VERSION)
        .bearer_auth(&token)
        .json(&file_write_body(
            &content,
            sha.as_deref(),
            branch.as_deref(),
            &message,
        ))
        .send()
        .await
        .map_err(|e| format!("Cannot reach api.github.com: {e}"))?;

    match response.status().as_u16() {
        409 => {
            return Err(
                "The file changed on GitHub since it was opened. Reload it and re-apply your edit."
                    .to_string(),
            )
        }
        404 => {
            return Err(
                "GitHub returned 404. The branch or file may have moved, or the token may lack write access."
                    .to_string(),
            )
        }
        _ => {}
    }
    if !response.status().is_success() {
        return Err(describe_error(response).await);
    }

    let value: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("GitHub returned an unreadable save response: {e}"))?;
    Ok(FileWriteResult {
        sha: value
            .pointer("/content/sha")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        commit_sha: value
            .pointer("/commit/sha")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        html_url: value
            .pointer("/content/html_url")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        path: value
            .pointer("/content/path")
            .and_then(|v| v.as_str())
            .unwrap_or(clean)
            .to_string(),
    })
}

#[tauri::command]
pub async fn github_repo_branches(owner: String, repo: String) -> Result<Vec<RepoBranch>, String> {
    let token = token()?;
    let (owner, repo) = coords(&owner, &repo)?;

    #[derive(Deserialize)]
    struct RawBranch {
        name: String,
        #[serde(default)]
        protected: bool,
        commit: RawCommit,
    }
    #[derive(Deserialize)]
    struct RawCommit {
        sha: String,
    }

    let value = get_json(
        &token,
        &format!("{API}/repos/{owner}/{repo}/branches?per_page=100"),
    )
    .await?;
    let raw: Vec<RawBranch> = serde_json::from_value(value)
        .map_err(|e| format!("GitHub returned an unreadable branch list: {e}"))?;
    Ok(raw
        .into_iter()
        .map(|branch| RepoBranch {
            name: branch.name,
            protected: branch.protected,
            sha: branch.commit.sha,
        })
        .collect())
}

#[tauri::command]
pub async fn github_repo_commits(
    owner: String,
    repo: String,
    git_ref: Option<String>,
    page: u32,
) -> Result<Vec<RepoCommit>, String> {
    let token = token()?;
    let (owner, repo) = coords(&owner, &repo)?;
    let page = page.clamp(1, 20);

    #[derive(Deserialize)]
    struct RawCommitEntry {
        sha: String,
        #[serde(default)]
        html_url: String,
        commit: RawCommitInfo,
        #[serde(default)]
        author: Option<RawUser>,
        #[serde(default)]
        parents: Vec<RawParent>,
    }
    #[derive(Deserialize)]
    struct RawParent {
        sha: String,
    }
    #[derive(Deserialize)]
    struct RawCommitInfo {
        message: String,
        #[serde(default)]
        author: Option<RawAuthor>,
    }
    #[derive(Deserialize)]
    struct RawAuthor {
        #[serde(default)]
        name: String,
        #[serde(default)]
        date: Option<String>,
    }
    #[derive(Deserialize)]
    struct RawUser {
        login: String,
        #[serde(default)]
        avatar_url: String,
    }

    let mut url = format!(
        "{API}/repos/{owner}/{repo}/commits?per_page=40&page={page}"
    );
    if let Some(git_ref) = git_ref.filter(|value| !value.trim().is_empty()) {
        url.push_str(&format!("&sha={}", encode_segment(git_ref.trim())));
    }

    let value = get_json(&token, &url).await?;
    let raw: Vec<RawCommitEntry> = serde_json::from_value(value)
        .map_err(|e| format!("GitHub returned an unreadable commit list: {e}"))?;
    Ok(raw
        .into_iter()
        .map(|entry| RepoCommit {
            short: entry.sha.chars().take(7).collect(),
            sha: entry.sha,
            message: entry.commit.message.lines().next().unwrap_or("").to_string(),
            author_name: entry
                .commit
                .author
                .as_ref()
                .map(|author| author.name.clone())
                .unwrap_or_else(|| "unknown".to_string()),
            date: entry.commit.author.and_then(|author| author.date),
            author_login: entry.author.as_ref().map(|user| user.login.clone()),
            author_avatar: entry.author.map(|user| user.avatar_url),
            html_url: entry.html_url,
            parents: entry.parents.into_iter().map(|parent| parent.sha).collect(),
        })
        .collect())
}

#[tauri::command]
pub async fn github_repo_commit(
    owner: String,
    repo: String,
    sha: String,
) -> Result<RepoCommitDetail, String> {
    let token = token()?;
    let (owner, repo) = coords(&owner, &repo)?;
    if sha.trim().len() < 7 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("Invalid commit reference.".to_string());
    }

    #[derive(Deserialize)]
    struct RawDetail {
        sha: String,
        #[serde(default)]
        html_url: String,
        commit: RawInfo,
        #[serde(default)]
        author: Option<RawUser>,
        #[serde(default)]
        stats: RawStats,
        #[serde(default)]
        files: Vec<RepoCommitFile>,
    }
    #[derive(Deserialize)]
    struct RawInfo {
        message: String,
        #[serde(default)]
        author: Option<RawAuthor>,
    }
    #[derive(Deserialize)]
    struct RawAuthor {
        #[serde(default)]
        name: String,
        #[serde(default)]
        date: Option<String>,
    }
    #[derive(Deserialize)]
    struct RawUser {
        login: String,
        #[serde(default)]
        avatar_url: String,
    }
    #[derive(Deserialize, Default)]
    struct RawStats {
        #[serde(default)]
        additions: u64,
        #[serde(default)]
        deletions: u64,
    }

    let value = get_json(
        &token,
        &format!("{API}/repos/{owner}/{repo}/commits/{sha}"),
    )
    .await?;
    let raw: RawDetail = serde_json::from_value(value)
        .map_err(|e| format!("GitHub returned an unreadable commit: {e}"))?;
    Ok(RepoCommitDetail {
        sha: raw.sha,
        message: raw.commit.message,
        author_name: raw
            .commit
            .author
            .as_ref()
            .map(|author| author.name.clone())
            .unwrap_or_else(|| "unknown".to_string()),
        date: raw.commit.author.and_then(|author| author.date),
        author_login: raw.author.as_ref().map(|user| user.login.clone()),
        author_avatar: raw.author.map(|user| user.avatar_url),
        additions: raw.stats.additions,
        deletions: raw.stats.deletions,
        html_url: raw.html_url,
        files: raw.files,
    })
}

fn pull_from_value(value: &serde_json::Value) -> RepoPull {
    RepoPull {
        number: value.get("number").and_then(|v| v.as_u64()).unwrap_or(0),
        title: value
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        state: value
            .get("state")
            .and_then(|v| v.as_str())
            .unwrap_or("open")
            .to_string(),
        draft: value.get("draft").and_then(|v| v.as_bool()).unwrap_or(false),
        merged: value.get("merged_at").map(|v| !v.is_null()).unwrap_or(false),
        author_login: value
            .pointer("/user/login")
            .and_then(|v| v.as_str())
            .unwrap_or("ghost")
            .to_string(),
        author_avatar: value
            .pointer("/user/avatar_url")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        body: value
            .get("body")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        created_at: value
            .get("created_at")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        updated_at: value
            .get("updated_at")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        comments: value.get("comments").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        additions: value.get("additions").and_then(|v| v.as_u64()).unwrap_or(0),
        deletions: value.get("deletions").and_then(|v| v.as_u64()).unwrap_or(0),
        changed_files: value
            .get("changed_files")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        head_ref: value
            .pointer("/head/ref")
            .and_then(|v| v.as_str())
            .unwrap_or("?")
            .to_string(),
        base_ref: value
            .pointer("/base/ref")
            .and_then(|v| v.as_str())
            .unwrap_or("?")
            .to_string(),
        head_sha: value
            .pointer("/head/sha")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        html_url: value
            .get("html_url")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
    }
}

#[tauri::command]
pub async fn github_repo_pulls(
    owner: String,
    repo: String,
    state: String,
) -> Result<Vec<RepoPull>, String> {
    let token = token()?;
    let (owner, repo) = coords(&owner, &repo)?;
    let state = match state.as_str() {
        "closed" | "all" | "open" => state,
        _ => "open".to_string(),
    };
    let value = get_json(
        &token,
        &format!(
            "{API}/repos/{owner}/{repo}/pulls?state={state}&per_page=50&sort=updated&direction=desc"
        ),
    )
    .await?;
    let list = value
        .as_array()
        .ok_or_else(|| "GitHub returned an unexpected pull request list.".to_string())?;
    Ok(list.iter().map(pull_from_value).collect())
}

#[tauri::command]
pub async fn github_repo_pull(
    owner: String,
    repo: String,
    number: u64,
) -> Result<RepoPullDetail, String> {
    let token = token()?;
    let (owner, repo) = coords(&owner, &repo)?;
    let pull_value = get_json(
        &token,
        &format!("{API}/repos/{owner}/{repo}/pulls/{number}"),
    )
    .await?;
    let files_value = get_json(
        &token,
        &format!("{API}/repos/{owner}/{repo}/pulls/{number}/files?per_page=100"),
    )
    .await?;
    let files: Vec<RepoPullFile> = serde_json::from_value(files_value)
        .map_err(|e| format!("GitHub returned unreadable pull request files: {e}"))?;
    Ok(RepoPullDetail {
        pull: pull_from_value(&pull_value),
        files,
    })
}

fn review_from_value(value: &serde_json::Value) -> RepoPullReview {
    RepoPullReview {
        id: value.get("id").and_then(|v| v.as_u64()).unwrap_or(0),
        state: value
            .get("state")
            .and_then(|v| v.as_str())
            .unwrap_or("COMMENTED")
            .to_string(),
        body: value
            .get("body")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        author_login: value
            .pointer("/user/login")
            .and_then(|v| v.as_str())
            .unwrap_or("ghost")
            .to_string(),
        author_avatar: value
            .pointer("/user/avatar_url")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        submitted_at: value
            .get("submitted_at")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        html_url: value
            .get("html_url")
            .and_then(|v| v.as_str())
            .map(str::to_string),
    }
}

fn pull_comment_from_value(value: &serde_json::Value) -> RepoPullComment {
    RepoPullComment {
        id: value.get("id").and_then(|v| v.as_u64()).unwrap_or(0),
        body: value
            .get("body")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        path: value
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        line: value.get("line").and_then(|v| v.as_u64()),
        original_line: value.get("original_line").and_then(|v| v.as_u64()),
        side: value
            .get("side")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        author_login: value
            .pointer("/user/login")
            .and_then(|v| v.as_str())
            .unwrap_or("ghost")
            .to_string(),
        author_avatar: value
            .pointer("/user/avatar_url")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        created_at: value
            .get("created_at")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        html_url: value
            .get("html_url")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
    }
}

fn review_event(raw: &str) -> Result<&'static str, String> {
    match raw.trim().to_ascii_uppercase().as_str() {
        "APPROVE" => Ok("APPROVE"),
        "REQUEST_CHANGES" => Ok("REQUEST_CHANGES"),
        "COMMENT" => Ok("COMMENT"),
        _ => Err("Choose approve, request changes or comment.".to_string()),
    }
}

#[tauri::command]
pub async fn github_repo_pull_reviews(
    owner: String,
    repo: String,
    number: u64,
) -> Result<Vec<RepoPullReview>, String> {
    let token = token()?;
    let (owner, repo) = coords(&owner, &repo)?;
    let value = get_json(
        &token,
        &format!("{API}/repos/{owner}/{repo}/pulls/{number}/reviews?per_page=50"),
    )
    .await?;
    value
        .as_array()
        .ok_or_else(|| "GitHub returned an unexpected review list.".to_string())
        .map(|items| items.iter().map(review_from_value).collect())
}

#[tauri::command]
pub async fn github_repo_pull_comments(
    owner: String,
    repo: String,
    number: u64,
) -> Result<Vec<RepoPullComment>, String> {
    let token = token()?;
    let (owner, repo) = coords(&owner, &repo)?;
    let value = get_json(
        &token,
        &format!("{API}/repos/{owner}/{repo}/pulls/{number}/comments?per_page=100"),
    )
    .await?;
    value
        .as_array()
        .ok_or_else(|| "GitHub returned an unexpected review comment list.".to_string())
        .map(|items| items.iter().map(pull_comment_from_value).collect())
}

/// Submits a real pull request review. The UI confirms before calling.
#[tauri::command]
pub async fn github_submit_pull_review(
    owner: String,
    repo: String,
    number: u64,
    event: String,
    body: String,
) -> Result<RepoPullReview, String> {
    let token = token()?;
    let (owner, repo) = coords(&owner, &repo)?;
    let event = review_event(&event)?;
    if body.len() > 65_536 {
        return Err("Review text must stay under 64 KB.".to_string());
    }
    if event != "APPROVE" && body.trim().is_empty() {
        return Err("Add a review comment before requesting changes or commenting.".to_string());
    }
    let mut payload = serde_json::json!({ "event": event });
    if !body.trim().is_empty() {
        payload["body"] = serde_json::Value::String(body.trim().to_string());
    }
    let response = client()?
        .post(format!("{API}/repos/{owner}/{repo}/pulls/{number}/reviews"))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", API_VERSION)
        .bearer_auth(&token)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Cannot reach api.github.com: {e}"))?;
    if !response.status().is_success() {
        return Err(describe_error(response).await);
    }
    let value: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("GitHub returned an unreadable review: {e}"))?;
    Ok(review_from_value(&value))
}

/// Creates an inline review comment on a line in the pull request diff.
#[tauri::command]
pub async fn github_create_pull_comment(
    owner: String,
    repo: String,
    number: u64,
    body: String,
    commit_id: String,
    path: String,
    line: u64,
    side: String,
) -> Result<RepoPullComment, String> {
    let token = token()?;
    let (owner, repo) = coords(&owner, &repo)?;
    if body.trim().is_empty() {
        return Err("Write a comment first.".to_string());
    }
    if body.len() > 65_536 {
        return Err("Comments must stay under 64 KB.".to_string());
    }
    let clean = path.trim().trim_start_matches('/');
    if !valid_file_path(clean) {
        return Err("That file path is not valid for a pull request comment.".to_string());
    }
    let commit_id = commit_id.trim();
    if commit_id.len() < 7 || !commit_id.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("The pull request head commit is missing.".to_string());
    }
    let side = match side.trim().to_ascii_uppercase().as_str() {
        "LEFT" => "LEFT",
        "RIGHT" => "RIGHT",
        _ => return Err("Comment side must be LEFT or RIGHT.".to_string()),
    };
    if !(1..=1_000_000).contains(&line) {
        return Err("Choose a valid diff line.".to_string());
    }
    let payload = serde_json::json!({
        "body": body.trim(),
        "commit_id": commit_id,
        "path": clean,
        "line": line,
        "side": side,
    });
    let response = client()?
        .post(format!("{API}/repos/{owner}/{repo}/pulls/{number}/comments"))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", API_VERSION)
        .bearer_auth(&token)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Cannot reach api.github.com: {e}"))?;
    if !response.status().is_success() {
        return Err(describe_error(response).await);
    }
    let value: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("GitHub returned an unreadable comment: {e}"))?;
    Ok(pull_comment_from_value(&value))
}

#[tauri::command]
pub async fn github_fork(owner: String, repo: String) -> Result<ForkResult, String> {
    let token = token()?;
    let (owner, repo) = coords(&owner, &repo)?;
    let response = client()?
        .post(format!("{API}/repos/{owner}/{repo}/forks"))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", API_VERSION)
        .header("Content-Length", "0")
        .bearer_auth(&token)
        .send()
        .await
        .map_err(|e| format!("Cannot reach api.github.com: {e}"))?;

    if !response.status().is_success() {
        return Err(describe_error(response).await);
    }
    response
        .json()
        .await
        .map_err(|e| format!("GitHub returned an unreadable fork response: {e}"))
}

#[tauri::command]
pub async fn github_set_star(
    owner: String,
    repo: String,
    starred: bool,
) -> Result<bool, String> {
    let token = token()?;
    let (owner, repo) = coords(&owner, &repo)?;
    let url = format!("{API}/user/starred/{owner}/{repo}");
    let builder = if starred {
        client()?.put(url)
    } else {
        client()?.delete(url)
    };
    let response = builder
        .header("X-GitHub-Api-Version", API_VERSION)
        .header("Content-Length", "0")
        .bearer_auth(&token)
        .send()
        .await
        .map_err(|e| format!("Cannot reach api.github.com: {e}"))?;

    if response.status().is_success() {
        Ok(starred)
    } else {
        Err(describe_error(response).await)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileReadme {
    pub exists: bool,
    pub content: Option<String>,
    pub sha: Option<String>,
    pub html_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishResult {
    pub sha: String,
    pub commit_sha: String,
    pub html_url: Option<String>,
}

/// Reads the README of the special `<login>/<login>` profile repository.
#[tauri::command]
pub async fn github_profile_readme(login: String) -> Result<ProfileReadme, String> {
    let token = token()?;
    let (login, _) = coords(&login, &login)?;
    let value = get_optional(
        &token,
        &format!("{API}/repos/{login}/{login}/contents/README.md"),
    )
    .await?;

    Ok(match value {
        Some(value) => {
            let file = file_from_value(&value);
            ProfileReadme {
                exists: true,
                content: file.content,
                sha: Some(file.sha),
                html_url: Some(file.html_url),
            }
        }
        None => ProfileReadme {
            exists: false,
            content: None,
            sha: None,
            html_url: None,
        },
    })
}

/// The contents API body for a profile README write. Kept separate so the
/// encoding and SHA handling can be tested without a network call.
fn publish_body(markdown: &str, sha: Option<&str>, message: &str) -> serde_json::Value {
    let mut body = serde_json::json!({
        "message": message,
        "content": BASE64.encode(markdown.as_bytes()),
    });
    if let Some(sha) = sha.filter(|value| !value.trim().is_empty()) {
        body["sha"] = serde_json::Value::String(sha.trim().to_string());
    }
    body
}

fn publish_message(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        "Update profile README".to_string()
    } else {
        trim_to(trimmed, 240)
    }
}

fn trim_to(value: &str, limit: usize) -> String {
    value.chars().take(limit).collect()
}

/// Writes the draft to the live profile README. This is the only function that
/// publishes profile content, and the UI confirms before calling it.
#[tauri::command]
pub async fn github_publish_profile(
    login: String,
    markdown: String,
    sha: Option<String>,
    message: String,
) -> Result<PublishResult, String> {
    let token = token()?;
    let (login, _) = coords(&login, &login)?;
    if markdown.trim().is_empty() {
        return Err("The draft is empty — nothing to publish.".to_string());
    }
    if markdown.len() > 900_000 {
        return Err("The profile README must stay under 900 KB.".to_string());
    }

    let url = format!("{API}/repos/{login}/{login}/contents/README.md");
    let response = client()?
        .put(&url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", API_VERSION)
        .bearer_auth(&token)
        .json(&publish_body(&markdown, sha.as_deref(), &publish_message(&message)))
        .send()
        .await
        .map_err(|e| format!("Cannot reach api.github.com: {e}"))?;

    if response.status().as_u16() == 404 {
        return Err(format!(
            "The profile repository {login}/{login} does not exist yet. Create a public repository named “{login}” on GitHub, then publish again."
        ));
    }
    if !response.status().is_success() {
        return Err(describe_error(response).await);
    }

    let value: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("GitHub returned an unreadable publish response: {e}"))?;
    Ok(PublishResult {
        sha: value
            .pointer("/content/sha")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        commit_sha: value
            .pointer("/commit/sha")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        html_url: value
            .pointer("/content/html_url")
            .and_then(|v| v.as_str())
            .map(str::to_string),
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
    fn builds_http_client() {
        let client = client();
        assert!(client.is_ok(), "client build failed: {:?}", client.err());
    }

    #[test]
    fn parses_issues_and_excludes_pull_request_entries() {
        let value = serde_json::json!({
            "number": 12,
            "title": "Fix the thing",
            "state": "open",
            "body": "Details",
            "user": { "login": "bushninja", "avatar_url": "https://example.com/a.png" },
            "labels": [{ "name": "bug" }, { "name": "linux" }],
            "comments": 3,
            "created_at": "2026-11-01T00:00:00Z",
            "updated_at": "2026-11-02T00:00:00Z",
            "html_url": "https://github.com/o/r/issues/12"
        });
        let issue = issue_from_value(&value);
        assert_eq!(issue.number, 12);
        assert_eq!(issue.labels, vec!["bug", "linux"]);
        assert_eq!(issue.author_login, "bushninja");
        assert!(validate_issue("  title  ", "body").is_ok());
        assert!(validate_issue("   ", "body").is_err());
        assert!(validate_issue(&"x".repeat(257), "body").is_err());
    }

    #[test]
    fn validates_file_paths_and_builds_write_payload() {
        assert!(valid_file_path("src/main.rs"));
        assert!(valid_file_path("README.md"));
        assert!(!valid_file_path(""));
        assert!(!valid_file_path("/etc/passwd"));
        assert!(!valid_file_path("src/../secret"));
        assert!(!valid_file_path("src//main.rs"));
        assert!(!valid_file_path(".git/config"));

        let body = file_write_body("hello\n", Some("abc123"), Some("main"), "Update notes");
        assert_eq!(body["message"], "Update notes");
        assert_eq!(body["sha"], "abc123");
        assert_eq!(body["branch"], "main");
        let decoded = decode_base64(body["content"].as_str().expect("content"))
            .expect("base64");
        assert_eq!(String::from_utf8(decoded).expect("utf8"), "hello\n");
        assert_eq!(file_commit_message("  ", "README.md"), "Update README.md");
    }

    #[test]
    fn builds_profile_publish_payload() {
        let body = publish_body("# Hi\n", None, "Update profile README");
        assert_eq!(body["message"], "Update profile README");
        assert!(body.get("sha").is_none());
        let decoded = decode_base64(body["content"].as_str().expect("content"))
            .expect("base64");
        assert_eq!(String::from_utf8(decoded).expect("utf8"), "# Hi\n");

        let update = publish_body("# Hi\n", Some("abc123"), &publish_message("  refresh\n"));
        assert_eq!(update["sha"], "abc123");
        assert_eq!(update["message"], "refresh");
        assert_eq!(publish_message("   "), "Update profile README");
    }

    #[test]
    fn builds_search_queries() {
        let query = search_query("ricing", Some("Rust"), Some("#hyprland"), Some("@bushninjadots"))
            .expect("query");
        assert_eq!(
            query,
            "ricing language:\"Rust\" topic:hyprland user:bushninjadots"
        );
        assert!(search_query("", None, None, None).is_err());
        assert!(search_query("x", None, Some("bad topic"), None).is_err());
        assert!(search_query("x", None, None, Some("not/a/login")).is_err());
    }

    #[test]
    fn maps_github_notifications_to_repository_targets() {
        let value = serde_json::json!({
            "id": "123",
            "reason": "mention",
            "unread": true,
            "updated_at": "2026-11-08T12:00:00Z",
            "subject": {
                "title": "Hyprland crash on reload",
                "type": "Issue",
                "url": "https://api.github.com/repos/o/r/issues/42"
            },
            "repository": {
                "id": 9,
                "name": "r",
                "full_name": "o/r",
                "private": false,
                "description": "rice",
                "language": "QML",
                "stargazers_count": 10,
                "forks_count": 2,
                "updated_at": "2026-11-08T12:00:00Z",
                "default_branch": "main",
                "html_url": "https://github.com/o/r",
                "owner": { "login": "o" }
            }
        });
        let notification = notification_from_value(&value).expect("notification");
        assert_eq!(notification.number, Some(42));
        assert_eq!(notification.repo.as_ref().map(|repo| repo.full_name.as_str()), Some("o/r"));
        assert_eq!(notification.reason, "mention");
    }

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
