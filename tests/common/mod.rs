#![allow(dead_code)]

use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use branchsight::config::Config;
use branchsight::http::{self, PAGE_HEADER};
use branchsight::url::encode;
use serde_json::Value;

static NEXT: AtomicUsize = AtomicUsize::new(0);

pub struct Sandbox(pub PathBuf);

impl Sandbox {
    pub fn new() -> Sandbox {
        let n = NEXT.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("branchsight-test-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Sandbox(if cfg!(windows) { dir } else { dir.canonicalize().unwrap() })
    }

    pub fn path(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub struct Reply {
    pub status: u16,
    pub content_type: String,
    pub body: String,
}

pub fn git(dir: &Path, args: &[&str]) -> String {
    let out = git_command(dir, args).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

pub fn git_succeeds(dir: &Path, args: &[&str]) -> bool {
    git_command(dir, args).output().unwrap().status.success()
}

fn git_command(dir: &Path, args: &[&str]) -> Command {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=Test", "-c", "user.email=test@example.com", "-c", "commit.gpgsign=false"])
        .args(["-c", "tag.gpgsign=false", "-c", "init.defaultBranch=main", "-c", "core.hooksPath=/dev/null"])
        .args(args);
    command
}

pub fn init(dir: &Path) {
    fs::create_dir_all(dir).unwrap();
    git(dir, &["init", "-q"]);
}

pub fn commit(dir: &Path, file: &str, message: &str) -> String {
    if let Some(parent) = dir.join(file).parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(dir.join(file), format!("{message}\n")).unwrap();
    git(dir, &["add", file]);
    commit_staged(dir, message)
}

pub fn commit_all(dir: &Path, message: &str) -> String {
    git(dir, &["add", "-A"]);
    commit_staged(dir, message)
}

fn commit_staged(dir: &Path, message: &str) -> String {
    git(dir, &["commit", "-q", "-m", message]);
    git(dir, &["rev-parse", "HEAD"])
}

pub fn clone(sb: &Sandbox, origin: &Path, name: &str) -> PathBuf {
    let target = sb.path(name);
    git(&sb.0, &["clone", "-q", origin.to_str().unwrap(), target.to_str().unwrap()]);
    target
}

pub fn start(roots: Vec<PathBuf>, default_repo: Option<PathBuf>) -> u16 {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let port = server.server_addr().to_ip().unwrap().port();
    let config = Arc::new(Config { roots, default_repo });
    thread::spawn(move || http::serve(server, config));
    port
}

pub fn request(port: u16, method: &str, path: &str, headers: &[(&str, &str)]) -> Reply {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    let extra: String = headers.iter().map(|(k, v)| format!("{k}: {v}\r\n")).collect();
    let has_host = headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("host"));
    let host = if has_host { String::new() } else { format!("Host: localhost:{port}\r\n") };
    write!(stream, "{method} {path} HTTP/1.0\r\n{host}{extra}Content-Length: 0\r\n\r\n").unwrap();
    let mut raw = String::new();
    stream.read_to_string(&mut raw).unwrap();
    let (head, body) = raw.split_once("\r\n\r\n").unwrap();
    let status = head.split(' ').nth(1).unwrap().parse().unwrap();
    let content_type = head
        .lines()
        .find_map(|l| l.strip_prefix("Content-Type: ").or_else(|| l.strip_prefix("content-type: ")))
        .unwrap_or("")
        .to_owned();
    Reply { status, content_type, body: body.to_owned() }
}

pub fn get(port: u16, path: &str) -> (u16, String) {
    let reply = request(port, "GET", path, &[(PAGE_HEADER, "1")]);
    (reply.status, reply.body)
}

pub fn post(port: u16, path: &str) -> (u16, String) {
    let reply = request(port, "POST", path, &[(PAGE_HEADER, "1")]);
    (reply.status, reply.body)
}

pub fn json(port: u16, path: &str) -> Value {
    let (status, body) = get(port, path);
    assert_eq!(status, 200, "{path}: {body}");
    serde_json::from_str(&body).unwrap()
}

pub fn repo_query(repo: &Path) -> String {
    format!("repo={}", encode(&repo.to_string_lossy()))
}

pub fn subjects(log: &Value) -> Vec<String> {
    log.as_array().unwrap().iter().map(|c| c["s"].as_str().unwrap().to_owned()).collect()
}

pub fn file<'a>(files: &'a Value, path: &str) -> &'a Value {
    files.as_array().unwrap().iter().find(|f| f["path"] == path).unwrap_or_else(|| panic!("{path} in {files}"))
}

pub fn paths(files: &Value) -> Vec<String> {
    files.as_array().unwrap().iter().map(|f| f["path"].as_str().unwrap().to_owned()).collect()
}
