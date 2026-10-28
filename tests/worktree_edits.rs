mod common;

use std::fs;
use std::path::Path;

use branchsight::url::encode;
use common::*;
use serde_json::Value;

fn staged(wt: &Value) -> Vec<String> {
    paths(&wt["staged"]["files"])
}

fn unstaged(wt: &Value) -> Vec<String> {
    paths(&wt["unstaged"]["files"])
}

fn untracked(wt: &Value) -> Vec<String> {
    wt["untracked"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_owned()).collect()
}

fn worktree(port: u16, repo: &Path) -> Value {
    json(port, &format!("/api/worktree?{}", repo_query(repo)))
}

#[test]
fn staging_moves_a_file_between_not_staged_and_staged() {
    // Given.
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    commit(&repo, "a.txt", "one");
    fs::write(repo.join("a.txt"), "changed\n").unwrap();
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&repo);

    // When.
    let (staged_ok, body) = post(port, &format!("/api/stage?{q}&path=a.txt"));
    let after_stage = worktree(port, &repo);
    let (unstaged_ok, _) = post(port, &format!("/api/unstage?{q}&path=a.txt"));
    let after_unstage = worktree(port, &repo);

    // Then.
    assert_eq!(staged_ok, 200, "{body}");
    assert_eq!(unstaged_ok, 200);
    assert_eq!(staged(&after_stage), ["a.txt"]);
    assert!(unstaged(&after_stage).is_empty());
    assert_eq!(unstaged(&after_unstage), ["a.txt"]);
    assert!(staged(&after_unstage).is_empty());
}

#[test]
fn stage_all_stages_every_change_including_untracked() {
    // Given.
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    commit(&repo, "a.txt", "one");
    fs::write(repo.join("a.txt"), "changed\n").unwrap();
    fs::write(repo.join("b.txt"), "new\n").unwrap();
    let port = start(vec![sb.0.clone()], None);

    // When.
    let (status, body) = post(port, &format!("/api/stage?{}&all=1", repo_query(&repo)));
    let wt = worktree(port, &repo);

    // Then.
    assert_eq!(status, 200, "{body}");
    assert_eq!(staged(&wt), ["a.txt", "b.txt"]);
    assert!(unstaged(&wt).is_empty());
    assert!(untracked(&wt).is_empty());
}

#[test]
fn commit_records_the_staged_files_and_leaves_the_rest() {
    // Given.
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    commit(&repo, "a.txt", "one");
    fs::write(repo.join("a.txt"), "reworked\n").unwrap();
    fs::write(repo.join("b.txt"), "new\n").unwrap();
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&repo);

    // When.
    post(port, &format!("/api/stage?{q}&path=a.txt"));
    let (status, body) = post(port, &format!("/api/commit-staged?{q}&message={}", encode("second")));

    // Then.
    assert_eq!(status, 200, "{body}");
    assert_eq!(git(&repo, &["log", "--format=%s"]), "second\none");
    assert_eq!(git(&repo, &["show", "HEAD:a.txt"]), "reworked");
    assert_eq!(untracked(&worktree(port, &repo)), ["b.txt"]);
}

#[test]
fn commit_refuses_an_empty_message_or_an_empty_index() {
    // Given.
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    commit(&repo, "a.txt", "one");
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&repo);

    // When.
    let (nothing, nothing_body) = post(port, &format!("/api/commit-staged?{q}&message={}", encode("empty")));
    fs::write(repo.join("a.txt"), "two\n").unwrap();
    post(port, &format!("/api/stage?{q}&path=a.txt"));
    let (blank, blank_body) = post(port, &format!("/api/commit-staged?{q}&message="));

    // Then.
    assert_eq!(nothing, 400);
    assert!(nothing_body.contains("nothing is staged"), "{nothing_body}");
    assert_eq!(blank, 400);
    assert!(blank_body.contains("message is required"), "{blank_body}");
}

#[test]
fn the_first_commit_works_before_any_history_exists() {
    // Given.
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    fs::write(repo.join("a.txt"), "one\n").unwrap();
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&repo);

    // When.
    post(port, &format!("/api/stage?{q}&all=1"));
    let (status, body) = post(port, &format!("/api/commit-staged?{q}&message={}", encode("first")));

    // Then.
    assert_eq!(status, 200, "{body}");
    assert_eq!(git(&repo, &["log", "--format=%s"]), "first");
}

#[test]
fn amend_folds_staged_changes_into_the_last_commit_and_rewords_it() {
    // Given.
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    commit(&repo, "a.txt", "one");
    fs::write(repo.join("a.txt"), "amended\n").unwrap();
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&repo);

    // When.
    post(port, &format!("/api/stage?{q}&path=a.txt"));
    let (status, body) = post(port, &format!("/api/commit-staged?{q}&amend=1&message={}", encode("reworded")));

    // Then.
    assert_eq!(status, 200, "{body}");
    assert_eq!(git(&repo, &["log", "--format=%s"]), "reworded");
    assert_eq!(git(&repo, &["show", "HEAD:a.txt"]), "amended");
}

