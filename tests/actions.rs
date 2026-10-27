mod common;

use std::path::PathBuf;

use branchsight::http::ACTION_HEADER;
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
fn actions_from_other_sites_are_refused() {
    // Given.
    let sb = Sandbox::new();
    let (_, copy) = cloned_with_feature(&sb);
    git(&copy, &["branch", "local-only"]);
    let port = start(vec![sb.0.clone()], None);
    let path = format!("/api/checkout?{}&ref=local-only", repo_query(&copy));

    // When.
    let without_header = request(port, "POST", &path, &[]).status;
    let cross_site = request(port, "POST", &path, &[(ACTION_HEADER, "1"), ("Sec-Fetch-Site", "cross-site")]).status;
    let after_refusals = git(&copy, &["branch", "--show-current"]);
    let own_page = request(port, "POST", &path, &[(ACTION_HEADER, "1"), ("Sec-Fetch-Site", "same-origin")]).status;

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

#[cfg(unix)]
#[test]
fn fetch_uses_the_ssh_command_the_user_configured() {
    use std::os::unix::fs::PermissionsExt;

    // Given.
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
