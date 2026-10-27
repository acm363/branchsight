mod common;

use std::fs;

use branchsight::url::encode;
use common::*;
use serde_json::{Value, json};

fn patch_of(changes: &Value, path: &str) -> String {
    file(&changes["files"], path)["patch"].as_str().unwrap().to_owned()
}

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
    let head = commit_all(&repo, "reshuffle");
    let port = start(vec![sb.0.clone()], None);

    // When.
    let shown = json(port, &format!("/api/commit?{}&h={head}", repo_query(&repo)));

    // Then.
    let files = &shown["files"];
    assert_eq!(file(files, "kept.txt")["status"], "M");
    assert_eq!((file(files, "kept.txt")["add"].as_u64(), file(files, "kept.txt")["del"].as_u64()), (Some(1), Some(0)));
    assert!(patch_of(&shown, "kept.txt").contains("+more"));
    assert_eq!(file(files, "gone.txt")["status"], "D");
    assert!(patch_of(&shown, "gone.txt").contains("-deleted later"));
    assert_eq!(file(files, "new \"name\".txt")["status"], "R");
    assert_eq!(file(files, "new \"name\".txt")["from"], "old name.txt");
    assert_eq!((file(files, "logo.png")["status"].as_str(), file(files, "logo.png")["add"].is_null()), (Some("A"), true), "binary files have no line counts");
    assert!(shown["header"].as_str().unwrap().starts_with(&format!("commit {head}")));
    assert_eq!(shown["truncated"], false);
}

#[cfg(unix)]
#[test]
fn a_file_that_becomes_a_symlink_keeps_its_own_diff() {
    // Given.
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    commit(&repo, "a.txt", "a");
    commit(&repo, "link", "plain file");
    commit(&repo, "z.txt", "z");
    fs::remove_file(repo.join("link")).unwrap();
    std::os::unix::fs::symlink("a.txt", repo.join("link")).unwrap();
    fs::write(repo.join("z.txt"), "z changed\n").unwrap();
    let head = commit_all(&repo, "typechange");
    let port = start(vec![sb.0.clone()], None);

    // When.
    let shown = json(port, &format!("/api/commit?{}&h={head}", repo_query(&repo)));

    // Then.
    assert_eq!(file(&shown["files"], "link")["status"], "T");
    assert!(patch_of(&shown, "link").contains("-plain file") && patch_of(&shown, "link").contains("+a.txt"));
    let z = patch_of(&shown, "z.txt");
    assert!(z.contains("+z changed") && !z.contains("plain file"), "{z}");
}

#[test]
fn a_merge_conflict_keeps_each_file_with_its_own_diff() {
    // Given.
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    commit(&repo, "a.txt", "base");
    git(&repo, &["checkout", "-q", "-b", "side"]);
    commit(&repo, "a.txt", "side");
    commit(&repo, "m.txt", "merged cleanly");
    git(&repo, &["checkout", "-q", "main"]);
    commit(&repo, "a.txt", "main");
    assert!(!git_succeeds(&repo, &["merge", "-q", "side"]), "the merge stops on the conflict");
    let port = start(vec![sb.0.clone()], None);

    // When.
    let worktree = json(port, &format!("/api/worktree?{}", repo_query(&repo)));

    // Then.
    let staged = &worktree["staged"];
    assert_eq!(file(&staged["files"], "a.txt")["status"], "U");
    assert!(!patch_of(staged, "a.txt").contains("merged cleanly"));
    assert!(patch_of(staged, "m.txt").contains("+merged cleanly"));
    assert_eq!(paths(&worktree["unstaged"]["files"]), ["a.txt"], "a conflicted file is listed once");
    assert!(patch_of(&worktree["unstaged"], "a.txt").contains("<<<<<<<"));
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
    assert!(patch_of(&shown, "topic.txt").contains("+topic work"));
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
    let state = json(port, &format!("/api/state?{q}"));
    let worktree = json(port, &format!("/api/worktree?{q}"));

    // Then.
    assert_eq!(state["status"], json!({ "staged": 1, "unstaged": 1, "untracked": 1 }));
    assert_eq!(paths(&worktree["staged"]["files"]), ["staged.txt"]);
    assert!(patch_of(&worktree["staged"], "staged.txt").contains("+one, staged"));
    assert_eq!(paths(&worktree["unstaged"]["files"]), ["edited.txt"]);
    assert!(patch_of(&worktree["unstaged"], "edited.txt").contains("+two, not staged"));
    assert_eq!(worktree["untracked"], json!(["new.txt"]));
}

