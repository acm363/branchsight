use std::cmp::Reverse;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde_json::{Value, json};

const SCAN_DEPTH: usize = 3;
const SKIP_DIRS: [&str; 4] = ["node_modules", "target", "dist", "vendor"];

pub fn list(roots: &[PathBuf]) -> Value {
    let mut out = Vec::new();
    for root in roots {
        scan(root, SCAN_DEPTH, &mut out);
    }
    out.sort_by_key(|p| Reverse(p["active"].as_u64().unwrap_or(0)));
    out.dedup_by(|a, b| a["path"] == b["path"]);
    Value::Array(out)
}

fn scan(dir: &Path, depth: usize, out: &mut Vec<Value>) {
    if dir.join(".git").exists() {
        out.push(json!({
            "name": dir.file_name().map(|n| n.to_string_lossy().into_owned()),
            "path": dir.to_string_lossy(),
            "active": last_activity(dir),
        }));
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
        if fs::metadata(&path).is_ok_and(|meta| meta.is_dir()) {
            scan(&path, depth - 1, out);
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
