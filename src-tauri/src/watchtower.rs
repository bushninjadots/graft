//! Watchtower: locally favourited repositories and conditional GitHub event
//! polling. Nothing here writes to GitHub; alerts are imported and stored in
//! SQLite so history survives restarts.

use crate::db::{now, Db};
use crate::github::{self, Repo};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tauri::State;

const MAX_WATCHED: i64 = 30;
const EVENT_LIMIT: i64 = 160;
pub const CATEGORIES: [&str; 8] = [
    "commits",
    "pull_requests",
    "issues",
    "releases",
    "reviews",
    "workflows",
    "stars",
    "forks",
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchedRepo {
    pub id: i64,
    pub owner: String,
    pub name: String,
    pub full_name: String,
    pub html_url: String,
    pub description: Option<String>,
    pub language: Option<String>,
    pub private: bool,
    pub last_polled_at: Option<i64>,
    pub last_error: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchEvent {
    pub id: i64,
    pub repo_id: i64,
    pub repo_full_name: String,
    pub category: String,
    pub kind: String,
    pub action: String,
    pub title: String,
    pub detail: String,
    pub actor: String,
    pub url: String,
    pub occurred_at: String,
    pub seen: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchtowerSnapshot {
    pub repos: Vec<WatchedRepo>,
    pub events: Vec<WatchEvent>,
    pub unread: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchtowerPoll {
    pub checked: u32,
    pub not_modified: u32,
    pub new_events: Vec<WatchEvent>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VisitedRepo {
    pub full_name: String,
    pub owner: String,
    pub name: String,
    pub html_url: String,
    pub description: Option<String>,
    pub language: Option<String>,
    pub topics: Vec<String>,
    pub stars: u32,
    pub forks: u32,
    pub default_branch: String,
    pub private: bool,
    pub visit_count: u32,
    pub last_visited_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recommendation {
    pub repo: Repo,
    pub reason: String,
    pub score: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchtowerDiscovery {
    pub starred: Vec<Repo>,
    pub visited: Vec<VisitedRepo>,
    pub recommended: Vec<Recommendation>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VisitInput {
    pub owner: String,
    pub name: String,
    pub html_url: String,
    pub description: Option<String>,
    pub language: Option<String>,
    #[serde(default)]
    pub topics: Vec<String>,
    #[serde(default)]
    pub stars: u32,
    #[serde(default)]
    pub forks: u32,
    #[serde(default = "default_branch")]
    pub default_branch: String,
    #[serde(default)]
    pub private: bool,
}

fn default_branch() -> String {
    "main".to_string()
}

#[derive(Debug, Clone)]
struct WatchRow {
    repo: WatchedRepo,
    etag: Option<String>,
    actions_etag: Option<String>,
    initialized: bool,
}

#[derive(Debug, Clone)]
struct EventInput {
    key: String,
    category: &'static str,
    kind: String,
    action: String,
    title: String,
    detail: String,
    actor: String,
    url: String,
    occurred_at: String,
}

fn lock(db: &Db) -> std::sync::MutexGuard<'_, Connection> {
    db.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn watched_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<WatchedRepo> {
    Ok(WatchedRepo {
        id: row.get(0)?,
        owner: row.get(1)?,
        name: row.get(2)?,
        full_name: row.get(3)?,
        html_url: row.get(4)?,
        description: row.get(5)?,
        language: row.get(6)?,
        private: row.get::<_, i64>(7)? != 0,
        last_polled_at: row.get(8)?,
        last_error: row.get(9)?,
        created_at: row.get(10)?,
    })
}

fn event_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<WatchEvent> {
    Ok(WatchEvent {
        id: row.get(0)?,
        repo_id: row.get(1)?,
        repo_full_name: row.get(2)?,
        category: row.get(3)?,
        kind: row.get(4)?,
        action: row.get(5)?,
        title: row.get(6)?,
        detail: row.get(7)?,
        actor: row.get(8)?,
        url: row.get(9)?,
        occurred_at: row.get(10)?,
        seen: row.get::<_, i64>(11)? != 0,
        created_at: row.get(12)?,
    })
}

fn list_repos(db: &Db) -> Result<Vec<WatchedRepo>, String> {
    let conn = lock(db);
    let mut stmt = conn
        .prepare(
            "SELECT id, owner, name, full_name, html_url, description, language,
                    private, last_polled_at, last_error, created_at
             FROM watched_repos ORDER BY full_name COLLATE NOCASE",
        )
        .map_err(|e| format!("cannot read watched repositories: {e}"))?;
    let rows = stmt
        .query_map([], watched_from_row)
        .map_err(|e| format!("cannot query watched repositories: {e}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("cannot decode watched repositories: {e}"))
}

fn visited_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<VisitedRepo> {
    let topics: String = row.get(6)?;
    Ok(VisitedRepo {
        full_name: row.get(0)?,
        owner: row.get(1)?,
        name: row.get(2)?,
        html_url: row.get(3)?,
        description: row.get(4)?,
        language: row.get(5)?,
        topics: serde_json::from_str(&topics).unwrap_or_default(),
        stars: row.get::<_, i64>(7)?.clamp(0, u32::MAX as i64) as u32,
        forks: row.get::<_, i64>(8)?.clamp(0, u32::MAX as i64) as u32,
        default_branch: row.get(9)?,
        private: row.get::<_, i64>(10)? != 0,
        visit_count: row.get::<_, i64>(11)?.clamp(0, u32::MAX as i64) as u32,
        last_visited_at: row.get(12)?,
    })
}

fn list_visited(db: &Db, limit: i64) -> Result<Vec<VisitedRepo>, String> {
    let conn = lock(db);
    let mut stmt = conn
        .prepare(
            "SELECT full_name, owner, name, html_url, description, language, topics,
                    stars, forks, default_branch, private, visit_count, last_visited_at
             FROM repo_visits
             ORDER BY visit_count DESC, last_visited_at DESC
             LIMIT ?1",
        )
        .map_err(|e| format!("cannot read visit history: {e}"))?;
    let rows = stmt
        .query_map(params![limit], visited_from_row)
        .map_err(|e| format!("cannot query visit history: {e}"))?;
    let visited = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("cannot decode visit history: {e}"))?;
    Ok(visited)
}

fn store_visit(db: &Db, input: VisitInput) -> Result<(), String> {
    let (owner, name) = github::coords(&input.owner, &input.name)?;
    let full_name = format!("{owner}/{name}");
    let html_url = if input.html_url.starts_with("https://github.com/") {
        input.html_url
    } else {
        format!("https://github.com/{full_name}")
    };
    let topics: Vec<String> = input
        .topics
        .into_iter()
        .filter(|topic| {
            let clean = topic.trim();
            !clean.is_empty()
                && clean.chars().count() <= 80
                && clean
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        })
        .take(20)
        .collect();
    let topics = serde_json::to_string(&topics)
        .map_err(|e| format!("cannot encode repository topics: {e}"))?;
    let branch = if input.default_branch.trim().is_empty() {
        "main".to_string()
    } else {
        input.default_branch.trim().to_string()
    };
    let at = now();
    let conn = lock(db);
    conn.execute(
        "INSERT INTO repo_visits
         (full_name, owner, name, html_url, description, language, topics, stars, forks,
          default_branch, private, visit_count, last_visited_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 1, ?12)
         ON CONFLICT(full_name) DO UPDATE SET
           owner = excluded.owner,
           name = excluded.name,
           html_url = excluded.html_url,
           description = excluded.description,
           language = excluded.language,
           topics = excluded.topics,
           stars = excluded.stars,
           forks = excluded.forks,
           default_branch = excluded.default_branch,
           private = excluded.private,
           visit_count = repo_visits.visit_count + 1,
           last_visited_at = excluded.last_visited_at",
        params![
            full_name,
            owner,
            name,
            html_url,
            input.description,
            input.language,
            topics,
            input.stars,
            input.forks,
            branch,
            if input.private { 1 } else { 0 },
            at
        ],
    )
    .map_err(|e| format!("cannot record repository visit: {e}"))?;
    Ok(())
}

#[tauri::command]
pub fn watchtower_record_visit(
    db: State<'_, Db>,
    repo: VisitInput,
) -> Result<(), String> {
    store_visit(&db, repo)
}

fn add_signal(map: &mut HashMap<String, u32>, value: &str, weight: u32) {
    let clean = value.trim();
    if clean.is_empty() {
        return;
    }
    *map.entry(clean.to_string()).or_insert(0) += weight.max(1);
}

fn top_signals(map: &HashMap<String, u32>, limit: usize) -> Vec<String> {
    let mut values: Vec<(String, u32)> = map
        .iter()
        .map(|(value, weight)| (value.clone(), *weight))
        .collect();
    values.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    values.truncate(limit);
    values.into_iter().map(|(value, _)| value).collect()
}

fn recommendation_queries(
    starred: &[Repo],
    watched: &[WatchedRepo],
    visited: &[VisitedRepo],
) -> Vec<(String, String)> {
    let mut topics: HashMap<String, u32> = HashMap::new();
    let mut languages: HashMap<String, u32> = HashMap::new();

    for repo in starred {
        if let Some(language) = &repo.language {
            add_signal(&mut languages, language, 1);
        }
        for topic in &repo.topics {
            add_signal(&mut topics, topic, 1);
        }
    }
    for repo in watched {
        if let Some(language) = &repo.language {
            add_signal(&mut languages, language, 3);
        }
    }
    for repo in visited {
        let weight = repo.visit_count.clamp(1, 6);
        if let Some(language) = &repo.language {
            add_signal(&mut languages, language, weight);
        }
        for topic in &repo.topics {
            add_signal(&mut topics, topic, weight);
        }
    }

    let mut queries = Vec::new();
    for topic in top_signals(&topics, 2) {
        let clean = topic.to_lowercase();
        if clean
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        {
            queries.push((
                format!("topic:{clean} stars:>25 archived:false"),
                format!("Similar to your #{clean} interests"),
            ));
        }
    }
    for language in top_signals(&languages, 2) {
        let clean = language.replace('"', "");
        queries.push((
            format!("language:\"{clean}\" stars:>50 archived:false"),
            format!("Popular in {clean}, which you follow"),
        ));
    }
    queries
}

async fn fetch_starred(token: &str) -> Result<Vec<Repo>, String> {
    let value = github::get_json(
        token,
        &format!("{}/user/starred?per_page=100&sort=updated", github::API),
    )
    .await?;
    serde_json::from_value(value)
        .map_err(|e| format!("GitHub returned an unreadable starred list: {e}"))
}

async fn recommendations(
    token: &str,
    starred: &[Repo],
    watched: &[WatchedRepo],
    visited: &[VisitedRepo],
    errors: &mut Vec<String>,
) -> Vec<Recommendation> {
    let queries = recommendation_queries(starred, watched, visited);
    if queries.is_empty() {
        return Vec::new();
    }

    let mut excluded: HashSet<String> = HashSet::new();
    for repo in starred {
        excluded.insert(repo.full_name.to_lowercase());
    }
    for repo in watched {
        excluded.insert(repo.full_name.to_lowercase());
    }
    for repo in visited {
        excluded.insert(repo.full_name.to_lowercase());
    }

    #[derive(Deserialize)]
    struct SearchResponse {
        #[serde(default)]
        items: Vec<Repo>,
    }

    let mut seen: HashSet<String> = HashSet::new();
    let mut found: Vec<Recommendation> = Vec::new();
    for (index, (query, reason)) in queries.iter().enumerate() {
        let url = match reqwest::Url::parse(&format!("{}/search/repositories", github::API)) {
            Ok(url) => url,
            Err(error) => {
                errors.push(format!("Recommendations: {error}"));
                continue;
            }
        };
        let mut url = url;
        url.query_pairs_mut()
            .append_pair("q", query)
            .append_pair("sort", "stars")
            .append_pair("order", "desc")
            .append_pair("per_page", "8");
        let value = match github::get_json(token, url.as_str()).await {
            Ok(value) => value,
            Err(error) => {
                errors.push(format!("Recommendations: {error}"));
                continue;
            }
        };
        let result: SearchResponse = match serde_json::from_value(value) {
            Ok(result) => result,
            Err(error) => {
                errors.push(format!("Recommendations: unreadable search result: {error}"));
                continue;
            }
        };
        for (rank, repo) in result.items.into_iter().enumerate() {
            if repo.archived || repo.fork {
                continue;
            }
            let key = repo.full_name.to_lowercase();
            if excluded.contains(&key) || !seen.insert(key) {
                continue;
            }
            found.push(Recommendation {
                repo,
                reason: reason.clone(),
                score: ((queries.len() - index) as u32) * 100 + (8 - rank.min(8) as u32),
            });
        }
    }
    found.sort_by(|a, b| b.score.cmp(&a.score));
    found.truncate(12);
    found
}

/// Returns real GitHub stars, local frequently visited repositories and
/// similar repositories derived from the languages and topics in those three
/// sources. One failed source does not hide the others.
#[tauri::command]
pub async fn watchtower_discover(db: State<'_, Db>) -> Result<WatchtowerDiscovery, String> {
    let token = github::token()?;
    let watched = list_repos(&db)?;
    let visited = list_visited(&db, 12)?;
    let mut errors = Vec::new();
    let starred = match fetch_starred(&token).await {
        Ok(starred) => starred,
        Err(error) => {
            errors.push(format!("Starred repositories: {error}"));
            Vec::new()
        }
    };
    let recommended = recommendations(&token, &starred, &watched, &visited, &mut errors).await;
    Ok(WatchtowerDiscovery {
        starred,
        visited,
        recommended,
        errors,
    })
}

fn unread_count(db: &Db) -> Result<i64, String> {
    let conn = lock(db);
    conn.query_row("SELECT COUNT(*) FROM watch_events WHERE seen = 0", [], |row| {
        row.get(0)
    })
    .map_err(|e| format!("cannot count unread alerts: {e}"))
}

fn snapshot(db: &Db) -> Result<WatchtowerSnapshot, String> {
    let repos = list_repos(db)?;
    let unread = unread_count(db)?;
    let conn = lock(db);
    let mut stmt = conn
        .prepare(
            "SELECT e.id, e.repo_id, r.full_name, e.category, e.kind, e.action,
                    e.title, e.detail, e.actor, e.url, e.occurred_at, e.seen, e.created_at
             FROM watch_events e
             JOIN watched_repos r ON r.id = e.repo_id
             ORDER BY e.occurred_at DESC, e.id DESC LIMIT ?1",
        )
        .map_err(|e| format!("cannot read alert history: {e}"))?;
    let events = stmt
        .query_map(params![EVENT_LIMIT], event_from_row)
        .map_err(|e| format!("cannot query alert history: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("cannot decode alert history: {e}"))?;
    Ok(WatchtowerSnapshot {
        repos,
        events,
        unread,
    })
}

fn watch_rows(db: &Db) -> Result<Vec<WatchRow>, String> {
    let conn = lock(db);
    let mut stmt = conn
        .prepare(
            "SELECT id, owner, name, full_name, html_url, description, language,
                    private, last_polled_at, last_error, created_at,
                    etag, actions_etag, initialized
             FROM watched_repos ORDER BY full_name COLLATE NOCASE",
        )
        .map_err(|e| format!("cannot read watch state: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(WatchRow {
                repo: watched_from_row(row)?,
                etag: row.get(11)?,
                actions_etag: row.get(12)?,
                initialized: row.get::<_, i64>(13)? != 0,
            })
        })
        .map_err(|e| format!("cannot query watch state: {e}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("cannot decode watch state: {e}"))
}

fn insert_events(
    conn: &Connection,
    repo_id: i64,
    full_name: &str,
    inputs: Vec<EventInput>,
) -> Result<Vec<WatchEvent>, String> {
    let mut inserted = Vec::new();
    for input in inputs {
        let at = now();
        let changed = conn
            .execute(
                "INSERT OR IGNORE INTO watch_events
                 (repo_id, event_key, category, kind, action, title, detail, actor, url,
                  occurred_at, seen, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0, ?11)",
                params![
                    repo_id,
                    input.key,
                    input.category,
                    input.kind,
                    input.action,
                    input.title,
                    input.detail,
                    input.actor,
                    input.url,
                    input.occurred_at,
                    at
                ],
            )
            .map_err(|e| format!("cannot record alert: {e}"))?;
        if changed == 0 {
            continue;
        }
        inserted.push(WatchEvent {
            id: conn.last_insert_rowid(),
            repo_id,
            repo_full_name: full_name.to_string(),
            category: input.category.to_string(),
            kind: input.kind,
            action: input.action,
            title: input.title,
            detail: input.detail,
            actor: input.actor,
            url: input.url,
            occurred_at: input.occurred_at,
            seen: false,
            created_at: at,
        });
    }
    Ok(inserted)
}

#[tauri::command]
pub fn watchtower_snapshot(db: State<'_, Db>) -> Result<WatchtowerSnapshot, String> {
    snapshot(&db)
}

/// Adds a repository to Watchtower. The repository metadata comes from the
/// real GitHub API, so a local favourite is never fabricated.
#[tauri::command]
pub async fn watchtower_watch(
    db: State<'_, Db>,
    owner: String,
    repo: String,
) -> Result<WatchedRepo, String> {
    let (owner, repo) = github::coords(&owner, &repo)?;
    let token = github::token()?;
    {
        let conn = lock(&db);
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM watched_repos", [], |row| row.get(0))
            .map_err(|e| format!("cannot count watched repositories: {e}"))?;
        if count >= MAX_WATCHED {
            return Err(format!(
                "Watchtower is limited to {MAX_WATCHED} repositories. Remove one before adding another."
            ));
        }
    }

    let value = github::get_json(&token, &format!("{}/repos/{owner}/{repo}", github::API)).await?;
    let data: Repo = serde_json::from_value(value)
        .map_err(|e| format!("GitHub returned an unreadable repository: {e}"))?;
    if data.full_name.trim().is_empty() {
        return Err("GitHub did not return a repository name.".to_string());
    }
    let parts: Vec<&str> = data.full_name.split('/').collect();
    let owner = parts.first().copied().unwrap_or(&owner).to_string();
    let name = parts.get(1).copied().unwrap_or(&repo).to_string();
    let at = now();
    {
        let conn = lock(&db);
        conn.execute(
            "INSERT INTO watched_repos
             (owner, name, full_name, html_url, description, language, private, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
             ON CONFLICT(full_name) DO UPDATE SET
               owner = excluded.owner,
               name = excluded.name,
               html_url = excluded.html_url,
               description = excluded.description,
               language = excluded.language,
               private = excluded.private,
               updated_at = excluded.updated_at",
            params![
                owner,
                name,
                data.full_name,
                data.html_url,
                data.description,
                data.language,
                if data.private { 1 } else { 0 },
                at
            ],
        )
        .map_err(|e| format!("cannot save watch: {e}"))?;
    }
    let conn = lock(&db);
    conn.query_row(
        "SELECT id, owner, name, full_name, html_url, description, language,
                private, last_polled_at, last_error, created_at
         FROM watched_repos WHERE full_name = ?1",
        params![data.full_name],
        watched_from_row,
    )
    .map_err(|e| format!("cannot read the saved watch: {e}"))
}

#[tauri::command]
pub fn watchtower_unwatch(db: State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = lock(&db);
    let changed = conn
        .execute("DELETE FROM watched_repos WHERE id = ?1", params![id])
        .map_err(|e| format!("cannot remove watch: {e}"))?;
    if changed == 0 {
        return Err("watch not found".to_string());
    }
    Ok(())
}

/// Marks one alert read, or all alerts when `id` is absent.
#[tauri::command]
pub fn watchtower_mark_read(db: State<'_, Db>, id: Option<i64>) -> Result<i64, String> {
    let conn = lock(&db);
    match id {
        Some(id) => {
            conn.execute("UPDATE watch_events SET seen = 1 WHERE id = ?1", params![id])
                .map_err(|e| format!("cannot mark alert read: {e}"))?;
        }
        None => {
            conn.execute("UPDATE watch_events SET seen = 1 WHERE seen = 0", [])
                .map_err(|e| format!("cannot mark alerts read: {e}"))?;
        }
    }
    drop(conn);
    unread_count(&db)
}

enum Conditional {
    NotModified,
    Fresh(reqwest::Response),
}

fn response_etag(response: &reqwest::Response) -> Option<String> {
    response
        .headers()
        .get("etag")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
}

async fn conditional_get(
    token: &str,
    url: &str,
    etag: Option<&str>,
) -> Result<Conditional, String> {
    let client = github::client()?;
    let mut request = client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", github::API_VERSION)
        .bearer_auth(token);
    if let Some(etag) = etag.filter(|value| !value.trim().is_empty()) {
        request = request.header("If-None-Match", etag);
    }
    let response = request
        .send()
        .await
        .map_err(|e| format!("Cannot reach api.github.com: {e}"))?;
    if response.status().as_u16() == 304 {
        return Ok(Conditional::NotModified);
    }
    if !response.status().is_success() {
        return Err(github::describe_error(response).await);
    }
    Ok(Conditional::Fresh(response))
}

fn short(value: &str, limit: usize) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= limit {
        compact
    } else {
        format!("{}…", compact.chars().take(limit).collect::<String>())
    }
}

fn text<'a>(value: &'a serde_json::Value, pointer: &str) -> Option<&'a str> {
    value.pointer(pointer)?.as_str()
}

fn event_from_value(
    value: &serde_json::Value,
    full_name: &str,
    categories: &HashSet<String>,
) -> Option<EventInput> {
    let kind = text(value, "/type")?;
    let key = value.get("id")?.to_string();
    let actor = text(value, "/actor/login").unwrap_or("someone").to_string();
    let occurred_at = text(value, "/created_at").unwrap_or_default().to_string();
    let payload = value.get("payload")?;
    let repo_url = format!("https://github.com/{full_name}");

    let action: String;
    let (category, title, detail, url) = match kind {
        "PushEvent" => {
            let commits = payload
                .get("commits")
                .and_then(|commits| commits.as_array())
                .cloned()
                .unwrap_or_default();
            let count = commits.len();
            let branch = text(payload, "/ref")
                .unwrap_or("a branch")
                .trim_start_matches("refs/heads/")
                .to_string();
            let title = commits
                .last()
                .and_then(|commit| text(commit, "/message"))
                .map(|message| short(message.lines().next().unwrap_or(message), 160))
                .filter(|message| !message.is_empty())
                .unwrap_or_else(|| format!("{count} commit{} pushed", if count == 1 { "" } else { "s" }));
            action = "pushed".to_string();
            (
                "commits",
                title,
                format!(
                    "{actor} pushed {count} commit{} to {branch}",
                    if count == 1 { "" } else { "s" }
                ),
                format!("{repo_url}/commits/{branch}"),
            )
        }
        "PullRequestEvent" => {
            let pr = payload.get("pull_request")?;
            let number = pr.get("number").or_else(|| payload.get("number")).and_then(|n| n.as_u64()).unwrap_or(0);
            let merged = pr.get("merged_at").map(|value| !value.is_null()).unwrap_or(false);
            let raw_action = text(payload, "/action").unwrap_or("updated");
            action = if merged {
                "merged".to_string()
            } else if raw_action == "synchronize" {
                "updated".to_string()
            } else {
                raw_action.to_string()
            };
            let title = format!("#{number} {}", text(pr, "/title").unwrap_or("Pull request"));
            (
                "pull_requests",
                short(&title, 180),
                format!("{actor} {action} a pull request"),
                text(pr, "/html_url").unwrap_or(&repo_url).to_string(),
            )
        }
        "IssuesEvent" => {
            let issue = payload.get("issue")?;
            let number = issue.get("number").and_then(|n| n.as_u64()).unwrap_or(0);
            action = text(payload, "/action").unwrap_or("updated").to_string();
            let title = format!("#{number} {}", text(issue, "/title").unwrap_or("Issue"));
            (
                "issues",
                short(&title, 180),
                format!("{actor} {action} an issue"),
                text(issue, "/html_url").unwrap_or(&repo_url).to_string(),
            )
        }
        "IssueCommentEvent" => {
            let issue = payload.get("issue")?;
            let number = issue.get("number").and_then(|n| n.as_u64()).unwrap_or(0);
            action = "commented".to_string();
            let title = format!("#{number} {}", text(issue, "/title").unwrap_or("Issue"));
            (
                "issues",
                short(&title, 180),
                format!("{actor} commented on an issue"),
                text(payload, "/comment/html_url")
                    .or_else(|| text(issue, "/html_url"))
                    .unwrap_or(&repo_url)
                    .to_string(),
            )
        }
        "ReleaseEvent" => {
            let release = payload.get("release")?;
            action = text(payload, "/action").unwrap_or("published").to_string();
            let tag = text(release, "/tag_name").unwrap_or("release");
            let title = text(release, "/name").filter(|name| !name.trim().is_empty()).unwrap_or(tag);
            (
                "releases",
                short(title, 160),
                format!("{actor} {action} release {tag}"),
                text(release, "/html_url").unwrap_or(&repo_url).to_string(),
            )
        }
        "PullRequestReviewEvent" | "PullRequestReviewCommentEvent" => {
            let pr = payload.get("pull_request")?;
            let number = pr.get("number").and_then(|n| n.as_u64()).unwrap_or(0);
            let raw_action = text(payload, "/action").unwrap_or("reviewed");
            let state = text(payload, "/review/state").unwrap_or("").to_lowercase();
            action = if kind == "PullRequestReviewCommentEvent" {
                "commented".to_string()
            } else if state == "approved" {
                "approved".to_string()
            } else if state == "changes_requested" {
                "requested changes on".to_string()
            } else if raw_action == "dismissed" {
                "dismissed a review on".to_string()
            } else {
                "reviewed".to_string()
            };
            let title = format!("#{number} {}", text(pr, "/title").unwrap_or("Pull request"));
            (
                "reviews",
                short(&title, 180),
                format!("{actor} {action} a pull request"),
                text(payload, "/review/html_url")
                    .or_else(|| text(pr, "/html_url"))
                    .unwrap_or(&repo_url)
                    .to_string(),
            )
        }
        "WatchEvent" => {
            action = "starred".to_string();
            (
                "stars",
                full_name.to_string(),
                format!("{actor} starred this repository"),
                repo_url,
            )
        }
        "ForkEvent" => {
            action = "forked".to_string();
            let fork = text(payload, "/forkee/full_name").unwrap_or("a fork");
            (
                "forks",
                fork.to_string(),
                format!("{actor} forked this repository"),
                text(payload, "/forkee/html_url").unwrap_or(&repo_url).to_string(),
            )
        }
        "WorkflowRunEvent" => {
            let run = payload.get("workflow_run")?;
            let conclusion = text(run, "/conclusion").unwrap_or("completed");
            if !matches!(
                conclusion,
                "failure" | "timed_out" | "action_required" | "startup_failure"
            ) {
                return None;
            }
            action = conclusion.to_string();
            let title = text(run, "/name").unwrap_or("Workflow run");
            let branch = text(run, "/head_branch").unwrap_or("unknown branch");
            (
                "workflows",
                short(title, 160),
                format!("Workflow {conclusion} on {branch}"),
                text(run, "/html_url").unwrap_or(&repo_url).to_string(),
            )
        }
        _ => return None,
    };

    if !categories.contains(category) {
        return None;
    }

    Some(EventInput {
        key: format!("event:{key}"),
        category,
        kind: kind.to_string(),
        action,
        title,
        detail,
        actor,
        url,
        occurred_at,
    })
}

fn parse_events(
    value: &serde_json::Value,
    full_name: &str,
    categories: &HashSet<String>,
) -> Vec<EventInput> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| event_from_value(item, full_name, categories))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_workflow_runs(
    value: &serde_json::Value,
    full_name: &str,
    categories: &HashSet<String>,
) -> Vec<EventInput> {
    if !categories.contains("workflows") {
        return Vec::new();
    }
    let repo_url = format!("https://github.com/{full_name}");
    value
        .get("workflow_runs")
        .and_then(|runs| runs.as_array())
        .map(|runs| {
            runs.iter()
                .filter_map(|run| {
                    let conclusion = text(run, "/conclusion")?;
                    if !matches!(
                        conclusion,
                        "failure" | "timed_out" | "action_required" | "startup_failure"
                    ) {
                        return None;
                    }
                    let id = run.get("id").and_then(|id| id.as_i64()).unwrap_or(0);
                    let updated = text(run, "/updated_at").unwrap_or_default();
                    let branch = text(run, "/head_branch").unwrap_or("unknown branch");
                    let title = text(run, "/name").unwrap_or("Workflow run");
                    Some(EventInput {
                        key: format!("workflow:{id}:{updated}"),
                        category: "workflows",
                        kind: "WorkflowRun".to_string(),
                        action: conclusion.to_string(),
                        title: short(title, 160),
                        detail: format!("Workflow {conclusion} on {branch}"),
                        actor: text(run, "/triggering_actor/login")
                            .or_else(|| text(run, "/actor/login"))
                            .unwrap_or("GitHub")
                            .to_string(),
                        url: text(run, "/html_url")
                            .unwrap_or(&repo_url)
                            .to_string(),
                        occurred_at: if updated.is_empty() {
                            text(run, "/created_at").unwrap_or_default().to_string()
                        } else {
                            updated.to_string()
                        },
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Polls every favourite with conditional requests. The first successful poll
/// for a repository establishes a baseline, so adding an old repository does
/// not flood the history with months of activity.
#[tauri::command]
pub async fn watchtower_poll(
    db: State<'_, Db>,
    categories: Vec<String>,
) -> Result<WatchtowerPoll, String> {
    let token = github::token()?;
    let categories: HashSet<String> = categories
        .into_iter()
        .filter(|category| CATEGORIES.contains(&category.as_str()))
        .collect();
    if categories.is_empty() {
        return Err("Select at least one alert category.".to_string());
    }

    let rows = watch_rows(&db)?;
    let mut result = WatchtowerPoll {
        checked: 0,
        not_modified: 0,
        new_events: Vec::new(),
        errors: Vec::new(),
    };

    for row in rows {
        let repo = row.repo;
        let mut etag = row.etag.clone();
        let mut actions_etag = row.actions_etag.clone();
        let mut initialized = row.initialized;
        let baseline = !initialized;
        let mut last_error: Option<String> = None;
        let events_url = format!(
            "{}/repos/{}/{}/events?per_page=30",
            github::API,
            repo.owner,
            repo.name
        );

        match conditional_get(&token, &events_url, etag.as_deref()).await {
            Ok(Conditional::NotModified) => result.not_modified += 1,
            Ok(Conditional::Fresh(response)) => {
                let next_etag = response_etag(&response);
                match response.json::<serde_json::Value>().await {
                    Ok(value) => {
                        if initialized {
                            let inputs = parse_events(&value, &repo.full_name, &categories);
                            if !inputs.is_empty() {
                                let conn = lock(&db);
                                match insert_events(&conn, repo.id, &repo.full_name, inputs) {
                                    Ok(mut fresh) => result.new_events.append(&mut fresh),
                                    Err(error) => last_error = Some(error),
                                }
                            }
                        } else {
                            initialized = true;
                        }
                        if last_error.is_none() {
                            etag = next_etag;
                        }
                    }
                    Err(error) => {
                        last_error = Some(format!("unreadable event response: {error}"));
                    }
                }
            }
            Err(error) => last_error = Some(error),
        }

        if last_error.is_none() && categories.contains("workflows") {
            let actions_url = format!(
                "{}/repos/{}/{}/actions/runs?per_page=10",
                github::API,
                repo.owner,
                repo.name
            );
            match conditional_get(&token, &actions_url, actions_etag.as_deref()).await {
                Ok(Conditional::NotModified) => result.not_modified += 1,
                Ok(Conditional::Fresh(response)) => {
                    let next_etag = response_etag(&response);
                    match response.json::<serde_json::Value>().await {
                        Ok(value) => {
                            if initialized && !baseline {
                                let inputs =
                                    parse_workflow_runs(&value, &repo.full_name, &categories);
                                if !inputs.is_empty() {
                                    let conn = lock(&db);
                                    match insert_events(
                                        &conn,
                                        repo.id,
                                        &repo.full_name,
                                        inputs,
                                    ) {
                                        Ok(mut fresh) => result.new_events.append(&mut fresh),
                                        Err(error) => last_error = Some(error),
                                    }
                                }
                            }
                            if last_error.is_none() {
                                actions_etag = next_etag;
                            }
                        }
                        Err(error) => {
                            last_error = Some(format!("unreadable workflow response: {error}"));
                        }
                    }
                }
                Err(error) => last_error = Some(error),
            }
        }

        result.checked += 1;
        if let Some(error) = &last_error {
            result.errors.push(format!("{}: {error}", repo.full_name));
        }
        let conn = lock(&db);
        conn.execute(
            "UPDATE watched_repos
             SET etag = ?1, actions_etag = ?2, initialized = ?3, last_polled_at = ?4,
                 last_error = ?5, updated_at = ?4
             WHERE id = ?6",
            params![
                etag,
                actions_etag,
                if initialized { 1 } else { 0 },
                now(),
                last_error,
                repo.id
            ],
        )
        .map_err(|e| format!("cannot save watch state: {e}"))?;
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn temp_path(label: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "graft-watch-{label}-{}-{nanos}.db",
            std::process::id()
        ))
    }

    fn cleanup(path: &Path) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(format!("{}-wal", path.display()));
        let _ = std::fs::remove_file(format!("{}-shm", path.display()));
    }

    fn categories(values: &[&str]) -> HashSet<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    fn seed_watch(db: &Db) -> i64 {
        let conn = lock(db);
        conn.execute(
            "INSERT INTO watched_repos
             (owner, name, full_name, html_url, description, language, private, created_at, updated_at)
             VALUES ('o', 'r', 'o/r', 'https://github.com/o/r', 'rice', 'Rust', 0, 100, 100)",
            [],
        )
        .expect("insert watch");
        conn.last_insert_rowid()
    }

    #[test]
    fn maps_push_events_into_commit_alerts() {
        let value = serde_json::json!({
            "id": "123",
            "type": "PushEvent",
            "actor": { "login": "bender" },
            "created_at": "2026-11-08T12:00:00Z",
            "payload": {
                "ref": "refs/heads/main",
                "commits": [
                    { "message": "Add Watchtower polling" },
                    { "message": "Document alert categories" }
                ]
            }
        });
        let event = event_from_value(&value, "o/r", &categories(&["commits"]))
            .expect("push event");
        assert_eq!(event.category, "commits");
        assert_eq!(event.title, "Document alert categories");
        assert!(event.detail.contains("pushed 2 commits"));
        assert_eq!(event.url, "https://github.com/o/r/commits/main");
        assert!(event_from_value(&value, "o/r", &categories(&["issues"])).is_none());
    }

    #[test]
    fn maps_reviews_and_merges() {
        let review = serde_json::json!({
            "id": "1",
            "type": "PullRequestReviewEvent",
            "actor": { "login": "reviewer" },
            "created_at": "2026-11-08T12:00:00Z",
            "payload": {
                "action": "submitted",
                "review": { "state": "approved", "html_url": "https://github.com/o/r/pull/3#review" },
                "pull_request": { "number": 3, "title": "Branch graph", "html_url": "https://github.com/o/r/pull/3" }
            }
        });
        let event = event_from_value(&review, "o/r", &categories(&["reviews"])).expect("review");
        assert_eq!(event.action, "approved");
        assert!(event.detail.contains("reviewer approved"));

        let merged = serde_json::json!({
            "id": "2",
            "type": "PullRequestEvent",
            "actor": { "login": "bender" },
            "created_at": "2026-11-08T12:00:00Z",
            "payload": {
                "action": "closed",
                "number": 4,
                "pull_request": {
                    "number": 4,
                    "title": "Ship it",
                    "merged_at": "2026-11-08T11:00:00Z",
                    "html_url": "https://github.com/o/r/pull/4"
                }
            }
        });
        let event = event_from_value(&merged, "o/r", &categories(&["pull_requests"]))
            .expect("pull request");
        assert_eq!(event.action, "merged");
    }

    #[test]
    fn only_reports_failed_workflow_runs() {
        let value = serde_json::json!({
            "workflow_runs": [
                {
                    "id": 9,
                    "name": "verify",
                    "conclusion": "failure",
                    "head_branch": "main",
                    "updated_at": "2026-11-08T12:00:00Z",
                    "html_url": "https://github.com/o/r/actions/runs/9"
                },
                {
                    "id": 10,
                    "name": "docs",
                    "conclusion": "success",
                    "head_branch": "main",
                    "updated_at": "2026-11-08T12:01:00Z"
                }
            ]
        });
        let runs = parse_workflow_runs(&value, "o/r", &categories(&["workflows"]));
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].action, "failure");
        assert_eq!(runs[0].category, "workflows");
    }

    #[test]
    fn history_deduplicates_and_marks_read() {
        let path = temp_path("history");
        let db = Db::open(&path).expect("open");
        let repo_id = seed_watch(&db);
        let inputs = vec![EventInput {
            key: "event:1".into(),
            category: "commits",
            kind: "PushEvent".into(),
            action: "pushed".into(),
            title: "One".into(),
            detail: "pushed one commit".into(),
            actor: "bender".into(),
            url: "https://github.com/o/r/commits/main".into(),
            occurred_at: "2026-11-08T12:00:00Z".into(),
        }];
        {
            let conn = lock(&db);
            assert_eq!(insert_events(&conn, repo_id, "o/r", inputs.clone()).expect("insert").len(), 1);
            assert!(insert_events(&conn, repo_id, "o/r", inputs).expect("dedupe").is_empty());
        }
        let first = snapshot(&db).expect("snapshot");
        assert_eq!(first.events.len(), 1);
        assert_eq!(first.unread, 1);
        {
            let conn = lock(&db);
            conn.execute("UPDATE watch_events SET seen = 1", []).expect("read");
        }
        assert_eq!(snapshot(&db).expect("snapshot").unread, 0);
        cleanup(&path);
    }

    #[test]
    fn visits_increment_and_seed_recommendations() {
        let path = temp_path("visits");
        let db = Db::open(&path).expect("open");
        let input = || VisitInput {
            owner: "o".into(),
            name: "r".into(),
            html_url: "https://github.com/o/r".into(),
            description: Some("rice".into()),
            language: Some("Rust".into()),
            topics: vec!["hyprland".into()],
            stars: 10,
            forks: 2,
            default_branch: "main".into(),
            private: false,
        };
        store_visit(&db, input()).expect("first visit");
        store_visit(&db, input()).expect("second visit");
        let visited = list_visited(&db, 10).expect("visit history");
        assert_eq!(visited.len(), 1);
        assert_eq!(visited[0].visit_count, 2);
        assert_eq!(visited[0].topics, vec!["hyprland"]);

        let queries = recommendation_queries(&[], &[], &visited);
        assert!(queries.iter().any(|(query, _)| query.contains("topic:hyprland")));
        assert!(queries
            .iter()
            .any(|(query, _)| query.contains("language:\"Rust\"")));
        cleanup(&path);
    }
}
