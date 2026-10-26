use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use branchsight::git::Repo;
use branchsight::server::{self, Config};
use branchsight::url::encode;
use serde_json::Value;

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Sandbox(PathBuf);

impl Sandbox {
    fn new() -> Sandbox {
        let n = NEXT.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("branchsight-test-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Sandbox(if cfg!(windows) { dir } else { dir.canonicalize().unwrap() })
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=Test", "-c", "user.email=test@example.com", "-c", "commit.gpgsign=false"])
        .args(["-c", "tag.gpgsign=false", "-c", "init.defaultBranch=main", "-c", "core.hooksPath=/dev/null"])
        .args(args)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

fn init(dir: &Path) {
    fs::create_dir_all(dir).unwrap();
    git(dir, &["init", "-q"]);
}

fn commit(dir: &Path, file: &str, message: &str) -> String {
    fs::write(dir.join(file), format!("{message}\n")).unwrap();
    git(dir, &["add", file]);
    git(dir, &["commit", "-q", "-m", message]);
    git(dir, &["rev-parse", "HEAD"])
}

fn start(roots: Vec<PathBuf>, default_repo: Option<PathBuf>) -> u16 {
    server::spawn_first_free(std::iter::empty(), Config { roots, default_repo }).unwrap()
}

fn request(port: u16, method: &str, path: &str, host: &str) -> (u16, String, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(stream, "{method} {path} HTTP/1.0\r\nHost: {host}\r\nContent-Length: 0\r\n\r\n").unwrap();
    let mut raw = String::new();
    stream.read_to_string(&mut raw).unwrap();
    let (head, body) = raw.split_once("\r\n\r\n").unwrap();
    let status = head.split(' ').nth(1).unwrap().parse().unwrap();
    let ctype = head
        .lines()
        .find_map(|l| l.strip_prefix("Content-Type: ").or_else(|| l.strip_prefix("content-type: ")))
        .unwrap_or("")
        .to_owned();
    (status, ctype, body.to_owned())
}

fn get(port: u16, path: &str) -> (u16, String) {
    let (status, _, body) = request(port, "GET", path, &format!("localhost:{port}"));
    (status, body)
}

fn json(port: u16, path: &str) -> Value {
    let (status, body) = get(port, path);
    assert_eq!(status, 200, "{path}: {body}");
    serde_json::from_str(&body).unwrap()
}

fn repo_query(repo: &Path) -> String {
    format!("repo={}", encode(&repo.to_string_lossy()))
}

fn subjects(log: &Value) -> Vec<String> {
    log.as_array().unwrap().iter().map(|c| c["s"].as_str().unwrap().to_owned()).collect()
}

#[test]
fn log_shows_only_the_history_of_the_selected_refs() {
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    commit(&repo, "a", "A");
    git(&repo, &["branch", "feature"]);
    git(&repo, &["branch", "other"]);
    commit(&repo, "b", "B on main");
    git(&repo, &["checkout", "-q", "feature"]);
    commit(&repo, "c", "C on feature");
    git(&repo, &["checkout", "-q", "other"]);
    commit(&repo, "d", "D on other");
    let port = start(vec![sb.0.clone()], None);

    let log = json(port, &format!("/api/log?{}&ref=feature", repo_query(&repo)));
    assert_eq!(subjects(&log), ["C on feature", "A"]);

    let log = json(port, &format!("/api/log?{}&ref=feature&ref=main", repo_query(&repo)));
    let mut got = subjects(&log);
    got.sort();
    assert_eq!(got, ["A", "B on main", "C on feature"]);

    let parents = &log.as_array().unwrap().iter().find(|c| c["s"] == "C on feature").unwrap()["p"];
    assert_eq!(parents.as_array().unwrap().len(), 1);
}

#[test]
fn log_groups_select_every_local_or_remote_branch() {
    let sb = Sandbox::new();
    let origin = sb.path("origin");
    init(&origin);
    commit(&origin, "a", "A");
    git(&origin, &["checkout", "-q", "-b", "remote-only"]);
    commit(&origin, "r", "R remote only");
    git(&origin, &["checkout", "-q", "main"]);
    let clone = sb.path("clone");
    git(&sb.0, &["clone", "-q", origin.to_str().unwrap(), clone.to_str().unwrap()]);
    git(&clone, &["checkout", "-q", "-b", "local-only"]);
    commit(&clone, "l", "L local only");
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&clone);

    let both = subjects(&json(port, &format!("/api/log?{q}&ref=@local&ref=@remotes")));
    assert!(both.contains(&"R remote only".into()) && both.contains(&"L local only".into()), "{both:?}");

    let local = subjects(&json(port, &format!("/api/log?{q}&ref=@local")));
    assert!(local.contains(&"L local only".into()) && !local.contains(&"R remote only".into()), "{local:?}");

    let (status, body) = get(port, &format!("/api/log?{q}&ref=@everything"));
    assert_eq!(status, 400, "{body}");
}

#[test]
fn log_and_commit_reject_option_injection() {
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    commit(&repo, "a", "A");
    let port = start(vec![sb.0.clone()], None);
    let target = sb.path("pwned");
    let q = repo_query(&repo);

    let (status, _) = get(port, &format!("/api/log?{q}&ref={}", encode(&format!("--output={}", target.display()))));
    assert_eq!(status, 400);
    assert!(!target.exists());

    for hash in ["--help", "HEAD", "abc", "zzzzzzz"] {
        let (status, _) = get(port, &format!("/api/commit?{q}&h={}", encode(hash)));
        assert_eq!(status, 400, "{hash}");
    }
}

#[test]
fn refs_list_kinds_and_peel_annotated_tags() {
    let sb = Sandbox::new();
    let origin = sb.path("origin");
    init(&origin);
    let a = commit(&origin, "a", "A");
    let clone = sb.path("clone");
    git(&sb.0, &["clone", "-q", origin.to_str().unwrap(), clone.to_str().unwrap()]);
    git(&clone, &["tag", "-a", "v1.0.0", "-m", "release"]);
    let port = start(vec![sb.0.clone()], None);

    let refs = json(port, &format!("/api/refs?{}", repo_query(&clone)));
    let find = |name: &str| refs.as_array().unwrap().iter().find(|r| r["name"] == name).cloned();
    assert_eq!(find("main").unwrap()["kind"], "head");
    assert_eq!(find("origin/main").unwrap()["kind"], "remote");
    assert!(find("origin/HEAD").is_none() && find("origin").is_none());
    let tag = find("v1.0.0").unwrap();
    assert_eq!((tag["kind"].as_str(), tag["hash"].as_str()), (Some("tag"), Some(a.as_str())));
}

#[test]
fn commit_shows_files_and_diff_against_the_first_parent() {
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    commit(&repo, "a", "A");
    git(&repo, &["checkout", "-q", "-b", "topic"]);
    commit(&repo, "topic.txt", "topic work");
    git(&repo, &["checkout", "-q", "main"]);
    commit(&repo, "main.txt", "main work");
    git(&repo, &["merge", "-q", "--no-ff", "topic", "-m", "merge topic"]);
    let merge = git(&repo, &["rev-parse", "HEAD"]);
    let port = start(vec![sb.0.clone()], None);

    let (status, body) = get(port, &format!("/api/commit?{}&h={merge}", repo_query(&repo)));
    assert_eq!(status, 200);
    assert!(body.starts_with(&format!("commit {merge}")), "{body}");
    assert!(body.contains("diff --git a/topic.txt b/topic.txt"), "{body}");
    assert!(!body.contains("main.txt"), "a merge must diff against its first parent only: {body}");
}

#[test]
fn state_fingerprint_changes_when_history_moves() {
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    commit(&repo, "a", "A");
    let port = start(vec![sb.0.clone()], None);
    let path = format!("/api/state?{}", repo_query(&repo));

    let before = json(port, &path)["v"].clone();
    assert_eq!(json(port, &path)["v"], before, "fingerprint must be stable while nothing changes");
    commit(&repo, "b", "B");
    let after_commit = json(port, &path)["v"].clone();
    assert_ne!(after_commit, before);
    git(&repo, &["checkout", "-q", "-b", "elsewhere"]);
    assert_ne!(json(port, &path)["v"], after_commit, "switching branch must change the fingerprint");
}

#[test]
fn fetch_brings_new_remote_commits_and_requires_post() {
    let sb = Sandbox::new();
    let origin = sb.path("origin");
    init(&origin);
    commit(&origin, "a", "A");
    let clone = sb.path("clone");
    git(&sb.0, &["clone", "-q", origin.to_str().unwrap(), clone.to_str().unwrap()]);
    let newer = commit(&origin, "b", "B upstream");
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&clone);

    let (status, _) = get(port, &format!("/api/fetch?{q}"));
    assert_eq!(status, 400);

    let (status, _, body) = request(port, "POST", &format!("/api/fetch?{q}"), "localhost");
    assert_eq!(status, 200, "{body}");
    assert_eq!(git(&clone, &["rev-parse", "origin/main"]), newer);
}

#[test]
fn projects_lists_repositories_under_the_roots_and_skips_vendored_ones() {
    let sb = Sandbox::new();
    init(&sb.path("alpha"));
    init(&sb.path("group/beta"));
    init(&sb.path("web/node_modules/dep"));
    init(&sb.path("alpha/nested-inside-a-repo"));
    fs::create_dir_all(sb.path("not-a-repo")).unwrap();
    let port = start(vec![sb.0.clone()], None);

    let projects = json(port, "/api/projects");
    let mut names: Vec<&str> = projects.as_array().unwrap().iter().map(|p| p["name"].as_str().unwrap()).collect();
    names.sort();
    assert_eq!(names, ["alpha", "beta"]);
}

#[test]
fn repository_resolution_reports_what_is_missing() {
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    commit(&repo, "a", "A");
    fs::create_dir_all(sb.path("plain")).unwrap();

    let without_default = start(vec![sb.0.clone()], None);
    let (status, body) = get(without_default, "/api/repo");
    assert_eq!((status, body.as_str()), (400, "no repository selected"));
    let (status, body) = get(without_default, &format!("/api/repo?{}", repo_query(&sb.path("plain"))));
    assert_eq!(status, 400);
    assert!(body.starts_with("not a git repository"), "{body}");

    let with_default = start(vec![sb.0.clone()], Some(repo.clone()));
    let info = json(with_default, "/api/repo");
    assert_eq!(info["path"], Repo::open(&repo).unwrap().root.to_string_lossy().as_ref());
    assert_eq!(info["branch"], "main");
}

#[test]
fn only_loopback_host_names_are_served() {
    let sb = Sandbox::new();
    let port = start(vec![sb.0.clone()], None);
    for host in ["evil.example", "localhost.evil.example", "192.168.1.10"] {
        let (status, _, _) = request(port, "GET", "/api/projects", host);
        assert_eq!(status, 403, "{host}");
    }
    let (status, _, _) = request(port, "GET", "/api/projects", "my-project.localhost:7420");
    assert_eq!(status, 200);
}

#[test]
fn page_and_every_asset_it_references_are_served() {
    let sb = Sandbox::new();
    let port = start(vec![sb.0.clone()], None);
    let (status, ctype, index) = request(port, "GET", "/", "localhost");
    assert_eq!(status, 200);
    assert!(ctype.starts_with("text/html"), "{ctype}");

    let mut assets: Vec<String> = ["src=\"/", "href=\"/"]
        .iter()
        .flat_map(|attr| index.match_indices(attr).map(|(i, m)| index[i + m.len() - 1..].split('"').next().unwrap().to_owned()))
        .collect();
    let (_, _, app) = request(port, "GET", "/app.js", "localhost");
    assets.extend(app.match_indices("from './").map(|(i, m)| format!("/{}", app[i + m.len()..].split('\'').next().unwrap())));
    assert!(assets.len() >= 4, "{assets:?}");

    for asset in &assets {
        let (status, ctype, body) = request(port, "GET", asset, "localhost");
        assert_eq!(status, 200, "{asset}");
        assert!(!body.is_empty() && !ctype.is_empty(), "{asset}");
    }
}

fn post(port: u16, path: &str) -> (u16, String) {
    let (status, _, body) = request(port, "POST", path, "localhost");
    (status, body)
}

fn cloned_with_feature(sb: &Sandbox) -> (PathBuf, PathBuf) {
    let origin = sb.path("origin");
    init(&origin);
    commit(&origin, "a", "A");
    git(&origin, &["checkout", "-q", "-b", "feature"]);
    commit(&origin, "f", "F");
    git(&origin, &["checkout", "-q", "main"]);
    let clone = sb.path("clone");
    git(&sb.0, &["clone", "-q", origin.to_str().unwrap(), clone.to_str().unwrap()]);
    (origin, clone)
}

#[test]
fn checkout_switches_to_a_local_branch_and_tracks_a_remote_one() {
    // Given.
    let sb = Sandbox::new();
    let (_, clone) = cloned_with_feature(&sb);
    git(&clone, &["branch", "local-only"]);
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&clone);