#[test]
fn amend_is_refused_before_any_history_exists() {
    // Given.
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    fs::write(repo.join("a.txt"), "one\n").unwrap();
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&repo);

    // When.
    post(port, &format!("/api/stage?{q}&all=1"));
    let (status, body) = post(port, &format!("/api/commit-staged?{q}&amend=1&message={}", encode("nope")));

    // Then.
    assert_eq!(status, 400);
    assert!(body.contains("no commit to amend"), "{body}");
}

#[test]
fn discard_restores_a_tracked_file_and_removes_an_untracked_one() {
    // Given.
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    commit(&repo, "a.txt", "one");
    fs::write(repo.join("a.txt"), "bad\n").unwrap();
    fs::write(repo.join("b.txt"), "junk\n").unwrap();
    let port = start(vec![sb.0.clone()], None);

    // When.
    let (status, body) = post(port, &format!("/api/discard?{}&path=a.txt&path=b.txt", repo_query(&repo)));

    // Then.
    assert_eq!(status, 200, "{body}");
    assert_eq!(fs::read_to_string(repo.join("a.txt")).unwrap(), "one\n");
    assert!(!repo.join("b.txt").exists());
}

#[test]
fn tags_are_created_lightweight_or_annotated_and_deleted() {
    // Given.
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    let head = commit(&repo, "a.txt", "one");
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&repo);

    // When.
    let (light, light_body) = post(port, &format!("/api/tag?{q}&name=v1&h={head}"));
    let (annotated, _) = post(port, &format!("/api/tag?{q}&name=v2&h={head}&message={}", encode("release one")));
    let (dup, dup_body) = post(port, &format!("/api/tag?{q}&name=v1&h={head}"));
    let (deleted, _) = post(port, &format!("/api/delete-tag?{q}&name=v1"));

    // Then.
    assert_eq!(light, 200, "{light_body}");
    assert_eq!(annotated, 200);
    assert_eq!(git(&repo, &["cat-file", "-t", "v2"]), "tag");
    assert_eq!(dup, 400);
    assert!(dup_body.contains("already exists"), "{dup_body}");
    assert_eq!(deleted, 200);
    assert!(!git_succeeds(&repo, &["rev-parse", "--verify", "-q", "refs/tags/v1"]));
}

#[test]
fn tag_and_branch_names_reject_option_injection_and_bad_names() {
    // Given.
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    let head = commit(&repo, "a.txt", "one");
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&repo);

    // When.
    let names = ["-d", "--contains", "a b", "a..b"];
    let tags: Vec<u16> = names.iter().map(|n| post(port, &format!("/api/tag?{q}&name={}&h={head}", encode(n))).0).collect();
    let branches: Vec<u16> = names.iter().map(|n| post(port, &format!("/api/branch?{q}&name={}&h={head}", encode(n))).0).collect();

    // Then.
    assert!(tags.iter().all(|&s| s == 400), "{tags:?}");
    assert!(branches.iter().all(|&s| s == 400), "{branches:?}");
    assert_eq!(git(&repo, &["tag", "-l"]), "");
}

#[test]
fn a_tag_is_pushed_to_and_deleted_from_a_remote() {
    // Given.
    let sb = Sandbox::new();
    let origin = sb.path("origin");
    init(&origin);
    commit(&origin, "a.txt", "one");
    let work = clone(&sb, &origin, "work");
    let head = git(&work, &["rev-parse", "HEAD"]);
    git(&work, &["tag", "v1", &head]);
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&work);

    // When.
    let (pushed, push_body) = post(port, &format!("/api/push-tag?{q}&name=v1&remote=origin"));
    let on_remote = git_succeeds(&origin, &["rev-parse", "--verify", "-q", "refs/tags/v1"]);
    let (removed, _) = post(port, &format!("/api/push-tag?{q}&name=v1&remote=origin&delete=1"));

    // Then.
    assert_eq!(pushed, 200, "{push_body}");
    assert!(on_remote);
    assert_eq!(removed, 200);
    assert!(!git_succeeds(&origin, &["rev-parse", "--verify", "-q", "refs/tags/v1"]));
}

#[test]
fn create_branch_makes_a_branch_at_a_commit_without_switching() {
    // Given.
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    let first = commit(&repo, "a.txt", "one");
    commit(&repo, "b.txt", "two");
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&repo);

    // When.
    let (status, body) = post(port, &format!("/api/branch?{q}&name=feature&h={first}"));
    let (dup, _) = post(port, &format!("/api/branch?{q}&name=feature&h={first}"));

    // Then.
    assert_eq!(status, 200, "{body}");
    assert_eq!(git(&repo, &["rev-parse", "feature"]), first);
    assert_eq!(git(&repo, &["branch", "--show-current"]), "main");
    assert_eq!(dup, 400);
}

#[test]
fn worktree_edits_require_post() {
    // Given.
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    commit(&repo, "a.txt", "one");
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&repo);

    // Then.
    for endpoint in ["stage", "unstage", "discard", "commit-staged", "tag", "delete-tag", "push-tag", "branch"] {
        assert_eq!(get(port, &format!("/api/{endpoint}?{q}")).0, 400, "{endpoint}");
    }
}
