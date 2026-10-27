mod diff;
mod patch;
mod read;
mod run;
mod write;

use std::path::{Path, PathBuf};

pub use diff::FileDiff;

pub struct Repo {
    pub root: PathBuf,
    shallow: PathBuf,
}

impl Repo {
    pub fn open(dir: &Path) -> Result<Repo, String> {
        let out = match run::git(dir, &["rev-parse", "--show-toplevel", "--git-path", "shallow"]) {
            Err(e) if e.contains("not a git repository") => return Err(format!("not a git repository: {}", dir.display())),
            other => other?,
        };
        let mut lines = out.lines();
        let (Some(root), Some(shallow)) = (lines.next(), lines.next()) else {
            return Err(format!("not a working tree: {}", dir.display()));
        };
        Ok(Repo { root: PathBuf::from(root), shallow: dir.join(shallow) })
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
