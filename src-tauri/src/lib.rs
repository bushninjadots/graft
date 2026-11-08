mod commands;
mod db;
mod git;
mod github;
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
            commands::open_external,
            github::github_status,
            github::github_set_client_id,
            github::github_start_device,
            github::github_poll_device,
            github::github_save_token,
            github::github_sign_out,
            github::github_list_repos,
            github::github_touch_profile,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start GRAFT");
}