    // When.
    let (status, body) = post(port, &format!("/api/checkout?{q}&ref=local-only"));

    // Then.
    assert_eq!(status, 200, "{body}");
    assert_eq!(git(&clone, &["branch", "--show-current"]), "local-only");

    // When.
    let (status, body) = post(port, &format!("/api/checkout?{q}&ref={}", encode("origin/feature")));

    // Then.
    assert_eq!(status, 200, "{body}");
    assert_eq!(git(&clone, &["branch", "--show-current"]), "feature");
    assert_eq!(git(&clone, &["rev-parse", "--abbrev-ref", "feature@{upstream}"]), "origin/feature");
}

#[test]
fn checkout_refuses_unknown_and_option_like_names_and_requires_post() {
    // Given.
    let sb = Sandbox::new();
    let (_, clone) = cloned_with_feature(&sb);
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&clone);

    // When.
    let refused: Vec<u16> = ["nope", "--orphan=x", "-", "origin/-f", "../main"]
        .iter()
        .map(|name| post(port, &format!("/api/checkout?{q}&ref={}", encode(name))).0)
        .collect();
    let (get_status, _) = get(port, &format!("/api/checkout?{q}&ref=main"));

    // Then.
    assert!(refused.iter().all(|s| *s == 400), "{refused:?}");
    assert_eq!(get_status, 400);
    assert_eq!(git(&clone, &["branch", "--show-current"]), "main");
}

