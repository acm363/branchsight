use std::cmp::Reverse;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde_json::{Value, json};

use crate::config::{canonical, expand_home};

const SCAN_DEPTH: usize = 3;
const SKIP_DIRS: [&str; 4] = ["node_modules", "target", "dist", "vendor"];

pub fn list(roots: &[PathBuf], extra: &[String]) -> Value {
    let mut found = Vec::new();
    let extra = extra.iter().map(|path| canonical(expand_home(path)));
    for root in roots.iter().cloned().chain(extra) {
        scan(&root, SCAN_DEPTH, &mut found);
    }
    let mut seen = HashSet::new();
    found.retain(|dir| seen.insert(dir.clone()));
    let mut projects: Vec<(u64, Value)> = found
        .iter()
        .map(|dir| {
            let active = last_activity(dir);
            (active, json!({ "name": dir.file_name().map(|n| n.to_string_lossy()), "path": dir.to_string_lossy(), "active": active }))
        })
        .collect();
    projects.sort_by_key(|(active, _)| Reverse(*active));
    Value::Array(projects.into_iter().map(|(_, project)| project).collect())
}

fn scan(dir: &Path, depth: usize, found: &mut Vec<PathBuf>) {
    if dir.join(".git").exists() {
        found.push(canonical(dir.to_path_buf()));
        return;
    }
    if depth == 0 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') || SKIP_DIRS.contains(&name.as_ref()) {
            continue;
        }
        let path = entry.path();
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() || (kind.is_symlink() && path.is_dir()) {
            scan(&path, depth - 1, found);
        }
    }
}

fn last_activity(repo: &Path) -> u64 {
    ["logs/HEAD", "HEAD", ""]
        .iter()
        .find_map(|f| fs::metadata(repo.join(".git").join(f)).ok())
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs())
}
