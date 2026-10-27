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
    let feature = json(port, &format!("/api/commits?{q}&ref=feature"));
    let both = json(port, &format!("/api/commits?{q}&ref=feature&ref=main"));

    // Then.
    assert_eq!(subjects(&feature), ["C on feature", "A"]);
    let mut got = subjects(&both);
    got.sort();
    assert_eq!(got, ["A", "B on main", "C on feature"]);
    let tip = both["commits"].as_array().unwrap().iter().find(|c| c["s"] == "C on feature").unwrap();
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
    let both = subjects(&json(port, &format!("/api/commits?{q}&ref=@local&ref=@remotes")));
    let local = subjects(&json(port, &format!("/api/commits?{q}&ref=@local")));
    let (unknown, body) = get(port, &format!("/api/commits?{q}&ref=@nothing-like-this"));

    // Then.
    assert!(both.contains(&"R remote only".into()) && both.contains(&"L local only".into()), "{both:?}");
    assert!(local.contains(&"L local only".into()) && !local.contains(&"R remote only".into()), "{local:?}");
    assert_eq!(unknown, 400, "{body}");
}

#[test]
fn a_branch_named_like_a_group_is_still_a_branch() {
    // Given.
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    commit(&repo, "a", "A");
    git(&repo, &["checkout", "-q", "-b", "@wip"]);
    commit(&repo, "w", "work in progress");
    let port = start(vec![sb.0.clone()], None);

    // When.
    let log = json(port, &format!("/api/commits?{}&ref={}", repo_query(&repo), encode("@wip")));

    // Then.
    assert_eq!(subjects(&log), ["work in progress", "A"]);
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
    let (log, _) = get(port, &format!("/api/commits?{q}&ref={}", encode(&format!("--output={}", target.display()))));
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
    assert_eq!((find("main").unwrap()["upstream"].as_str(), find("main").unwrap()["upstreamRemote"].as_str()), (Some("origin/main"), Some("origin")));
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
    git(&repo, &["checkout", "-q", "-b", "elsewhere"]);
    let switched = state();
    fs::create_dir_all(repo.join("new/deep")).unwrap();
    fs::write(repo.join("new/one.txt"), "one\n").unwrap();
    let untracked = state();
    fs::write(repo.join("new/deep/two.txt"), "two\n").unwrap();
    let deeper = state();

    // Then.
    assert_eq!(unchanged, clean, "nothing changed, nothing moves");
    assert_eq!(edited["history"], clean["history"]);
    assert_ne!(edited["worktree"], clean["worktree"]);
    assert_eq!(edited["status"]["unstaged"], 1);
    assert_ne!(same_counts["worktree"], edited["worktree"], "an edit with the same line counts is still seen");
    assert_ne!(staged["worktree"], same_counts["worktree"]);
    assert_eq!(staged["status"]["staged"], 1);
    assert_ne!(committed["history"], staged["history"]);
    assert_ne!(switched["history"], committed["history"], "a branch switch without a commit moves the current branch");
    assert_eq!(untracked["status"]["untracked"], 1);
    assert_ne!(deeper["worktree"], untracked["worktree"], "a file added deep inside an untracked folder is seen");
    assert_eq!(deeper["status"]["untracked"], 2);
}

#[test]
fn deepening_a_shallow_clone_changes_the_history_fingerprint() {
    // Given.
    let sb = Sandbox::new();
    let origin = sb.path("origin");
    init(&origin);
    commit(&origin, "a", "A");
    commit(&origin, "b", "B");
    let copy = sb.path("shallow");
    let url = format!("file:///{}", origin.display().to_string().replace('\\', "/").trim_start_matches('/'));
    git(&sb.0, &["clone", "-q", "--depth", "1", &url, copy.to_str().unwrap()]);
    let port = start(vec![sb.0.clone()], None);
    let state = || json(port, &format!("/api/state?{}", repo_query(&copy)));
    let before = state();

    // When.
    git(&copy, &["fetch", "-q", "--unshallow"]);

    // Then.
    assert_ne!(state()["history"], before["history"]);
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
    let status = &json(port, &format!("/api/state?{}", repo_query(&repo)))["status"];

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
    for setting in ["diff.noprefix=true", "diff.srcPrefix=old/", "color.ui=always", "color.diff=always", "log.abbrevCommit=true"] {
        let (key, value) = setting.split_once('=').unwrap();
        git(&repo, &["config", key, value]);
    }
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&repo);

    // When.
    let log = json(port, &format!("/api/commits?{q}"));
    let shown = json(port, &format!("/api/commit?{q}&h={second}"));

    // Then.
    assert_eq!(log["commits"][0]["h"], second.as_str());
    assert_eq!(log["history"], json(port, &format!("/api/state?{q}"))["history"], "a log says which history it was read from");
    assert_eq!(paths(&shown["files"]), ["README.md"]);
    assert!(shown["header"].as_str().unwrap().starts_with(&format!("commit {second}")));
    let patch = file(&shown["files"], "README.md")["patch"].as_str().unwrap();
    assert!(patch.starts_with("diff --git a/README.md b/README.md") && !patch.contains('\x1b'), "{patch}");
}