#[test]
fn a_large_patch_is_cut_but_its_file_list_and_counts_stay_complete() {
    // Given.
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    commit(&repo, "seed.txt", "seed");
    let big: String = (0..300_000).map(|n| format!("generated line {n}\n")).collect();
    fs::write(repo.join("big.lock"), big).unwrap();
    fs::write(repo.join("zz-small.txt"), "small\n").unwrap();
    let head = commit_all(&repo, "vendor");
    let port = start(vec![sb.0.clone()], None);

    // When.
    let shown = json(port, &format!("/api/commit?{}&h={head}", repo_query(&repo)));

    // Then.
    assert_eq!(shown["truncated"], true);
    assert!(patch_of(&shown, "big.lock").len() <= 4 * 1024 * 1024);
    assert_eq!(paths(&shown["files"]), ["big.lock", "zz-small.txt"]);
    assert_eq!(file(&shown["files"], "big.lock")["add"], 300_000);
    assert_eq!(file(&shown["files"], "zz-small.txt")["add"], 1, "a file past the cut still has its counts");
}

#[test]
fn file_diff_returns_the_whole_file_for_each_source() {
    // Given.
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    let lines: String = (1..=30).map(|n| format!("line {n}\n")).collect();
    fs::write(repo.join("long.txt"), &lines).unwrap();
    let first = commit_all(&repo, "long file");
    fs::write(repo.join("long.txt"), lines.replace("line 15\n", "line fifteen\n")).unwrap();
    let second = commit_all(&repo, "rename line 15");
    fs::write(repo.join("long.txt"), lines.replace("line 2\n", "line two\n")).unwrap();
    fs::write(repo.join("fresh.txt"), "hello\n").unwrap();
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&repo);
    let diff = |extra: &str| json(port, &format!("/api/filediff?{q}&{extra}"))["patch"].as_str().unwrap().to_owned();

    // When.
    let commit = diff(&format!("kind=commit&h={second}&path=long.txt"));
    let root = diff(&format!("kind=commit&h={first}&path=long.txt"));
    let compare = diff(&format!("kind=compare&from={first}&to={second}&path=long.txt"));
    let unstaged = diff("kind=unstaged&path=long.txt");
    let untracked = diff("kind=untracked&path=fresh.txt");

    // Then.
    assert!(commit.contains(" line 1\n") && commit.contains(" line 30") && commit.contains("+line fifteen"), "{commit}");
    assert!(root.contains("+line 1\n") && root.contains("+line 30"), "a first commit diffs against nothing: {root}");
    assert_eq!(compare, commit);
    assert!(unstaged.contains("-line 2\n") && unstaged.contains("+line two") && unstaged.contains(" line 29"), "{unstaged}");
    assert!(untracked.contains("+hello"), "{untracked}");
}

#[test]
fn user_settings_do_not_hide_lines_or_the_first_commit() {
    // Given.
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    fs::write(repo.join("a.txt"), "one\n\nthree\n").unwrap();
    let root = commit_all(&repo, "first");
    fs::write(repo.join("a.txt"), "one\n\nTHREE\n").unwrap();
    git(&repo, &["config", "log.showRoot", "false"]);
    git(&repo, &["config", "diff.suppressBlankEmpty", "true"]);
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&repo);

    // When.
    let first = json(port, &format!("/api/commit?{q}&h={root}"));
    let unstaged = json(port, &format!("/api/filediff?{q}&kind=unstaged&path=a.txt"));

    // Then.
    assert_eq!(paths(&first["files"]), ["a.txt"]);
    assert!(unstaged["patch"].as_str().unwrap().contains("\n \n"), "the blank context line keeps its leading space");
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
    let diff = json(port, &format!("/api/filediff?{}&kind=commit&h={root}&path=a.txt", repo_query(&repo)));

    // Then.
    assert_eq!(root.len(), 64);
    assert!(diff["patch"].as_str().unwrap().contains("+first"), "{diff}");
}

#[test]
fn file_diff_refuses_unknown_sources_and_files() {
    // Given.
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    let head = commit(&repo, "tracked.txt", "A");
    init(&repo.join("nested"));
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
        status(&format!("kind=untracked&path={}", encode("nested/"))),
    ];
    let (option_like, body) = get(port, &format!("/api/filediff?{q}&kind=commit&h={head}&path={}", encode("--output=x")));

    // Then.
    assert_eq!(refused, [400; 6]);
    assert_eq!(option_like, 200, "an option-like path is only a path: {body}");
    assert!(!repo.join("x").exists());
}
