#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

use branchsight::git::Repo;
use branchsight::server::{self, Config};
use tauri::{WebviewUrl, WebviewWindowBuilder};

const PREFERRED_PORTS: std::ops::RangeInclusive<u16> = 7421..=7430;

fn main() {
    let default_repo = std::env::args().nth(1).map(PathBuf::from).and_then(|dir| Repo::open(&dir)).map(|r| r.root);
    let config = Config { roots: Config::home_roots(), default_repo };
    let port = server::spawn_first_free(PREFERRED_PORTS, config).expect("branchsight: cannot start the local server");
    let url = format!("http://127.0.0.1:{port}/").parse().expect("loopback url");

    tauri::Builder::default()
        .setup(move |app| {
            WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
                .title("BranchSight")
                .inner_size(1400.0, 900.0)
                .min_inner_size(640.0, 400.0)
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("branchsight: the desktop app failed to start");
}
