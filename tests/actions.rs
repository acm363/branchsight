mod common;

use std::path::PathBuf;

use branchsight::http::PAGE_HEADER;
use branchsight::url::encode;
use common::*;

fn cloned_with_feature(sb: &Sandbox) -> (PathBuf, PathBuf) {
    let origin = sb.path("origin");
    init(&origin);
    commit(&origin, "a", "A");
    git(&origin, &["checkout", "-q", "-b", "feature"]);
    commit(&origin, "f", "F");
    git(&origin, &["checkout", "-q", "main"]);
    let copy = clone(sb, &origin, "clone");
    (origin, copy)
}

#[test]
fn requests_from_other_sites_are_refused_before_git_runs() {
    // Given.
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    commit(&repo, "a", "A");
    let port = start(vec![sb.0.clone()], None);
    let path = format!("/api/state?{}", repo_query(&repo));

    // When.
    let without_header = request(port, "GET", &path, &[]).status;
    let cross_site = request(port, "GET", &path, &[(PAGE_HEADER, "1"), ("Sec-Fetch-Site", "cross-site")]).status;
    let page = request(port, "GET", "/", &[]).status;
    let ping = request(port, "GET", "/api/ping", &[]).status;

    // Then.
    assert_eq!((without_header, cross_site), (403, 403));
    assert_eq!((page, ping), (200, 200), "the page itself and the liveness check need no header");
}

#[test]
fn actions_from_other_sites_are_refused() {
    // Given.
    let sb = Sandbox::new();
    let (_, copy) = cloned_with_feature(&sb);
    git(&copy, &["branch", "local-only"]);
    let port = start(vec![sb.0.clone()], None);
    let path = format!("/api/checkout?{}&ref=local-only", repo_query(&copy));

    // When.
    let without_header = request(port, "POST", &path, &[]).status;
    let cross_site = request(port, "POST", &path, &[(PAGE_HEADER, "1"), ("Sec-Fetch-Site", "cross-site")]).status;
    let after_refusals = git(&copy, &["branch", "--show-current"]);
    let own_page = request(port, "POST", &path, &[(PAGE_HEADER, "1"), ("Sec-Fetch-Site", "same-origin")]).status;

    // Then.
    assert_eq!((without_header, cross_site), (403, 403));
    assert_eq!(after_refusals, "main");
    assert_eq!(own_page, 200);
    assert_eq!(git(&copy, &["branch", "--show-current"]), "local-only");
}

#[test]
fn checkout_switches_to_a_local_branch_and_tracks_a_remote_one() {
    // Given.
    let sb = Sandbox::new();
    let (_, copy) = cloned_with_feature(&sb);
    git(&copy, &["branch", "local-only"]);
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&copy);

    // When.
    let (local, local_body) = post(port, &format!("/api/checkout?{q}&ref=local-only"));
    let on_local = git(&copy, &["branch", "--show-current"]);
    let (remote, remote_body) = post(port, &format!("/api/checkout?{q}&ref={}", encode("origin/feature")));

    // Then.
    assert_eq!(local, 200, "{local_body}");
    assert_eq!(on_local, "local-only");
    assert_eq!(remote, 200, "{remote_body}");
    assert_eq!(git(&copy, &["branch", "--show-current"]), "feature");
    assert_eq!(git(&copy, &["rev-parse", "--abbrev-ref", "feature@{upstream}"]), "origin/feature");
}

fn behind_and_ahead(sb: &Sandbox, local_too: bool) -> (std::path::PathBuf, String) {
    let (origin, copy) = cloned_with_feature(sb);
    git(&copy, &["switch", "-q", "feature"]);
    if local_too {
        commit(&copy, "local.txt", "local only");
    }
    git(&copy, &["switch", "-q", "main"]);
    git(&origin, &["switch", "-q", "feature"]);
    let newer = commit(&origin, "remote.txt", "remote only");
    git(&origin, &["switch", "-q", "main"]);
    git(&copy, &["fetch", "-q"]);
    (copy, newer)
}

#[test]
fn checkout_of_a_remote_branch_only_switches_to_its_local_copy() {
    // Given.
    let sb = Sandbox::new();
    let (copy, newer) = behind_and_ahead(&sb, false);
    let before = git(&copy, &["rev-parse", "feature"]);
    let port = start(vec![sb.0.clone()], None);

    // When.
    let (status, body) = post(port, &format!("/api/checkout?{}&ref={}", repo_query(&copy), encode("origin/feature")));

    // Then.
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("sync to bring them in"), "{body}");
    assert_eq!(git(&copy, &["branch", "--show-current"]), "feature");
    assert_eq!(git(&copy, &["rev-parse", "HEAD"]), before, "a checkout does not pull");
    assert_ne!(before, newer);
}

