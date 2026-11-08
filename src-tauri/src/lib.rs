mod commands;
mod db;
mod git;
mod github;
mod profile;
mod secrets;
mod tools;

use db::Db;
use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;

            let db_path = data_dir.join("graft.db");
            let db = Db::open(&db_path).map_err(std::io::Error::other)?;
            app.manage(db);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::get_setting,
            commands::set_setting,
            commands::list_projects,
            commands::create_project,
            commands::delete_project,
            commands::detect_tools,
            commands::project_git_status,
            commands::git_action,
            commands::git_branches,
            commands::git_stage,
            commands::git_commit,
            commands::git_create_branch,
            commands::git_switch_branch,
            commands::clone_repo,
            commands::clone_destination,
            commands::open_external,
            github::github_status,
            github::github_set_client_id,
            github::github_start_device,
            github::github_poll_device,
            github::github_save_token,
            github::github_sign_out,
            github::github_list_repos,
            github::github_search_repos,
            github::github_notifications,
            github::github_touch_profile,
            github::github_repo_detail,
            github::github_repo_readme,
            github::github_repo_contents,
            github::github_repo_file,
            github::github_repo_branches,
            github::github_repo_commits,
            github::github_repo_commit,
            github::github_repo_pulls,
            github::github_repo_pull,
            github::github_fork,
            github::github_set_star,
            github::github_profile_readme,
            github::github_publish_profile,
            github::github_save_file,
            github::github_repo_issues,
            github::github_create_issue,
            github::github_create_profile_repo,
            profile::github_profile_snapshot,
            profile::github_set_profile_status,
            profile::profile_render,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start GRAFT");
}