#[test]
fn delete_remote_branch_removes_it_on_the_remote_only() {
    // Given.
    let sb = Sandbox::new();
    let (origin, clone) = cloned_with_feature(&sb);
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&clone);

    // When.
    let (get_status, _) = get(port, &format!("/api/delete-remote?{q}&ref={}", encode("origin/feature")));
    let (status, body) = post(port, &format!("/api/delete-remote?{q}&ref={}", encode("origin/feature")));

    // Then.
    assert_eq!(get_status, 400);
    assert_eq!(status, 200, "{body}");
    assert_eq!(git(&origin, &["branch", "--list", "feature"]), "");
    assert_eq!(git(&origin, &["branch", "--list", "main"]), "* main");
    let (status, _) = post(port, &format!("/api/delete-remote?{q}&ref=main"));
    assert_eq!(status, 400, "a local branch is not a remote one");
}

#[test]
fn checkout_detaches_head_on_a_tag_or_a_commit() {
    // Given.
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    let first = commit(&repo, "a", "A");
    git(&repo, &["tag", "-a", "v1.0.0", "-m", "release"]);
    let second = commit(&repo, "b", "B");
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&repo);

    // When.
    let (status, body) = post(port, &format!("/api/checkout?{q}&ref=v1.0.0"));

    // Then.
    assert_eq!(status, 200, "{body}");
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), first);
    assert_eq!(git(&repo, &["branch", "--show-current"]), "", "HEAD is detached");

    // When.
    let (status, body) = post(port, &format!("/api/checkout?{q}&ref={}", &second[..12]));

    // Then.
    assert_eq!(status, 200, "{body}");
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), second);
    let (status, _) = post(port, &format!("/api/checkout?{q}&ref=deadbeefdeadbeef"));
    assert_eq!(status, 400, "an unknown hash is refused");
}

