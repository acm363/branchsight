mod common;

use std::collections::BTreeSet;
use std::fs;

use branchsight::git::Repo;
use branchsight::http::PAGE_HEADER;
use common::*;

#[test]
fn projects_lists_repositories_under_the_roots_and_skips_vendored_ones() {
    // Given.
    let sb = Sandbox::new();
    init(&sb.path("alpha"));
    init(&sb.path("group/beta"));
    init(&sb.path("web/node_modules/dep"));
    init(&sb.path("alpha/nested-inside-a-repo"));
    fs::create_dir_all(sb.path("not-a-repo")).unwrap();
    let port = start(vec![sb.0.clone()], None);

    // When.
    let projects = json(port, "/api/projects");

    // Then.
    let mut names: Vec<&str> = projects.as_array().unwrap().iter().map(|p| p["name"].as_str().unwrap()).collect();
    names.sort();
    assert_eq!(names, ["alpha", "beta"]);
}

#[cfg(unix)]
#[test]
fn projects_are_found_through_symlinked_folders() {
    // Given.
    let sb = Sandbox::new();
    init(&sb.path("elsewhere/linked"));
    fs::create_dir_all(sb.path("home")).unwrap();
    std::os::unix::fs::symlink(sb.path("elsewhere"), sb.path("home/code")).unwrap();
    std::os::unix::fs::symlink(sb.path("elsewhere"), sb.path("home/same-again")).unwrap();
    let port = start(vec![sb.path("home")], None);

    // When.
    let projects = json(port, "/api/projects");

    // Then.
    assert_eq!(projects.as_array().unwrap().len(), 1, "{projects}");
    assert_eq!(projects[0]["path"], Repo::open(&sb.path("home/code/linked")).unwrap().root.to_string_lossy().as_ref());
}

#[cfg(unix)]
#[test]
fn a_repository_in_a_folder_ending_with_a_space_opens() {
    // Given.
    let sb = Sandbox::new();
    let repo = sb.path("spaced ");
    init(&repo);
    commit(&repo, "a", "A");
    let port = start(vec![sb.0.clone()], None);

    // When.
    let info = json(port, &format!("/api/repo?{}", repo_query(&repo)));

    // Then.
    assert_eq!(info["name"], "spaced ");
}

#[test]
fn repository_resolution_reports_what_is_missing() {
    // Given.
    let sb = Sandbox::new();
    let repo = sb.path("repo");
    init(&repo);
    commit(&repo, "a", "A");
    fs::create_dir_all(sb.path("plain")).unwrap();
    let without_default = start(vec![sb.0.clone()], None);
    let with_default = start(vec![sb.0.clone()], Some(repo.clone()));

    // When.
    let none = get(without_default, "/api/repo");
    let plain = get(without_default, &format!("/api/repo?{}", repo_query(&sb.path("plain"))));
    let missing = get(without_default, &format!("/api/repo?{}", repo_query(&sb.path("does-not-exist"))));
    let info = json(with_default, "/api/repo");

    // Then.
    assert_eq!(none, (400, "no repository selected".into()));
    assert_eq!(plain.0, 400);
    assert!(plain.1.starts_with("not a git repository"), "{}", plain.1);
    assert_eq!(missing.0, 400);
    assert!(missing.1.contains("does-not-exist"), "the reason names the folder: {}", missing.1);
    assert_eq!(info["path"], Repo::open(&repo).unwrap().root.to_string_lossy().as_ref());
    assert_eq!(info["branch"], "main");
}

#[test]
fn only_loopback_host_names_are_served() {
    // Given.
    let sb = Sandbox::new();
    let port = start(vec![sb.0.clone()], None);

    // When.
    let foreign: Vec<u16> = ["evil.example", "localhost.evil.example", "192.168.1.10"]
        .iter()
        .map(|host| request(port, "GET", "/api/projects", &[("Host", host), (PAGE_HEADER, "1")]).status)
        .collect();
    let named = request(port, "GET", "/api/projects", &[("Host", "my-project.localhost:7420"), (PAGE_HEADER, "1")]).status;

    // Then.
    assert_eq!(foreign, [403; 3]);
    assert_eq!(named, 200);
}

#[test]
fn the_page_and_every_module_it_imports_are_served() {
    // Given.
    let sb = Sandbox::new();
    let port = start(vec![sb.0.clone()], None);
    let index = request(port, "GET", "/", &[]);

    // When.
    let mut pending: Vec<String> = ["src=\"/", "href=\"/"]
        .iter()
        .flat_map(|attr| index.body.match_indices(attr).map(|(i, m)| index.body[i + m.len() - 1..].split('"').next().unwrap().to_owned()))
        .collect();
    let mut served = BTreeSet::new();
    while let Some(asset) = pending.pop() {
        if !served.insert(asset.clone()) {
            continue;
        }
        let reply = request(port, "GET", &asset, &[]);
        assert_eq!(reply.status, 200, "{asset}");
        assert!(!reply.body.is_empty() && !reply.content_type.is_empty(), "{asset}");
        if asset.ends_with(".js") {
            let folder = asset.rsplit_once('/').map_or("", |(dir, _)| dir).to_owned();
            pending.extend(reply.body.match_indices("from '").map(|(i, m)| resolve(&folder, reply.body[i + m.len()..].split('\'').next().unwrap())));
        }
    }

    // Then.
    assert!(index.content_type.starts_with("text/html"), "{}", index.content_type);
    assert!(served.contains("/app.js") && served.contains("/history/graph.js") && served.contains("/style.css"), "{served:?}");
    assert_eq!(served.iter().filter(|a| a.ends_with(".js")).count(), 20, "{served:?}");
}

fn resolve(folder: &str, import: &str) -> String {
    let mut parts: Vec<&str> = folder.split('/').filter(|p| !p.is_empty()).collect();
    for part in import.split('/') {
        match part {
            "." => {}
            ".." => {
                parts.pop();
            }
            name => parts.push(name),
        }
    }
    format!("/{}", parts.join("/"))
}
