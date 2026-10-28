mod common;

use std::fs;
use std::path::PathBuf;

use common::*;

fn with_side_branch(sb: &Sandbox) -> (PathBuf, String) {
    let repo = sb.path("repo");
    init(&repo);
    commit(&repo, "a.txt", "base");
    git(&repo, &["switch", "-q", "-c", "side"]);
    let side = commit(&repo, "b.txt", "side work");
    git(&repo, &["switch", "-q", "main"]);
    commit(&repo, "c.txt", "main work");
    (repo, side)
}

#[test]
fn cherry_pick_copies_a_commit_onto_the_current_branch() {
    // Given.
    let sb = Sandbox::new();
    let (repo, side) = with_side_branch(&sb);
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&repo);

    // When.
    let (as_get, _) = get(port, &format!("/api/cherry-pick?{q}&h={side}"));
    let (picked, body) = post(port, &format!("/api/cherry-pick?{q}&h={side}"));

    // Then.
    assert_eq!(as_get, 400);
    assert_eq!(picked, 200, "{body}");
    assert_eq!(git(&repo, &["log", "-1", "--format=%s"]), "side work");
    assert!(repo.join("b.txt").exists());
}

#[test]
fn cherry_pick_and_revert_can_stop_before_committing() {
    // Given.
    let sb = Sandbox::new();
    let (repo, side) = with_side_branch(&sb);
    let head = git(&repo, &["rev-parse", "HEAD"]);
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&repo);

    // When.
    let (picked, _) = post(port, &format!("/api/cherry-pick?{q}&h={side}&commit=0"));
    let staged = git(&repo, &["diff", "--cached", "--name-only"]);
    git(&repo, &["cherry-pick", "--quit"]);
    git(&repo, &["reset", "-q", "--hard"]);
    let (reverted, body) = post(port, &format!("/api/revert?{q}&h={head}&commit=0"));

    // Then.
    assert_eq!(picked, 200);
    assert_eq!(staged, "b.txt");
    assert_eq!(reverted, 200, "{body}");
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), head, "nothing is committed");
    assert_eq!(git(&repo, &["diff", "--cached", "--name-status"]), "D\tc.txt");
}

#[test]
fn revert_adds_a_commit_that_undoes_another() {
    // Given.
    let sb = Sandbox::new();
    let (repo, _) = with_side_branch(&sb);
    let head = git(&repo, &["rev-parse", "HEAD"]);
    let port = start(vec![sb.0.clone()], None);

    // When.
    let (status, body) = post(port, &format!("/api/revert?{}&h={head}", repo_query(&repo)));

    // Then.
    assert_eq!(status, 200, "{body}");
    assert_eq!(git(&repo, &["log", "-1", "--format=%s"]), "Revert \"main work\"");
    assert!(!repo.join("c.txt").exists());
}

#[test]
fn drop_removes_a_commit_and_keeps_the_ones_after_it() {
    // Given.
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    commit(&repo, "a.txt", "one");
    let doomed = commit(&repo, "b.txt", "two");
    commit(&repo, "c.txt", "three");
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&repo);

    // When.
    let (status, body) = post(port, &format!("/api/drop?{q}&h={doomed}"));
    let root = git(&repo, &["rev-list", "--max-parents=0", "HEAD"]);
    let (first, _) = post(port, &format!("/api/drop?{q}&h={root}"));

    // Then.
    assert_eq!(status, 200, "{body}");
    assert_eq!(git(&repo, &["log", "--format=%s"]), "three\none");
    assert!(!repo.join("b.txt").exists() && repo.join("c.txt").exists());
    assert_eq!(first, 400, "the first commit cannot be dropped");
}

#[test]
fn rebase_replays_the_current_branch_on_another() {
    // Given.
    let sb = Sandbox::new();
    let (repo, side) = with_side_branch(&sb);
    let port = start(vec![sb.0.clone()], None);

    // When.
    let (status, body) = post(port, &format!("/api/rebase?{}&ref=side", repo_query(&repo)));

    // Then.
    assert_eq!(status, 200, "{body}");
    assert_eq!(git(&repo, &["rev-parse", "HEAD~1"]), side);
    assert_eq!(git(&repo, &["log", "-1", "--format=%s"]), "main work");
}

#[test]
fn reset_moves_the_branch_and_keeps_or_discards_changes_by_mode() {
    // Given.
    let sb = Sandbox::new();
    let (repo, _) = with_side_branch(&sb);
    let base = git(&repo, &["rev-parse", "HEAD~1"]);
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&repo);

    // When.
    let (soft, _) = post(port, &format!("/api/reset?{q}&h={base}&mode=soft"));
    let staged_after_soft = git(&repo, &["diff", "--cached", "--name-only"]);
    git(&repo, &["commit", "-q", "-m", "main work again"]);
    let (hard, body) = post(port, &format!("/api/reset?{q}&h={base}&mode=hard"));
    let (unknown, _) = post(port, &format!("/api/reset?{q}&h={base}&mode=sideways"));

    // Then.
    assert_eq!(soft, 200);
    assert_eq!(staged_after_soft, "c.txt");
    assert_eq!(hard, 200, "{body}");
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), base);
    assert!(!repo.join("c.txt").exists());
    assert_eq!(unknown, 400);
}

