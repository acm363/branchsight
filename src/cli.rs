use std::path::PathBuf;
use std::process::exit;

use branchsight::git::Repo;
use branchsight::server::Config;

const DEFAULT_PORT: u16 = 7420;
const USAGE: &str = "usage: branchsight serve [--port N] [--root DIR]... [dir]
Serves the BranchSight page for every git repository under the roots, on loopback only.
dir, when a repository, is the default one; the roots default to its parent.
If a branchsight already listens on the port, prints the URL of dir's repository and exits.";

pub struct Args {
    pub port: u16,
    pub config: Config,
}

pub fn parse() -> Args {
    let mut port = DEFAULT_PORT;
    let mut dir = PathBuf::from(".");
    let mut roots = Vec::new();
    let mut args = std::env::args().skip(1).peekable();
    if args.next_if(|a| a == "serve").is_none() {
        eprintln!("{USAGE}");
        exit(if args.peek().is_some_and(|a| a == "-h" || a == "--help") { 0 } else { 2 });
    }
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-p" | "--port" => port = args.next().and_then(|p| p.parse().ok()).unwrap_or(port),
            "-r" | "--root" => roots.extend(args.next().map(PathBuf::from)),
            "-h" | "--help" => {
                println!("{USAGE}");
                exit(0);
            }
            other => dir = PathBuf::from(other),
        }
    }

    let dir = dir.canonicalize().unwrap_or(dir);
    let default_repo = Repo::open(&dir).map(|r| r.root);
    if roots.is_empty() {
        roots.push(match &default_repo {
            Some(top) => top.parent().unwrap_or(top).to_path_buf(),
            None => dir,
        });
    }
    let roots = roots.into_iter().map(|r| r.canonicalize().unwrap_or(r)).collect();
    Args { port, config: Config { roots, default_repo } }
}
