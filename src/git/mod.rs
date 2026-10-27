mod diff;
mod read;
mod run;
mod write;

use std::path::{Path, PathBuf};

pub use diff::FileDiff;

pub struct Repo {
    pub root: PathBuf,
}

impl Repo {
    pub fn open(dir: &Path) -> Result<Repo, String> {
        match run::git(dir, &["rev-parse", "--show-toplevel"]) {
            Ok(top) => Ok(Repo { root: PathBuf::from(top.trim()) }),
            Err(e) if e.contains("not a git repository") => Err(format!("not a git repository: {}", dir.display())),
            Err(e) => Err(e),
        }
    }

    fn git(&self, args: &[&str]) -> Result<String, String> {
        run::git(&self.root, args)
    }

    fn check_commit(&self, hash: &str) -> Result<(), String> {
        let is_hash = (4..=64).contains(&hash.len()) && hash.chars().all(|c| c.is_ascii_hexdigit());
        if is_hash && self.git(&["cat-file", "-e", &format!("{hash}^{{commit}}")]).is_ok() {
            Ok(())
        } else {
            Err(format!("unknown commit {hash}"))
        }
    }
}
