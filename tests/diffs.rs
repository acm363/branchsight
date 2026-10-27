mod common;

use std::fs;

use branchsight::url::encode;
use common::*;
use serde_json::json;

#[test]
fn commit_lists_every_file_with_its_status_counts_and_rename_source() {
    // Given.
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    commit(&repo, "kept.txt", "kept");
    commit(&repo, "old name.txt", "renamed later");
    commit(&repo, "gone.txt", "deleted later");
    git(&repo, &["mv", "old name.txt", "new \"name\".txt"]);
    git(&repo, &["rm", "-q", "gone.txt"]);
    fs::write(repo.join("logo.png"), [0x89, b'P', b'N', b'G', 0, 1, 2]).unwrap();
    fs::write(repo.join("kept.txt"), "kept\nmore\n").unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "reshuffle"]);
    let head = git(&repo, &["rev-parse", "HEAD"]);
    let port = start(vec![sb.0.clone()], None);

    // When.
    let shown = json(port, &format!("/api/commit?{}&h={head}", repo_query(&repo)));

    // Then.
    let files = shown["files"].as_array().unwrap();
    let file = |path: &str| files.iter().find(|f| f["path"] == path).unwrap_or_else(|| panic!("{path} in {files:?}"));
    assert_eq!(file("kept.txt")["status"], "M");
    assert_eq!((file("kept.txt")["add"].as_u64(), file("kept.txt")["del"].as_u64()), (Some(1), Some(0)));
    assert_eq!(file("gone.txt")["status"], "D");
    assert_eq!(file("new \"name\".txt")["status"], "R");
    assert_eq!(file("new \"name\".txt")["from"], "old name.txt");
    assert_eq!((file("logo.png")["status"].as_str(), file("logo.png")["add"].is_null()), (Some("A"), true), "binary files have no line counts");
    assert!(shown["header"].as_str().unwrap().starts_with(&format!("commit {head}")));
    assert_eq!(shown["truncated"], false);
}

#[test]
fn a_merge_is_shown_against_its_first_parent() {
    // Given.
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

    // When.
    let shown = json(port, &format!("/api/commit?{}&h={merge}", repo_query(&repo)));

    // Then.
    assert_eq!(paths(&shown["files"]), ["topic.txt"]);
    assert!(shown["patch"].as_str().unwrap().contains("diff --git a/topic.txt b/topic.txt"));
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
    let compared = json(port, &format!("/api/compare?{q}&from={from}&to={to}"));
    let refused: Vec<u16> = ["--output=x", "HEAD", "deadbeefdeadbeef"].iter().map(|bad| get(port, &format!("/api/compare?{q}&from={from}&to={}", encode(bad))).0).collect();

    // Then.
    assert_eq!(paths(&compared["files"]), ["b.txt", "c.txt"], "the starting commit's own change is not part of it");
    assert_eq!(refused, [400; 3]);
}

#[test]
fn worktree_splits_staged_unstaged_and_untracked_changes() {
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
    assert_eq!(paths(&worktree["staged"]["files"]), ["staged.txt"]);
    assert!(worktree["staged"]["patch"].as_str().unwrap().contains("+one, staged"));
    assert_eq!(paths(&worktree["unstaged"]["files"]), ["edited.txt"]);
    assert!(worktree["unstaged"]["patch"].as_str().unwrap().contains("+two, not staged"));
    assert_eq!(worktree["untracked"], json!(["new.txt"]));
}

#[test]
fn a_large_patch_is_cut_but_its_file_list_stays_complete() {
    // Given.
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    commit(&repo, "seed.txt", "seed");
    let big: String = (0..300_000).map(|n| format!("generated line {n}\n")).collect();
    fs::write(repo.join("big.lock"), big).unwrap();
    fs::write(repo.join("zz-small.txt"), "small\n").unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "vendor"]);
    let head = git(&repo, &["rev-parse", "HEAD"]);
    let port = start(vec![sb.0.clone()], None);

    // When.
    let shown = json(port, &format!("/api/commit?{}&h={head}", repo_query(&repo)));

    // Then.
    assert_eq!(shown["truncated"], true);
    assert!(shown["patch"].as_str().unwrap().len() <= 4 * 1024 * 1024);
    assert_eq!(paths(&shown["files"]), ["big.lock", "zz-small.txt"]);
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
    assert!(commit.contains(" line 1\n") && commit.contains(" line 30") && commit.contains("+line fifteen"), "{commit}");
    assert!(root.contains("+line 1\n") && root.contains("+line 30"), "a first commit diffs against nothing: {root}");
    assert!(compare.contains("+line fifteen") && compare.contains(" line 30"), "{compare}");
    assert!(unstaged.contains("-line 2\n") && unstaged.contains("+line two") && unstaged.contains(" line 29"), "{unstaged}");
    assert_eq!(untracked_status, 200, "{untracked}");
    assert!(untracked.contains("+hello"), "{untracked}");
}

#[test]
fn file_diff_of_a_first_commit_works_with_sha256_ids() {
    // Given.
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q", "--object-format=sha256"]);
    let root = commit(&repo, "a.txt", "first");
    let port = start(vec![sb.0.clone()], None);

    // When.
    let (status, body) = get(port, &format!("/api/filediff?{}&kind=commit&h={root}&path=a.txt", repo_query(&repo)));

    // Then.
    assert_eq!(root.len(), 64);
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("+first"), "{body}");
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
    let (option_like, body) = get(port, &format!("/api/filediff?{q}&kind=commit&h={head}&path={}", encode("--output=x")));

    // Then.
    assert_eq!(refused, [400; 5]);
    assert_eq!(option_like, 200, "an option-like path is only a path: {body}");
    assert!(!repo.join("x").exists());
}
