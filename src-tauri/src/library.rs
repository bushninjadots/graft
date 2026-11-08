//! Standalone Explore saved searches and the local Collections library.
//! Collection entries copy real repository metadata from GitHub once; notes
//! and review state remain local.

use crate::db::{now, Db};
use crate::github::{self, Repo};
use rusqlite::{params, Connection};
use serde::Serialize;
use tauri::State;

const STATUSES: [&str; 5] = [
    "want_to_try",
    "studying",
    "using",
    "contributing",
    "archived",
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedSearch {
    pub id: i64,
    pub name: String,
    pub query: String,
    pub language: String,
    pub topic: String,
    pub author: String,
    pub sort: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub repo_count: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionRepo {
    pub id: i64,
    pub collection_id: i64,
    pub owner: String,
    pub name: String,
    pub full_name: String,
    pub html_url: String,
    pub description: Option<String>,
    pub language: Option<String>,
    pub private: bool,
    pub topics: Vec<String>,
    pub stars: i64,
    pub forks: i64,
    pub default_branch: String,
    pub status: String,
    pub notes: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionDetail {
    pub collection: Collection,
    pub repos: Vec<CollectionRepo>,
}

fn lock(db: &Db) -> std::sync::MutexGuard<'_, Connection> {
    db.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn clean_name(raw: &str) -> Result<String, String> {
    let value = raw.trim();
    if value.is_empty() {
        return Err("A name is required.".to_string());
    }
    if value.chars().count() > 80 {
        return Err("Names must stay under 80 characters.".to_string());
    }
    Ok(value.to_string())
}

fn clean_notes(raw: &str) -> Result<String, String> {
    if raw.len() > 4000 {
        return Err("Notes must stay under 4,000 characters.".to_string());
    }
    Ok(raw.trim().to_string())
}

fn clean_status(raw: &str) -> Result<String, String> {
    if STATUSES.contains(&raw) {
        Ok(raw.to_string())
    } else {
        Err("Choose a valid collection status.".to_string())
    }
}

fn saved_search_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SavedSearch> {
    Ok(SavedSearch {
        id: row.get(0)?,
        name: row.get(1)?,
        query: row.get(2)?,
        language: row.get(3)?,
        topic: row.get(4)?,
        author: row.get(5)?,
        sort: row.get(6)?,
        created_at: row.get(7)?,
    })
}

fn collection_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Collection> {
    Ok(Collection {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        repo_count: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

fn collection_repo_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CollectionRepo> {
    let topics: String = row.get(8)?;
    Ok(CollectionRepo {
        id: row.get(0)?,
        collection_id: row.get(1)?,
        owner: row.get(2)?,
        name: row.get(3)?,
        full_name: row.get(4)?,
        html_url: row.get(5)?,
        description: row.get(6)?,
        language: row.get(7)?,
        private: row.get::<_, i64>(9)? != 0,
        topics: serde_json::from_str(&topics).unwrap_or_default(),
        stars: row.get(10)?,
        forks: row.get(11)?,
        default_branch: row.get(12)?,
        status: row.get(13)?,
        notes: row.get(14)?,
        created_at: row.get(15)?,
        updated_at: row.get(16)?,
    })
}

#[tauri::command]
pub fn explore_saved_searches(db: State<'_, Db>) -> Result<Vec<SavedSearch>, String> {
    let conn = lock(&db);
    let mut stmt = conn
        .prepare(
            "SELECT id, name, query, language, topic, author, sort, created_at
             FROM saved_searches ORDER BY created_at DESC, id DESC",
        )
        .map_err(|e| format!("cannot read saved searches: {e}"))?;
    let rows = stmt
        .query_map([], saved_search_from_row)
        .map_err(|e| format!("cannot query saved searches: {e}"))?;
    let searches = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("cannot decode saved searches: {e}"))?;
    Ok(searches)
}

#[tauri::command]
pub fn explore_save_search(
    db: State<'_, Db>,
    name: String,
    query: String,
    language: String,
    topic: String,
    author: String,
    sort: String,
) -> Result<SavedSearch, String> {
    let name = clean_name(&name)?;
    if query.trim().is_empty()
        && language.trim().is_empty()
        && topic.trim().is_empty()
        && author.trim().is_empty()
    {
        return Err("Run a search before saving it.".to_string());
    }
    let sort = match sort.as_str() {
        "stars" | "forks" | "updated" => sort,
        _ => "best".to_string(),
    };
    let at = now();
    let conn = lock(&db);
    conn.execute(
        "INSERT INTO saved_searches (name, query, language, topic, author, sort, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            name,
            query.trim(),
            language.trim(),
            topic.trim().trim_start_matches('#'),
            author.trim().trim_start_matches('@'),
            sort,
            at
        ],
    )
    .map_err(|e| format!("cannot save search: {e}"))?;
    let id = conn.last_insert_rowid();
    conn.query_row(
        "SELECT id, name, query, language, topic, author, sort, created_at
         FROM saved_searches WHERE id = ?1",
        params![id],
        saved_search_from_row,
    )
    .map_err(|e| format!("cannot read saved search: {e}"))
}

#[tauri::command]
pub fn explore_delete_search(db: State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = lock(&db);
    let changed = conn
        .execute("DELETE FROM saved_searches WHERE id = ?1", params![id])
        .map_err(|e| format!("cannot delete saved search: {e}"))?;
    if changed == 0 {
        return Err("saved search not found".to_string());
    }
    Ok(())
}

#[tauri::command]
pub fn collections_list(db: State<'_, Db>) -> Result<Vec<Collection>, String> {
    let conn = lock(&db);
    let mut stmt = conn
        .prepare(
            "SELECT c.id, c.name, c.description, COUNT(r.id), c.created_at, c.updated_at
             FROM collections c
             LEFT JOIN collection_repos r ON r.collection_id = c.id
             GROUP BY c.id
             ORDER BY c.updated_at DESC, c.name COLLATE NOCASE",
        )
        .map_err(|e| format!("cannot read collections: {e}"))?;
    let rows = stmt
        .query_map([], collection_from_row)
        .map_err(|e| format!("cannot query collections: {e}"))?;
    let collections = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("cannot decode collections: {e}"))?;
    Ok(collections)
}

#[tauri::command]
pub fn collections_create(
    db: State<'_, Db>,
    name: String,
    description: String,
) -> Result<Collection, String> {
    let name = clean_name(&name)?;
    if description.len() > 600 {
        return Err("Collection descriptions must stay under 600 characters.".to_string());
    }
    let at = now();
    let conn = lock(&db);
    conn.execute(
        "INSERT INTO collections (name, description, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?3)",
        params![name, description.trim(), at],
    )
    .map_err(|e| format!("cannot create collection: {e}"))?;
    let id = conn.last_insert_rowid();
    conn.query_row(
        "SELECT c.id, c.name, c.description, 0, c.created_at, c.updated_at
         FROM collections c WHERE c.id = ?1",
        params![id],
        collection_from_row,
    )
    .map_err(|e| format!("cannot read collection: {e}"))
}

#[tauri::command]
pub fn collections_delete(db: State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = lock(&db);
    let changed = conn
        .execute("DELETE FROM collections WHERE id = ?1", params![id])
        .map_err(|e| format!("cannot delete collection: {e}"))?;
    if changed == 0 {
        return Err("collection not found".to_string());
    }
    Ok(())
}

#[tauri::command]
pub fn collection_detail(db: State<'_, Db>, id: i64) -> Result<CollectionDetail, String> {
    let conn = lock(&db);
    let collection = conn
        .query_row(
            "SELECT c.id, c.name, c.description, COUNT(r.id), c.created_at, c.updated_at
             FROM collections c
             LEFT JOIN collection_repos r ON r.collection_id = c.id
             WHERE c.id = ?1 GROUP BY c.id",
            params![id],
            collection_from_row,
        )
        .map_err(|e| format!("cannot read collection: {e}"))?;
    let mut stmt = conn
        .prepare(
            "SELECT id, collection_id, owner, name, full_name, html_url, description, language,
                    topics, private, stars, forks, default_branch, status, notes,
                    created_at, updated_at
             FROM collection_repos WHERE collection_id = ?1
             ORDER BY updated_at DESC, full_name COLLATE NOCASE",
        )
        .map_err(|e| format!("cannot read collection repositories: {e}"))?;
    let repos = stmt
        .query_map(params![id], collection_repo_from_row)
        .map_err(|e| format!("cannot query collection repositories: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("cannot decode collection repositories: {e}"))?;
    Ok(CollectionDetail { collection, repos })
}

#[tauri::command]
pub async fn collection_add_repo(
    db: State<'_, Db>,
    collection_id: i64,
    owner: String,
    repo: String,
) -> Result<CollectionRepo, String> {
    let (owner, repo) = github::coords(&owner, &repo)?;
    {
        let conn = lock(&db);
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM collections WHERE id = ?1",
                params![collection_id],
                |row| row.get(0),
            )
            .map_err(|e| format!("cannot read collection: {e}"))?;
        if exists == 0 {
            return Err("collection not found".to_string());
        }
    }
    let token = github::token()?;
    let value = github::get_json(&token, &format!("{}/repos/{owner}/{repo}", github::API)).await?;
    let data: Repo = serde_json::from_value(value)
        .map_err(|e| format!("GitHub returned an unreadable repository: {e}"))?;
    let parts: Vec<&str> = data.full_name.split('/').collect();
    let owner = parts.first().copied().unwrap_or(&owner).to_string();
    let name = parts.get(1).copied().unwrap_or(&repo).to_string();
    let topics = serde_json::to_string(&data.topics).unwrap_or_else(|_| "[]".to_string());
    let at = now();
    let conn = lock(&db);
    conn.execute(
        "INSERT INTO collection_repos
         (collection_id, owner, name, full_name, html_url, description, language, private,
          topics, stars, forks, default_branch, status, notes, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 'want_to_try', '', ?13, ?13)
         ON CONFLICT(collection_id, full_name) DO UPDATE SET
           owner = excluded.owner,
           name = excluded.name,
           html_url = excluded.html_url,
           description = excluded.description,
           language = excluded.language,
           private = excluded.private,
           topics = excluded.topics,
           stars = excluded.stars,
           forks = excluded.forks,
           default_branch = excluded.default_branch,
           updated_at = excluded.updated_at",
        params![
            collection_id,
            owner,
            name,
            data.full_name,
            data.html_url,
            data.description,
            data.language,
            if data.private { 1 } else { 0 },
            topics,
            data.stargazers_count,
            data.forks_count,
            data.default_branch,
            at
        ],
    )
    .map_err(|e| format!("cannot add repository to collection: {e}"))?;
    conn.execute(
        "UPDATE collections SET updated_at = ?1 WHERE id = ?2",
        params![at, collection_id],
    )
    .map_err(|e| format!("cannot update collection: {e}"))?;
    conn.query_row(
        "SELECT id, collection_id, owner, name, full_name, html_url, description, language,
                topics, private, stars, forks, default_branch, status, notes,
                created_at, updated_at
         FROM collection_repos WHERE collection_id = ?1 AND full_name = ?2",
        params![collection_id, data.full_name],
        collection_repo_from_row,
    )
    .map_err(|e| format!("cannot read collection repository: {e}"))
}

#[tauri::command]
pub fn collection_update_repo(
    db: State<'_, Db>,
    id: i64,
    status: String,
    notes: String,
) -> Result<CollectionRepo, String> {
    let status = clean_status(&status)?;
    let notes = clean_notes(&notes)?;
    let at = now();
    let conn = lock(&db);
    let changed = conn
        .execute(
            "UPDATE collection_repos SET status = ?1, notes = ?2, updated_at = ?3 WHERE id = ?4",
            params![status, notes, at, id],
        )
        .map_err(|e| format!("cannot update collection repository: {e}"))?;
    if changed == 0 {
        return Err("collection repository not found".to_string());
    }
    let collection_id: i64 = conn
        .query_row(
            "SELECT collection_id FROM collection_repos WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .map_err(|e| format!("cannot read collection repository: {e}"))?;
    conn.execute(
        "UPDATE collections SET updated_at = ?1 WHERE id = ?2",
        params![at, collection_id],
    )
    .map_err(|e| format!("cannot update collection: {e}"))?;
    conn.query_row(
        "SELECT id, collection_id, owner, name, full_name, html_url, description, language,
                topics, private, stars, forks, default_branch, status, notes,
                created_at, updated_at
         FROM collection_repos WHERE id = ?1",
        params![id],
        collection_repo_from_row,
    )
    .map_err(|e| format!("cannot read collection repository: {e}"))
}

#[tauri::command]
pub fn collection_remove_repo(db: State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = lock(&db);
    let changed = conn
        .execute("DELETE FROM collection_repos WHERE id = ?1", params![id])
        .map_err(|e| format!("cannot remove collection repository: {e}"))?;
    if changed == 0 {
        return Err("collection repository not found".to_string());
    }
    Ok(())
}
