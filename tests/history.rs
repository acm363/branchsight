mod common;

use std::fs;

use branchsight::url::encode;
use common::*;

#[test]
fn log_shows_only_the_history_of_the_selected_refs() {
    // Given.
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
    let q = repo_query(&repo);

    // When.
    let feature = json(port, &format!("/api/log?{q}&ref=feature"));
    let both = json(port, &format!("/api/log?{q}&ref=feature&ref=main"));

    // Then.
    assert_eq!(subjects(&feature), ["C on feature", "A"]);
    let mut got = subjects(&both);
    got.sort();
    assert_eq!(got, ["A", "B on main", "C on feature"]);
    let tip = both.as_array().unwrap().iter().find(|c| c["s"] == "C on feature").unwrap();
    assert_eq!(tip["p"].as_array().unwrap().len(), 1);
}

#[test]
fn log_groups_select_every_local_or_remote_branch() {
    // Given.
    let sb = Sandbox::new();
    let origin = sb.path("origin");
    init(&origin);
    commit(&origin, "a", "A");
    git(&origin, &["checkout", "-q", "-b", "remote-only"]);
    commit(&origin, "r", "R remote only");
    git(&origin, &["checkout", "-q", "main"]);
    let copy = clone(&sb, &origin, "clone");
    git(&copy, &["checkout", "-q", "-b", "local-only"]);
    commit(&copy, "l", "L local only");
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&copy);

    // When.
    let both = subjects(&json(port, &format!("/api/log?{q}&ref=@local&ref=@remotes")));
    let local = subjects(&json(port, &format!("/api/log?{q}&ref=@local")));
    let (unknown, body) = get(port, &format!("/api/log?{q}&ref=@everything"));

    // Then.
    assert!(both.contains(&"R remote only".into()) && both.contains(&"L local only".into()), "{both:?}");
    assert!(local.contains(&"L local only".into()) && !local.contains(&"R remote only".into()), "{local:?}");
    assert_eq!(unknown, 400, "{body}");
}

#[test]
fn log_and_commit_reject_option_injection() {
    // Given.
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    commit(&repo, "a", "A");
    let blob = git(&repo, &["rev-parse", "HEAD:a"]);
    let port = start(vec![sb.0.clone()], None);
    let target = sb.path("pwned");
    let q = repo_query(&repo);

    // When.
    let (log, _) = get(port, &format!("/api/log?{q}&ref={}", encode(&format!("--output={}", target.display()))));
    let commits: Vec<u16> = ["--help", "HEAD", "abc", "zzzzzzz", &blob].iter().map(|h| get(port, &format!("/api/commit?{q}&h={}", encode(h))).0).collect();

    // Then.
    assert_eq!(log, 400);
    assert!(!target.exists());
    assert_eq!(commits, [400; 5], "a blob id is not a commit either");
}

#[test]
fn refs_split_remote_names_and_skip_symbolic_heads() {
    // Given.
    let sb = Sandbox::new();
    let origin = sb.path("origin");
    init(&origin);
    let a = commit(&origin, "a", "A");
    git(&origin, &["branch", "feature/x"]);
    let copy = clone(&sb, &origin, "clone");
    git(&copy, &["remote", "add", "team/origin", origin.to_str().unwrap()]);
    git(&copy, &["fetch", "-q", "team/origin"]);
    git(&copy, &["tag", "-a", "v1.0.0", "-m", "release"]);
    let port = start(vec![sb.0.clone()], None);

    // When.
    let refs = json(port, &format!("/api/refs?{}", repo_query(&copy)));

    // Then.
    let find = |name: &str| refs.as_array().unwrap().iter().find(|r| r["name"] == name).cloned();
    assert_eq!(find("main").unwrap()["kind"], "head");
    let nested = find("team/origin/feature/x").unwrap();
    assert_eq!((nested["remote"].as_str(), nested["branch"].as_str()), (Some("team/origin"), Some("feature/x")));
    assert_eq!(find("origin/main").unwrap()["branch"], "main");
    assert!(find("origin/HEAD").is_none() && find("origin").is_none());
    let tag = find("v1.0.0").unwrap();
    assert_eq!((tag["kind"].as_str(), tag["hash"].as_str()), (Some("tag"), Some(a.as_str())));
}

#[test]
fn state_keeps_history_and_worktree_changes_apart() {
    // Given.
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    commit(&repo, "a.txt", "limit = 10");
    let port = start(vec![sb.0.clone()], None);
    let state = || json(port, &format!("/api/state?{}", repo_query(&repo)));
    let clean = state();

    // When.
    let unchanged = state();
    fs::write(repo.join("a.txt"), "limit = 20\n").unwrap();
    let edited = state();
    fs::write(repo.join("a.txt"), "limit = 30\n").unwrap();
    let same_counts = state();
    git(&repo, &["add", "a.txt"]);
    let staged = state();
    git(&repo, &["commit", "-q", "-m", "B"]);
    let committed = state();

    // Then.
    assert_eq!(unchanged, clean, "nothing changed, nothing moves");
    assert_eq!(edited["history"], clean["history"]);
    assert_ne!(edited["worktree"], clean["worktree"]);
    assert_eq!(edited["status"]["unstaged"], 1);
    assert_ne!(same_counts["worktree"], edited["worktree"], "an edit with the same line counts is still seen");
    assert_ne!(staged["worktree"], same_counts["worktree"]);
    assert_eq!(staged["status"]["staged"], 1);
    assert_ne!(committed["history"], staged["history"]);
}

#[test]
fn status_counts_survive_renames_of_short_names() {
    // Given.
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    commit(&repo, "a", "A");
    git(&repo, &["mv", "a", "renamed"]);
    fs::write(repo.join("new.txt"), "brand new\n").unwrap();
    let port = start(vec![sb.0.clone()], None);

    // When.
    let status = json(port, &format!("/api/status?{}", repo_query(&repo)));

    // Then.
    assert_eq!((status["staged"].as_u64(), status["unstaged"].as_u64(), status["untracked"].as_u64()), (Some(1), Some(0), Some(1)));
}

#[test]
fn user_settings_do_not_change_what_is_parsed() {
    // Given.
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    commit(&repo, "README.md", "first");
    let second = commit(&repo, "README.md", "second");
    for setting in ["diff.noprefix=true", "color.ui=always", "color.diff=always", "log.showSignature=true"] {
        let (key, value) = setting.split_once('=').unwrap();
        git(&repo, &["config", key, value]);
    }
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&repo);

    // When.
    let log = json(port, &format!("/api/log?{q}"));
    let shown = json(port, &format!("/api/commit?{q}&h={second}"));

    // Then.
    assert_eq!(log[0]["h"], second.as_str());
    assert_eq!(paths(&shown["files"]), ["README.md"]);
    let patch = shown["patch"].as_str().unwrap();
    assert!(patch.starts_with("diff --git a/README.md b/README.md") && !patch.contains('\x1b'), "{patch}");
}
