use std::collections::HashSet;
use std::io::{Read, Write};
use std::net::{Ipv6Addr, SocketAddr, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex};
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};
use tiny_http::{Header, Method, Request, Response, Server};

use crate::git::Repo;
use crate::projects;
use crate::url::query;

const INDEX_HTML: &str = include_str!("web/index.html");
const STYLE_CSS: &str = include_str!("web/style.css");
const APP_JS: &str = include_str!("web/app.js");
const GRAPH_JS: &str = include_str!("web/graph.js");
const FAVICON_SVG: &str = include_str!("web/favicon.svg");
const PING: &str = "branchsight";
const DEFAULT_LIMIT: usize = 5000;
const MAX_LIMIT: usize = 200_000;

pub struct Config {
    pub roots: Vec<PathBuf>,
    pub default_repo: Option<PathBuf>,
}

pub struct Reply {
    pub status: u16,
    pub content_type: &'static str,
    pub body: String,
}

type Routed = (String, &'static str);
type BindError = Box<dyn std::error::Error + Send + Sync>;

static FETCHING: LazyLock<Mutex<HashSet<PathBuf>>> = LazyLock::new(Default::default);

impl Config {
    pub fn home_roots() -> Vec<PathBuf> {
        home_dir().into_iter().collect()
    }
}

pub fn listen(port: u16) -> Result<(Server, Option<Server>), BindError> {
    let ipv4 = Server::http(("127.0.0.1", port))?;
    let ipv6 = Server::http((Ipv6Addr::LOCALHOST, port)).ok();
    Ok((ipv4, ipv6))
}

pub fn spawn_first_free(ports: impl IntoIterator<Item = u16>, config: Config) -> Result<u16, BindError> {
    let mut last = None;
    for port in ports.into_iter().chain([0]) {
        match Server::http(("127.0.0.1", port)) {
            Ok(http) => {
                let port = http.server_addr().to_ip().map_or(port, |a: SocketAddr| a.port());
                let config = Arc::new(config);
                thread::spawn(move || serve(http, config));
                return Ok(port);
            }
            Err(e) => last = Some(e),
        }
    }
    Err(last.unwrap_or_else(|| "no port to bind".into()))
}

pub fn serve(http: Server, config: Arc<Config>) {
    for req in http.incoming_requests() {
        let config = Arc::clone(&config);
        thread::spawn(move || handle(&config, req));
    }
}

pub fn handle(cfg: &Config, req: Request) {
    let host = req.headers().iter().find(|h| h.field.equiv("Host")).map(|h| h.value.as_str());
    if !host.is_some_and(is_local_host) {
        let _ = req.respond(Response::from_string("forbidden host").with_status_code(403));
        return;
    }
    let reply = respond(cfg, *req.method() == Method::Post, req.url());
    let response = Response::from_string(reply.body)
        .with_status_code(reply.status)
        .with_header(header("Content-Type", reply.content_type))
        .with_header(header("Cache-Control", "no-store"));
    let _ = req.respond(response);
}

pub fn respond(cfg: &Config, post: bool, url: &str) -> Reply {
    match route(cfg, post, url) {
        Some(Ok((body, content_type))) => Reply { status: 200, content_type, body },
        Some(Err(body)) => Reply { status: 400, content_type: "text/plain; charset=utf-8", body },
        None => Reply { status: 404, content_type: "text/plain; charset=utf-8", body: "not found".into() },
    }
}

fn route(cfg: &Config, post: bool, url: &str) -> Option<Result<Routed, String>> {
    let path = url.split('?').next().unwrap_or("/");
    let params = query(url);
    let param = |k: &str| params.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
    let repo = || open_repo(cfg, param("repo"));

    Some(match path {
        "/" => Ok((INDEX_HTML.to_owned(), "text/html; charset=utf-8")),
        "/style.css" => Ok((STYLE_CSS.to_owned(), "text/css; charset=utf-8")),
        "/app.js" => Ok((APP_JS.to_owned(), "text/javascript; charset=utf-8")),
        "/graph.js" => Ok((GRAPH_JS.to_owned(), "text/javascript; charset=utf-8")),
        "/favicon.svg" => Ok((FAVICON_SVG.to_owned(), "image/svg+xml")),
        "/api/ping" => Ok((PING.to_owned(), "text/plain")),
        "/api/context" => Ok(json_reply(json!({
            "version": env!("CARGO_PKG_VERSION"),
            "default": cfg.default_repo.as_ref().map(|p| p.to_string_lossy()),
            "roots": cfg.roots.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>(),
        }))),
        "/api/projects" => Ok(json_reply(projects::list(&cfg.roots))),
        "/api/repo" => repo().and_then(|r| r.info()).map(json_reply),
        "/api/refs" => repo().and_then(|r| r.refs()).map(json_reply),
        "/api/log" => {
            let refs: Vec<String> = params.iter().filter(|(k, _)| k == "ref").map(|(_, v)| v.clone()).collect();
            let limit = param("limit").and_then(|l| l.parse().ok()).unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
            repo().and_then(|r| r.log(&refs, limit)).map(json_reply)
        }
        "/api/state" => repo().and_then(|r| r.fingerprint()).map(|v| json_reply(json!({ "v": v }))),
        "/api/fetch" if post => repo().and_then(|r| fetch(&r)).map(|out| json_reply(json!({ "out": out }))),
        "/api/checkout" if post => repo().and_then(|r| r.checkout(param("ref").unwrap_or(""))).map(|out| json_reply(json!({ "out": out }))),
        "/api/delete-remote" if post => repo()
            .and_then(|r| r.delete_remote_branch(param("ref").unwrap_or("")))
            .map(|out| json_reply(json!({ "out": out }))),
        "/api/fetch" | "/api/checkout" | "/api/delete-remote" => Err("POST required".into()),
        "/api/commit" => repo()
            .and_then(|r| r.show(param("h").unwrap_or("")))
            .map(|t| (t, "text/plain; charset=utf-8")),
        _ => return None,
    })
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

fn open_repo(cfg: &Config, requested: Option<&str>) -> Result<Repo, String> {
    let dir = requested
        .filter(|r| !r.is_empty())
        .map(expand_home)
        .or_else(|| cfg.default_repo.clone())
        .ok_or("no repository selected")?;
    Repo::open(&dir).ok_or_else(|| format!("not a git repository: {}", dir.display()))
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from)
}

fn expand_home(path: &str) -> PathBuf {
    match (path.strip_prefix("~/").or(path.strip_prefix("~\\")).or((path == "~").then_some("")), home_dir()) {
        (Some(rest), Some(home)) => home.join(rest),
        _ => PathBuf::from(path),
    }
}

pub fn is_running(port: u16) -> bool {
    let Ok(mut stream) = TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), Duration::from_millis(300)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
    let mut body = String::new();
    stream.write_all(b"GET /api/ping HTTP/1.0\r\nHost: localhost\r\n\r\n").is_ok()
        && stream.read_to_string(&mut body).is_ok()
        && body.ends_with(PING)
}

fn is_local_host(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    let name = match host.strip_prefix('[') {
        Some(v6) => v6.split(']').next().unwrap_or(""),
        None => host.split(':').next().unwrap_or(""),
    };
    matches!(name, "localhost" | "127.0.0.1" | "::1") || name.ends_with(".localhost")
}

fn json_reply(v: Value) -> Routed {
    (v.to_string(), "application/json")
}

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("static header")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_loopback_hosts_only() {
        for ok in ["localhost:7420", "127.0.0.1:7420", "[::1]:7420", "my-project.localhost:7420", "LOCALHOST"] {
            assert!(is_local_host(ok), "{ok}");
        }
        for ko in ["evil.com", "localhost.evil.com:7420", "127.0.0.2", "[::2]:7420", ""] {
            assert!(!is_local_host(ko), "{ko}");
        }
    }
}