#[test]
fn pull_brings_a_branch_that_is_only_behind_up_without_switching() {
    // Given.
    let sb = Sandbox::new();
    let (copy, newer) = behind_and_ahead(&sb, false);
    let port = start(vec![sb.0.clone()], None);

    // When.
    let (status, body) = post(port, &format!("/api/pull?{}&branch=feature&from={}&mode=merge", repo_query(&copy), encode("origin/feature")));

    // Then.
    assert_eq!(status, 200, "{body}");
    assert_eq!(git(&copy, &["rev-parse", "feature"]), newer);
    assert_eq!(git(&copy, &["branch", "--show-current"]), "main", "the current branch stays");
}

#[test]
fn pull_of_a_diverged_branch_merges_or_rebases_it() {
    // Given.
    let sb = Sandbox::new();
    let (copy, newer) = behind_and_ahead(&sb, true);
    let port = start(vec![sb.0.clone()], None);
    let pull = |mode: &str| post(port, &format!("/api/pull?{}&branch=feature&from={}&mode={mode}", repo_query(&copy), encode("origin/feature")));

    // When.
    let (forward, forward_body) = pull("fast-forward");
    let (rebased, body) = pull("rebase");

    // Then.
    assert_eq!(forward, 400);
    assert!(forward_body.contains("have diverged"), "{forward_body}");
    assert_eq!(rebased, 200, "{body}");
    assert_eq!(git(&copy, &["branch", "--show-current"]), "feature", "a diverged branch is pulled where it is checked out");
    assert_eq!(git(&copy, &["rev-parse", "HEAD~1"]), newer);
    assert_eq!(git(&copy, &["log", "-1", "--format=%s"]), "local only");
}

#[test]
fn refs_say_how_far_a_branch_is_ahead_of_and_behind_what_it_tracks() {
    // Given.
    let sb = Sandbox::new();
    let (copy, _) = behind_and_ahead(&sb, true);
    let port = start(vec![sb.0.clone()], None);

    // When.
    let refs = json(port, &format!("/api/refs?{}", repo_query(&copy)));

    // Then.
    let feature = refs.as_array().unwrap().iter().find(|r| r["name"] == "feature").unwrap();
    let main = refs.as_array().unwrap().iter().find(|r| r["name"] == "main").unwrap();
    assert_eq!((feature["ahead"].as_u64(), feature["behind"].as_u64()), (Some(1), Some(1)));
    assert_eq!((main["ahead"].as_u64(), main["behind"].as_u64()), (Some(0), Some(0)));
}

#[test]
fn reset_to_remote_makes_a_branch_match_it_where_it_is_or_when_checked_out() {
    // Given.
    let sb = Sandbox::new();
    let (copy, newer) = behind_and_ahead(&sb, true);
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&copy);
    let reset = || post(port, &format!("/api/reset-to-remote?{q}&branch=feature&from={}", encode("origin/feature")));

    // When.
    let (elsewhere, body) = reset();
    let moved = git(&copy, &["rev-parse", "feature"]);
    git(&copy, &["switch", "-q", "feature"]);
    commit(&copy, "again.txt", "local again");
    std::fs::write(copy.join("draft.txt"), "uncommitted\n").unwrap();
    let (here, _) = reset();
    let refused = post(port, &format!("/api/reset-to-remote?{q}&branch=feature&from=main")).0;

    // Then.
    assert_eq!(elsewhere, 200, "{body}");
    assert_eq!(moved, newer);
    assert_eq!(git(&copy, &["branch", "--show-current"]), "feature");
    assert_eq!(here, 200);
    assert_eq!(git(&copy, &["rev-parse", "HEAD"]), newer);
    assert!(copy.join("draft.txt").exists(), "uncommitted work is kept");
    assert_eq!(refused, 400);
}

#[test]
fn pull_into_the_current_branch_merges_and_refuses_unknown_names() {
    // Given.
    let sb = Sandbox::new();
    let (copy, newer) = behind_and_ahead(&sb, true);
    git(&copy, &["switch", "-q", "feature"]);
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&copy);

    // When.
    let (merged, body) = post(port, &format!("/api/pull?{q}&branch=feature&from={}&mode=merge", encode("origin/feature")));
    let refused = [
        post(port, &format!("/api/pull?{q}&branch=nope&from={}&mode=merge", encode("origin/feature"))).0,
        post(port, &format!("/api/pull?{q}&branch=feature&from=main&mode=merge")).0,
        post(port, &format!("/api/pull?{q}&branch=feature&from={}&mode=sideways", encode("origin/feature"))).0,
    ];

    // Then.
    assert_eq!(merged, 200, "{body}");
    assert_eq!(git(&copy, &["rev-list", "--parents", "-n", "1", "HEAD"]).split(' ').count(), 3);
    assert!(git_succeeds(&copy, &["merge-base", "--is-ancestor", &newer, "HEAD"]));
    assert_eq!(refused, [400; 3]);
}