#[test]
fn status_and_worktree_split_staged_unstaged_and_untracked_changes() {
    // Given.
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    commit(&repo, "staged.txt", "one");
    commit(&repo, "edited.txt", "two");
    fs::write(repo.join("staged.txt"), "one, staged\n").unwrap();
    git(&repo, &["add", "staged.txt"]);
    fs::write(repo.join("edited.txt"), "two, not staged\n").unwrap();
    fs::write(repo.join("new.txt"), "brand new\n").unwrap();
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&repo);

    // When.
    let status = json(port, &format!("/api/status?{q}"));
    let worktree = json(port, &format!("/api/worktree?{q}"));

    // Then.
    assert_eq!((status["staged"].as_u64(), status["unstaged"].as_u64(), status["untracked"].as_u64()), (Some(1), Some(1), Some(1)));
    let (staged, unstaged) = (worktree["staged"].as_str().unwrap(), worktree["unstaged"].as_str().unwrap());
    assert!(staged.contains("+one, staged") && !staged.contains("edited.txt"), "{staged}");
    assert!(unstaged.contains("+two, not staged") && !unstaged.contains("staged.txt |"), "{unstaged}");
    assert_eq!(worktree["untracked"], serde_json::json!(["new.txt"]));
}

#[test]
fn state_fingerprint_changes_when_a_file_is_edited() {
    // Given.
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    commit(&repo, "a.txt", "A");
    let port = start(vec![sb.0.clone()], None);
    let path = format!("/api/state?{}", repo_query(&repo));
    let clean = json(port, &path)["v"].clone();

    // When.
    fs::write(repo.join("a.txt"), "first edit\n").unwrap();
    let first_edit = json(port, &path)["v"].clone();
    fs::write(repo.join("a.txt"), "first edit\nsecond edit\n").unwrap();
    let second_edit = json(port, &path)["v"].clone();

    // Then.
    assert_ne!(first_edit, clean);
    assert_ne!(second_edit, first_edit, "a further edit of an already modified file must be seen too");
}

