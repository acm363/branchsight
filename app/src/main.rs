#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::Arc;

use branchsight::api;
use branchsight::config::Config;
use branchsight::git::Repo;
use tauri::State;

#[tauri::command]
async fn api(config: State<'_, Arc<Config>>, method: String, path: String) -> Result<(u16, String), String> {
    let config = Arc::clone(&config);
    tauri::async_runtime::spawn_blocking(move || {
        let reply = api::respond(&config, method == "POST", &path);
        #[cfg(debug_assertions)]
        eprintln!("api {method} {} -> {}", path.split('?').next().unwrap_or(""), reply.status);
        (reply.status, reply.body)
    })
    .await
    .map_err(|e| e.to_string())
}

fn main() {
    #[cfg(target_os = "linux")]
    glib::set_prgname(Some("BranchSight"));
    let default_repo = std::env::args().nth(1).and_then(|dir| Repo::open(&PathBuf::from(dir)).ok()).map(|repo| repo.root);
    let config = Config { default_repo, ..Config::home() };

    tauri::Builder::default()
        .manage(Arc::new(config))
        .invoke_handler(tauri::generate_handler![api])
        .run(tauri::generate_context!())
        .expect("branchsight: the desktop app failed to start");
}