#[test]
fn checkout_accepts_a_branch_with_a_component_starting_with_a_dash() {
    // Given.
    let sb = Sandbox::new();
    let (_, copy) = cloned_with_feature(&sb);
    git(&copy, &["branch", "feature/-wip"]);
    let port = start(vec![sb.0.clone()], None);

    // When.
    let (status, body) = post(port, &format!("/api/checkout?{}&ref={}", repo_query(&copy), encode("feature/-wip")));

    // Then.
    assert_eq!(status, 200, "{body}");
    assert_eq!(git(&copy, &["branch", "--show-current"]), "feature/-wip");
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
    let (tag, tag_body) = post(port, &format!("/api/checkout?{q}&ref=v1.0.0"));
    let at_tag = git(&repo, &["rev-parse", "HEAD"]);
    let branch_at_tag = git(&repo, &["branch", "--show-current"]);
    let (short, short_body) = post(port, &format!("/api/checkout?{q}&ref={}", &second[..12]));
    let (unknown, _) = post(port, &format!("/api/checkout?{q}&ref=deadbeefdeadbeef"));

    // Then.
    assert_eq!(tag, 200, "{tag_body}");
    assert_eq!((at_tag.as_str(), branch_at_tag.as_str()), (first.as_str(), ""), "HEAD is detached at the tag");
    assert_eq!(short, 200, "{short_body}");
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), second);
    assert_eq!(unknown, 400, "an unknown hash is refused");
}

#[test]
fn checkout_refuses_unknown_and_option_like_names_and_requires_post() {
    // Given.
    let sb = Sandbox::new();
    let (_, copy) = cloned_with_feature(&sb);
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&copy);

    // When.
    let refused: Vec<u16> = ["nope", "--orphan=x", "-", "origin/-f", "../main"]
        .iter()
        .map(|name| post(port, &format!("/api/checkout?{q}&ref={}", encode(name))).0)
        .collect();
    let (as_get, _) = get(port, &format!("/api/checkout?{q}&ref=main"));

    // Then.
    assert_eq!(refused, [400; 5]);
    assert_eq!(as_get, 400);
    assert_eq!(git(&copy, &["branch", "--show-current"]), "main");
}

#[test]
fn delete_remote_branch_removes_it_on_the_remote_only() {
    // Given.
    let sb = Sandbox::new();
    let (origin, copy) = cloned_with_feature(&sb);
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&copy);

    // When.
    let (as_get, _) = get(port, &format!("/api/delete-remote?{q}&ref={}", encode("origin/feature")));
    let (deleted, body) = post(port, &format!("/api/delete-remote?{q}&ref={}", encode("origin/feature")));
    let (local_name, _) = post(port, &format!("/api/delete-remote?{q}&ref=main"));

    // Then.
    assert_eq!(as_get, 400);
    assert_eq!(deleted, 200, "{body}");
    assert_eq!(git(&origin, &["branch", "--list", "feature"]), "");
    assert_eq!(git(&origin, &["branch", "--list", "main"]), "* main");
    assert_eq!(local_name, 400, "a local branch is not a remote one");
}

#[test]
fn fetch_brings_new_remote_commits_and_requires_post() {
    // Given.
    let sb = Sandbox::new();
    let origin = sb.path("origin");
    init(&origin);
    commit(&origin, "a", "A");
    let copy = clone(&sb, &origin, "clone");
    let newer = commit(&origin, "b", "B upstream");
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&copy);

    // When.
    let (as_get, _) = get(port, &format!("/api/fetch?{q}"));
    let (fetched, body) = post(port, &format!("/api/fetch?{q}"));

    // Then.
    assert_eq!(as_get, 400);
    assert_eq!(fetched, 200, "{body}");
    assert_eq!(git(&copy, &["rev-parse", "origin/main"]), newer);
}

