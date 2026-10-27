use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};

use serde_json::{Value, json};

use crate::assets;
use crate::config::{self, Config};
use crate::git::{FileDiff, Repo};
use crate::projects;
use crate::url::Query;

pub const PING: &str = "branchsight";
const DEFAULT_LIMIT: usize = 5000;
const MAX_LIMIT: usize = 200_000;
const TEXT: &str = "text/plain; charset=utf-8";
const JSON: &str = "application/json";

static FETCHING: LazyLock<Mutex<HashSet<PathBuf>>> = LazyLock::new(Default::default);

pub struct Reply {
    pub status: u16,
    pub content_type: &'static str,
    pub body: String,
}

impl Reply {
    fn ok(content_type: &'static str, body: String) -> Reply {
        Reply { status: 200, content_type, body }
    }

    fn json(value: Value) -> Reply {
        Reply::ok(JSON, value.to_string())
    }

    fn failed(status: u16, message: String) -> Reply {
        Reply { status, content_type: TEXT, body: message }
    }
}

pub fn respond(config: &Config, post: bool, url: &str) -> Reply {
    let path = url.split('?').next().unwrap_or("/");
    if let Some(asset) = assets::get(path) {
        return Reply::ok(asset.content_type, asset.body.to_owned());
    }
    route(config, post, path, &Query::parse(url)).unwrap_or_else(|message| Reply::failed(400, message))
}

fn route(config: &Config, post: bool, path: &str, q: &Query) -> Result<Reply, String> {
    let repo = || config::open_repo(config, q.get("repo"));
    let action = || if post { repo() } else { Err("POST required".to_owned()) };
    Ok(match path {
        "/api/ping" => Reply::ok(TEXT, PING.into()),
        "/api/context" => Reply::json(json!({
            "version": env!("CARGO_PKG_VERSION"),
            "default": config.default_repo.as_ref().map(|p| p.to_string_lossy()),
            "roots": config.roots.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>(),
        })),
        "/api/projects" => Reply::json(projects::list(&config.roots)),
        "/api/repo" => Reply::json(repo()?.info()?),
        "/api/refs" => Reply::json(repo()?.refs()?),
        "/api/log" => Reply::json(repo()?.log(&q.all("ref"), limit(q))?),
        "/api/state" => Reply::json(repo()?.state()?),
        "/api/commit" => Reply::json(repo()?.show(q.get("h"))?),
        "/api/compare" => Reply::json(repo()?.compare(q.get("from"), q.get("to"))?),
        "/api/worktree" => Reply::json(repo()?.worktree()?),
        "/api/filediff" => Reply::json(repo()?.file_diff(file_diff(q)?, &q.all("path"))?),
        "/api/fetch" => done(fetch(&action()?)?),
        "/api/checkout" => done(action()?.checkout(q.get("ref"))?),
        "/api/push" => done(action()?.push(q.get("ref"), q.get("remote"), q.get("mode"))?),
        "/api/delete-remote" => done(action()?.delete_remote_branch(q.get("ref"))?),
        _ => Reply::failed(404, "not found".into()),
    })
}

fn limit(q: &Query) -> usize {
    q.get("limit").parse().unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
}

fn file_diff(q: &Query) -> Result<FileDiff<'_>, String> {
    match q.get("kind") {
        "commit" => Ok(FileDiff::Commit(q.get("h"))),
        "compare" => Ok(FileDiff::Compare(q.get("from"), q.get("to"))),
        "staged" => Ok(FileDiff::Staged),
        "unstaged" => Ok(FileDiff::Unstaged),
        "untracked" => Ok(FileDiff::Untracked),
        other => Err(format!("unknown kind {other}")),
    }
}

fn done(out: String) -> Reply {
    Reply::json(json!({ "out": out }))
}

fn fetch(repo: &Repo) -> Result<String, String> {
    if !FETCHING.lock().map_err(|e| e.to_string())?.insert(repo.root.clone()) {
        return Err("a fetch is already running for this repository".into());
    }
    let result = repo.fetch();
    if let Ok(mut running) = FETCHING.lock() {
        running.remove(&repo.root);
    }
    result
}
