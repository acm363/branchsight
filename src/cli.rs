use std::path::PathBuf;
use std::process::exit;

use branchsight::config::{Config, canonical, expand_home};
use branchsight::git::Repo;

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
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("serve") => {}
        Some("-h" | "--help") => {
            println!("{USAGE}");
            exit(0);
        }
        Some(other) => fail(&format!("unknown command {other}")),
        None => fail("a command is required"),
    }

    let mut port = DEFAULT_PORT;
    let mut dir = PathBuf::from(".");
    let mut roots = Vec::new();
    while let Some(arg) = args.next() {
        let (flag, inline) = match arg.split_once('=') {
            Some((flag, value)) if flag.starts_with("--") => (flag.to_owned(), Some(value.to_owned())),
            _ => (arg.clone(), None),
        };
        let mut value = || inline.clone().or_else(|| args.next()).unwrap_or_else(|| fail(&format!("{flag} needs a value")));
        match flag.as_str() {
            "-p" | "--port" => {
                let given = value();
                port = given.parse().unwrap_or_else(|_| fail(&format!("invalid port {given}")));
            }
            "-r" | "--root" => roots.push(expand_home(&value())),
            "-h" | "--help" => {
                println!("{USAGE}");
                exit(0);
            }
            unknown if unknown.starts_with('-') => fail(&format!("unknown option {unknown}")),
            _ => dir = expand_home(&arg),
        }
    }

    let dir = canonical(dir);
    let default_repo = Repo::open(&dir).ok().map(|repo| repo.root);
    if roots.is_empty() {
        roots.push(match &default_repo {
            Some(top) => top.parent().unwrap_or(top).to_path_buf(),
            None => dir,
        });
    }
    let roots = roots.into_iter().map(canonical).collect();
    Args { port, config: Config { roots, default_repo } }
}

fn fail(message: &str) -> ! {
    eprintln!("branchsight: {message}\n{USAGE}");
    exit(2);
}