#[test]
fn push_sends_a_new_branch_and_sets_its_upstream() {
    // Given.
    let sb = Sandbox::new();
    let (origin, copy) = cloned_with_feature(&sb);
    git(&copy, &["checkout", "-q", "-b", "fresh"]);
    let tip = commit(&copy, "fresh.txt", "fresh work");
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&copy);

    // When.
    let (as_get, _) = get(port, &format!("/api/push?{q}&ref=fresh&remote=origin&mode=normal"));
    let (pushed, body) = post(port, &format!("/api/push?{q}&ref=fresh&remote=origin&mode=normal"));

    // Then.
    assert_eq!(as_get, 400);
    assert_eq!(pushed, 200, "{body}");
    assert_eq!(git(&origin, &["rev-parse", "refs/heads/fresh"]), tip);
    assert_eq!(git(&copy, &["rev-parse", "--abbrev-ref", "fresh@{upstream}"]), "origin/fresh");
}

#[test]
fn push_of_a_branch_started_from_another_remote_branch_tracks_its_own_name() {
    // Given.
    let sb = Sandbox::new();
    let (origin, copy) = cloned_with_feature(&sb);
    git(&copy, &["switch", "-q", "-c", "fix", "origin/main"]);
    let tip = commit(&copy, "fix.txt", "the fix");
    let port = start(vec![sb.0.clone()], None);

    // When.
    let (pushed, body) = post(port, &format!("/api/push?{}&ref=fix&remote=origin&mode=normal", repo_query(&copy)));

    // Then.
    assert_eq!(pushed, 200, "{body}");
    assert!(body.contains("now tracks origin/fix"), "{body}");
    assert_eq!(git(&origin, &["rev-parse", "refs/heads/fix"]), tip);
    assert_ne!(git(&origin, &["rev-parse", "refs/heads/main"]), tip, "main on the remote is untouched");
    assert_eq!(git(&copy, &["rev-parse", "--abbrev-ref", "fix@{upstream}"]), "origin/fix");
}

#[test]
fn push_overwrites_a_rewritten_branch_only_when_asked() {
    // Given.
    let sb = Sandbox::new();
    let (origin, copy) = cloned_with_feature(&sb);
    git(&copy, &["checkout", "-q", "-b", "feature", "origin/feature"]);
    git(&copy, &["commit", "-q", "--amend", "-m", "F rewritten"]);
    let rewritten = git(&copy, &["rev-parse", "HEAD"]);
    let port = start(vec![sb.0.clone()], None);
    let push = |mode: &str| post(port, &format!("/api/push?{}&ref=feature&remote=origin&mode={mode}", repo_query(&copy)));

    // When.
    let (normal, normal_body) = push("normal");
    let after_normal = git(&origin, &["rev-parse", "refs/heads/feature"]);
    let (lease, body) = push("force-with-lease");

    // Then.
    assert_eq!(normal, 400, "a normal push refuses to drop remote commits");
    assert!(normal_body.contains("has commits you do not have"), "{normal_body}");
    assert_ne!(after_normal, rewritten);
    assert_eq!(lease, 200, "{body}");
    assert_eq!(git(&origin, &["rev-parse", "refs/heads/feature"]), rewritten);
}

#[test]
fn push_refuses_unknown_branches_remotes_and_modes() {
    // Given.
    let sb = Sandbox::new();
    let (_, copy) = cloned_with_feature(&sb);
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&copy);
    let push = |rest: &str| post(port, &format!("/api/push?{q}&{rest}")).0;

    // When.
    let refused = [
        push("ref=nope&remote=origin&mode=normal"),
        push("ref=main&remote=elsewhere&mode=normal"),
        push("ref=main&remote=origin&mode=sideways"),
        push(&format!("ref=main&remote={}&mode=normal", encode("--receive-pack=x"))),
    ];

    // Then.
    assert_eq!(refused, [400; 4]);
}

fn with_side_branch(sb: &Sandbox) -> (std::path::PathBuf, String) {
    let repo = sb.path("repo");
    init(&repo);
    commit(&repo, "a.txt", "base");
    git(&repo, &["switch", "-q", "-c", "side"]);
    let side = commit(&repo, "b.txt", "side work");
    git(&repo, &["switch", "-q", "main"]);
    (repo, side)
}