#[test]
fn a_conflicting_cherry_pick_says_what_stopped_it() {
    // Given.
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    commit(&repo, "a.txt", "base");
    git(&repo, &["switch", "-q", "-c", "side"]);
    let theirs = commit(&repo, "a.txt", "theirs");
    git(&repo, &["switch", "-q", "main"]);
    commit(&repo, "a.txt", "ours");
    let port = start(vec![sb.0.clone()], None);

    // When.
    let (status, body) = post(port, &format!("/api/cherry-pick?{}&h={theirs}", repo_query(&repo)));

    // Then.
    assert_eq!(status, 400);
    assert!(body.contains("cherry-pick stopped on conflicts in a.txt") && body.contains("continue or abort"), "{body}");
    assert!(fs::read_to_string(repo.join("a.txt")).unwrap().contains("<<<<<<<"));
}

fn conflicting(sb: &Sandbox) -> (PathBuf, String) {
    let repo = sb.path("repo");
    init(&repo);
    commit(&repo, "a.txt", "base");
    git(&repo, &["switch", "-q", "-c", "side"]);
    let theirs = commit(&repo, "a.txt", "theirs");
    git(&repo, &["switch", "-q", "main"]);
    commit(&repo, "a.txt", "ours");
    (repo, theirs)
}

#[test]
fn actions_that_make_no_sense_for_a_commit_are_refused() {
    // Given.
    let sb = Sandbox::new();
    let (repo, side) = with_side_branch(&sb);
    let own = git(&repo, &["rev-parse", "HEAD~1"]);
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&repo);

    // When.
    let (picked, pick_body) = post(port, &format!("/api/cherry-pick?{q}&h={own}"));
    let (reverted, revert_body) = post(port, &format!("/api/revert?{q}&h={side}"));
    let (_, rebase_body) = post(port, &format!("/api/rebase?{q}&ref={own}"));

    // Then.
    assert_eq!(picked, 400);
    assert!(pick_body.contains("already contains"), "{pick_body}");
    assert_eq!(reverted, 400);
    assert!(revert_body.contains("not in the history"), "{revert_body}");
    assert!(rebase_body.contains("nothing to rebase"), "{rebase_body}");
    assert!(json(port, &format!("/api/state?{q}"))["operation"].is_null(), "nothing was left half done");
}

#[test]
fn a_stopped_merge_is_reported_then_finished_once_resolved() {
    // Given.
    let sb = Sandbox::new();
    let (repo, _) = conflicting(&sb);
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&repo);
    post(port, &format!("/api/merge?{q}&ref=side&mode=fast-forward"));

    // When.
    let stopped = json(port, &format!("/api/state?{q}"));
    let (early, early_body) = post(port, &format!("/api/continue?{q}"));
    fs::write(repo.join("a.txt"), "ours and theirs\n").unwrap();
    let (finished, body) = post(port, &format!("/api/continue?{q}"));

    // Then.
    assert_eq!((stopped["operation"].as_str(), stopped["status"]["conflicts"].as_u64()), (Some("merge"), Some(1)));
    assert_eq!(early, 400);
    assert!(early_body.contains("conflict markers"), "{early_body}");
    assert_eq!(finished, 200, "{body}");
    assert_eq!(git(&repo, &["log", "-1", "--format=%s"]), "Merge branch 'side'");
    assert!(json(port, &format!("/api/state?{q}"))["operation"].is_null());
}

#[test]
fn a_stopped_rebase_can_be_aborted_back_to_where_it_was() {
    // Given.
    let sb = Sandbox::new();
    let (repo, _) = conflicting(&sb);
    let before = git(&repo, &["rev-parse", "HEAD"]);
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&repo);
    let (stopped, _) = post(port, &format!("/api/rebase?{q}&ref=side"));

    // When.
    let during = json(port, &format!("/api/state?{q}"))["operation"].clone();
    let (aborted, body) = post(port, &format!("/api/abort?{q}"));

    // Then.
    assert_eq!(stopped, 400);
    assert_eq!(during, "rebase");
    assert_eq!(aborted, 200, "{body}");
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), before);
    assert_eq!(git(&repo, &["branch", "--show-current"]), "main");
}

#[test]
fn drop_and_rebase_keep_uncommitted_work_aside_and_put_it_back() {
    // Given.
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    commit(&repo, "a.txt", "one");
    let doomed = commit(&repo, "b.txt", "two");
    commit(&repo, "c.txt", "three");
    fs::write(repo.join("a.txt"), "edited, not committed\n").unwrap();
    let port = start(vec![sb.0.clone()], None);

    // When.
    let (status, body) = post(port, &format!("/api/drop?{}&h={doomed}", repo_query(&repo)));

    // Then.
    assert_eq!(status, 200, "{body}");
    assert_eq!(git(&repo, &["log", "--format=%s"]), "three\none");
    assert_eq!(fs::read_to_string(repo.join("a.txt")).unwrap(), "edited, not committed\n");
}
