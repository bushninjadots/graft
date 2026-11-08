use crate::db::{now, Db, Project, SCHEMA_VERSION};
use crate::tools::{self, ToolInfo};
use serde::Serialize;
use std::path::Path;
use tauri::{Manager, State};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub data_dir: String,
    pub db_path: String,
    pub schema_version: i64,
}

#[tauri::command]
pub fn app_info(app: tauri::AppHandle, _db: State<'_, Db>) -> Result<AppInfo, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("cannot resolve application data directory: {e}"))?;
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        data_dir: data_dir.display().to_string(),
        db_path: data_dir.join("graft.db").display().to_string(),
        schema_version: SCHEMA_VERSION,
    })
}

#[tauri::command]
pub fn get_setting(db: State<'_, Db>, key: String) -> Result<Option<String>, String> {
    db.get_setting(&key)
}

#[tauri::command]
pub fn set_setting(db: State<'_, Db>, key: String, value: String) -> Result<(), String> {
    if key.trim().is_empty() {
        return Err("setting key must not be empty".into());
    }
    db.set_setting(&key, &value)
}

#[tauri::command]
pub fn list_projects(db: State<'_, Db>) -> Result<Vec<Project>, String> {
    db.list_projects()
}

#[tauri::command]
pub fn create_project(
    db: State<'_, Db>,
    name: String,
    path: String,
    description: String,
) -> Result<Project, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("A project name is required.".into());
    }
    if name.chars().count() > 120 {
        return Err("Project names are limited to 120 characters.".into());
    }

    let path = path.trim();
    if !path.is_empty() {
        if !Path::new(path).is_dir() {
            return Err(format!("Directory does not exist: {path}"));
        }
    }

    db.create_project(name, path, description.trim(), now())
}

#[tauri::command]
pub fn delete_project(db: State<'_, Db>, id: i64) -> Result<(), String> {
    db.delete_project(id)
}

#[tauri::command]
pub fn detect_tools() -> Vec<ToolInfo> {
    tools::detect()
}

#[tauri::command]
pub fn project_git_status(path: String) -> crate::git::GitStatus {
    crate::git::status(&path)
}

#[tauri::command]
pub fn git_action(path: String, action: String) -> crate::git::GitActionResult {
    crate::git::action(&path, &action)
}

#[tauri::command]
pub fn git_branches(path: String) -> Result<Vec<crate::git::GitBranch>, String> {
    crate::git::branches(&path)
}

#[tauri::command]
pub fn git_stage(path: String) -> crate::git::GitActionResult {
    crate::git::stage(&path)
}

#[tauri::command]
pub fn git_commit(
    path: String,
    message: String,
    stage_all: bool,
) -> crate::git::GitActionResult {
    crate::git::commit(&path, &message, stage_all)
}

#[tauri::command]
pub fn git_create_branch(
    path: String,
    name: String,
    switch: bool,
) -> crate::git::GitActionResult {
    crate::git::create_branch(&path, &name, switch)
}

#[tauri::command]
pub fn git_switch_branch(path: String, name: String) -> crate::git::GitActionResult {
    crate::git::switch_branch(&path, &name)
}

#[tauri::command]
pub fn clone_repo(url: String, dest: String) -> Result<String, String> {
    crate::git::clone(&url, &dest)
}

/// Suggests `~/Projects/<owner>-<repo>` (or `~/<owner>-<repo>` when there is no
/// Projects folder). The UI stays editable; nothing is created here.
#[tauri::command]
pub fn clone_destination(owner: String, repo: String) -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let projects = Path::new(&home).join("Projects");
    let base = if projects.is_dir() {
        projects
    } else {
        Path::new(&home).to_path_buf()
    };
    base.join(format!("{owner}-{repo}"))
        .display()
        .to_string()
}

/// Opens a link in the user's browser. Restricted to http(s) so a bad render can
/// never hand an arbitrary scheme to the desktop environment.
#[tauri::command]
pub fn open_external(url: String) -> Result<(), String> {
    let target = url.trim();
    if !(target.starts_with("https://") || target.starts_with("http://")) {
        return Err("Only http(s) links can be opened externally.".to_string());
    }

    std::process::Command::new("xdg-open")
        .arg(target)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Cannot open the link: {e}"))
}