#[test]
fn compare_shows_every_change_between_two_commits() {
    // Given.
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    let from = commit(&repo, "a.txt", "A");
    commit(&repo, "b.txt", "B");
    let to = commit(&repo, "c.txt", "C");
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&repo);

    // When.
    let (status, body) = get(port, &format!("/api/compare?{q}&from={from}&to={to}"));

    // Then.
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("diff --git a/b.txt b/b.txt") && body.contains("diff --git a/c.txt b/c.txt"), "{body}");
    assert!(!body.contains("a/a.txt"), "the starting commit's own change is not part of the comparison");
    for bad in ["--output=x", "HEAD", "deadbeefdeadbeef"] {
        let (status, _) = get(port, &format!("/api/compare?{q}&from={from}&to={}", encode(bad)));
        assert_eq!(status, 400, "{bad}");
    }
}

#[test]
fn file_diff_returns_the_whole_file_for_each_source() {
    // Given.
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    let lines: String = (1..=30).map(|n| format!("line {n}\n")).collect();
    fs::write(repo.join("long.txt"), &lines).unwrap();
    git(&repo, &["add", "long.txt"]);
    git(&repo, &["commit", "-q", "-m", "long file"]);
    let first = git(&repo, &["rev-parse", "HEAD"]);
    fs::write(repo.join("long.txt"), lines.replace("line 15\n", "line fifteen\n")).unwrap();
    git(&repo, &["commit", "-q", "-am", "rename line 15"]);
    let second = git(&repo, &["rev-parse", "HEAD"]);
    fs::write(repo.join("long.txt"), lines.replace("line 2\n", "line two\n")).unwrap();
    fs::write(repo.join("fresh.txt"), "hello\n").unwrap();
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&repo);
    let diff = |extra: &str| get(port, &format!("/api/filediff?{q}&{extra}"));

    // When.
    let (commit_status, commit) = diff(&format!("kind=commit&h={second}&path=long.txt"));
    let (_, root) = diff(&format!("kind=commit&h={first}&path=long.txt"));
    let (_, compare) = diff(&format!("kind=compare&from={first}&to={second}&path=long.txt"));
    let (_, unstaged) = diff("kind=unstaged&path=long.txt");
    let (untracked_status, untracked) = diff("kind=untracked&path=fresh.txt");

    // Then.
    assert_eq!(commit_status, 200, "{commit}");
    assert!(commit.contains(" line 1\n") && commit.contains(" line 30") && commit.contains("+line fifteen"), "whole file, not 3 lines of context: {commit}");
    assert!(root.contains("+line 1\n") && root.contains("+line 30"), "a first commit diffs against nothing: {root}");
    assert_eq!(compare, commit);
    assert!(unstaged.contains("-line 2\n") && unstaged.contains("+line two") && unstaged.contains(" line 29"), "{unstaged}");
    assert_eq!(untracked_status, 200, "{untracked}");
    assert!(untracked.contains("+hello"), "{untracked}");
}

#[test]
fn file_diff_refuses_unknown_sources_and_files() {
    // Given.
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    let head = commit(&repo, "tracked.txt", "A");
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&repo);
    let status = |extra: &str| get(port, &format!("/api/filediff?{q}&{extra}")).0;

    // When.
    let refused = [
        status("kind=everything&path=tracked.txt"),
        status(&format!("kind=commit&h={head}")),
        status("kind=commit&h=HEAD&path=tracked.txt"),
        status("kind=untracked&path=tracked.txt"),
        status(&format!("kind=untracked&path={}", encode("../outside.txt"))),
    ];

    // Then.
    assert_eq!(refused, [400; 5]);
    let (ok, body) = get(port, &format!("/api/filediff?{q}&kind=commit&h={head}&path={}", encode("--output=x")));
    assert_eq!(ok, 200, "an option-like path is only a path: {body}");
    assert!(!repo.join("x").exists());
}