#[test]
fn merge_fast_forwards_by_default_and_can_force_a_merge_commit() {
    // Given.
    let sb = Sandbox::new();
    let (repo, side) = with_side_branch(&sb);
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&repo);

    // When.
    let (as_get, _) = get(port, &format!("/api/merge?{q}&ref=side&mode=fast-forward"));
    let (forward, body) = post(port, &format!("/api/merge?{q}&ref=side&mode=fast-forward"));
    let after_forward = git(&repo, &["rev-parse", "HEAD"]);
    git(&repo, &["reset", "-q", "--hard", "HEAD~1"]);
    let (merged, _) = post(port, &format!("/api/merge?{q}&ref=side&mode=merge-commit"));

    // Then.
    assert_eq!(as_get, 400);
    assert_eq!(forward, 200, "{body}");
    assert_eq!(after_forward, side);
    assert_eq!(merged, 200);
    assert_eq!(git(&repo, &["rev-list", "--parents", "-n", "1", "HEAD"]).split(' ').count(), 3, "a merge commit has two parents");
    assert_eq!(git(&repo, &["log", "-1", "--format=%s"]), "Merge branch 'side'");
}

#[test]
fn merge_can_squash_or_stop_before_committing() {
    // Given.
    let sb = Sandbox::new();
    let (repo, _) = with_side_branch(&sb);
    let base = git(&repo, &["rev-parse", "HEAD"]);
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&repo);

    // When.
    let (squashed, body) = post(port, &format!("/api/merge?{q}&ref=side&mode=squash"));
    let staged = git(&repo, &["diff", "--cached", "--name-only"]);
    let head_after_squash = git(&repo, &["rev-parse", "HEAD"]);
    git(&repo, &["reset", "-q", "--hard"]);
    let (pending, _) = post(port, &format!("/api/merge?{q}&ref=side&mode=fast-forward&commit=0"));

    // Then.
    assert_eq!(squashed, 200, "{body}");
    assert_eq!((staged.as_str(), head_after_squash.as_str()), ("b.txt", base.as_str()), "a squash only stages the changes");
    assert_eq!(pending, 200);
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), base, "nothing is committed yet");
    assert!(git_succeeds(&repo, &["rev-parse", "-q", "--verify", "MERGE_HEAD"]), "the merge waits to be committed");
}

#[test]
fn merge_reports_conflicts_and_leaves_them_to_resolve() {
    // Given.
    let sb = Sandbox::new();
    let (repo, _) = with_side_branch(&sb);
    git(&repo, &["switch", "-q", "side"]);
    commit(&repo, "a.txt", "theirs");
    git(&repo, &["switch", "-q", "main"]);
    commit(&repo, "a.txt", "ours");
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&repo);

    // When.
    let (status, body) = post(port, &format!("/api/merge?{q}&ref=side&mode=fast-forward"));

    // Then.
    assert_eq!(status, 400);
    assert!(body.contains("conflicts in a.txt") && body.contains("continue or abort"), "{body}");
    let state = json(port, &format!("/api/state?{q}"));
    assert_eq!((state["status"]["conflicts"].as_u64(), state["operation"].as_str()), (Some(1), Some("merge")));
}

#[test]
fn merge_refuses_what_it_cannot_merge() {
    // Given.
    let sb = Sandbox::new();
    let (repo, _) = with_side_branch(&sb);
    let port = start(vec![sb.0.clone()], None);
    let q = repo_query(&repo);
    let merge = |rest: &str| post(port, &format!("/api/merge?{q}&{rest}"));

    // When.
    let refused = [
        merge("ref=nope&mode=fast-forward").0,
        merge("ref=main&mode=fast-forward").0,
        merge("ref=side&mode=sideways").0,
        merge(&format!("ref={}&mode=fast-forward", encode("--strategy=ours"))).0,
    ];
    let (_, already) = merge(&format!("ref={}&mode=fast-forward", git(&repo, &["rev-parse", "HEAD~0"])));

    // Then.
    assert_eq!(refused, [400; 4]);
    assert!(already.contains("already contains"), "{already}");
}

#[cfg(unix)]
#[test]
fn fetch_uses_the_ssh_command_the_user_configured() {
    use std::os::unix::fs::PermissionsExt;

    // Given.
    if std::env::var_os("GIT_SSH_COMMAND").is_some() || std::env::var_os("GIT_SSH").is_some() {
        return;
    }
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    commit(&repo, "a", "A");
    let marker = sb.path("ssh-was-used");
    let script = sb.path("my-ssh");
    std::fs::write(&script, format!("#!/bin/sh\ntouch '{}'\nexit 1\n", marker.display())).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    git(&repo, &["remote", "add", "origin", "ssh://git.example.invalid/repo.git"]);
    git(&repo, &["config", "core.sshCommand", script.to_str().unwrap()]);
    let port = start(vec![sb.0.clone()], None);

    // When.
    let (status, _) = post(port, &format!("/api/fetch?{}", repo_query(&repo)));

    // Then.
    assert_eq!(status, 400, "the fake ssh fails the fetch");
    assert!(marker.exists(), "BranchSight must not replace the configured core.sshCommand");
}
