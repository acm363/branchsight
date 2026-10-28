mod diff;
mod history;
mod patch;
mod read;
mod run;
mod write;

use std::path::{Path, PathBuf};

pub use diff::FileDiff;

const OPERATIONS: [(&str, &str); 5] =
    [("merge", "MERGE_HEAD"), ("rebase", "rebase-merge"), ("rebase", "rebase-apply"), ("cherry-pick", "CHERRY_PICK_HEAD"), ("revert", "REVERT_HEAD")];

pub struct Repo {
    pub root: PathBuf,
    shallow: PathBuf,
    operations: Vec<(&'static str, PathBuf)>,
}

impl Repo {
    pub fn open(dir: &Path) -> Result<Repo, String> {
        let mut args = vec!["rev-parse", "--show-toplevel", "--git-path", "shallow"];
        for (_, file) in OPERATIONS {
            args.extend(["--git-path", file]);
        }
        let out = match run::git(dir, &args) {
            Err(e) if e.contains("not a git repository") => return Err(format!("not a git repository: {}", dir.display())),
            other => other?,
        };
        let lines: Vec<&str> = out.lines().collect();
        let [root, shallow, operations @ ..] = &lines[..] else {
            return Err(format!("not a working tree: {}", dir.display()));
        };
        let operations = OPERATIONS.iter().zip(operations).map(|((name, _), path)| (*name, dir.join(path))).collect();
        Ok(Repo { root: PathBuf::from(root), shallow: dir.join(shallow), operations })
    }

    fn operation(&self) -> Option<&'static str> {
        self.operations.iter().find(|(_, path)| path.exists()).map(|(name, _)| *name)
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
