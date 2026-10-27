use std::collections::HashMap;

use serde_json::{Value, json};

use super::run::{self, Call, DIFF_LIMIT, Output};
use super::{Repo, patch};

const WHOLE_FILE: &str = "--unified=1000000";

#[derive(Clone, Copy)]
pub enum FileDiff<'a> {
    Commit(&'a str),
    Compare(&'a str, &'a str),
    Staged,
    Unstaged,
    Untracked,
}

impl Repo {
    pub fn show(&self, hash: &str) -> Result<Value, String> {
        let source = self.source(FileDiff::Commit(hash))?;
        let header = self.git(&["show", "--no-patch", "--format=fuller", hash, "--"])?;
        let mut changes = self.changes(&source)?;
        changes["header"] = json!(header);
        Ok(changes)
    }

    pub fn compare(&self, from: &str, to: &str) -> Result<Value, String> {
        self.changes(&self.source(FileDiff::Compare(from, to))?)
    }

    pub fn worktree(&self) -> Result<Value, String> {
        let untracked = self.git(&["ls-files", "--others", "--exclude-standard", "-z"])?;
        Ok(json!({
            "staged": self.changes(&self.source(FileDiff::Staged)?)?,
            "unstaged": self.changes(&self.source(FileDiff::Unstaged)?)?,
            "untracked": untracked.split('\0').filter(|p| !p.is_empty()).collect::<Vec<_>>(),
        }))
    }

    pub fn file_diff(&self, which: FileDiff, paths: &[String]) -> Result<Value, String> {
        if paths.is_empty() || paths.iter().any(String::is_empty) {
            return Err("a file path is required".into());
        }
        let specs: Vec<String> = paths.iter().map(|p| format!(":(literal){p}")).collect();
        let output = match which {
            FileDiff::Untracked => self.untracked_diff(&paths[0], &specs[0])?,
            _ => {
                let mut source = self.source(which)?;
                if let FileDiff::Unstaged = which {
                    source.push("--ours");
                }
                let specs: Vec<&str> = specs.iter().map(String::as_str).collect();
                self.capped(&Call::new(&[&source[..], &["--no-ext-diff", "-M", WHOLE_FILE, "--"], &specs[..]].concat()))?
            }
        };
        Ok(json!({ "patch": output.text, "truncated": output.truncated }))
    }

    fn source<'a>(&self, which: FileDiff<'a>) -> Result<Vec<&'a str>, String> {
        Ok(match which {
            FileDiff::Commit(hash) => {
                self.check_commit(hash)?;
                vec!["show", "--format=", "--diff-merges=first-parent", hash]
            }
            FileDiff::Compare(from, to) => {
                self.check_commit(from)?;
                self.check_commit(to)?;
                vec!["diff", from, to]
            }
            FileDiff::Staged => vec!["diff", "--cached"],
            FileDiff::Unstaged => vec!["diff"],
            FileDiff::Untracked => return Err("an untracked file has no history to diff against".into()),
        })
    }

    fn untracked_diff(&self, path: &str, spec: &str) -> Result<Output, String> {
        let listed = self.git(&["ls-files", "--others", "--exclude-standard", "-z", "--", spec])?;
        if !listed.split('\0').any(|p| p == path) {
            return Err(format!("{path} is not an untracked file"));
        }
        let args = ["diff", "--no-ext-diff", "--no-index", WHOLE_FILE, "--", "/dev/null", path];
        let output = self.capped(&Call { accepted_exit: 1, ..Call::new(&args) })?;
        if output.text.is_empty() {
            return Err(format!("{path} cannot be read as a file"));
        }
        Ok(output)
    }

    fn changes(&self, source: &[&str]) -> Result<Value, String> {
        let with = |options: &[&'static str]| [source, options, &["--"]].concat();
        let raw = self.git(&with(&["-M", "--raw", "-z"]))?;
        let patch = self.capped(&Call::new(&with(&["-M", "--no-ext-diff", "--patch"])))?;
        let entries = patch::entries(&raw);
        let bodies = patch::split(&patch.text, &entries);
        let exact = if patch.truncated { patch::numstat(&self.git(&with(&["-M", "--numstat", "-z"]))?) } else { HashMap::new() };
        let files: Vec<Value> = entries
            .into_iter()
            .zip(bodies)
            .map(|(entry, body)| {
                let counts = exact.get(&entry.path).copied().unwrap_or_else(|| patch::counts(&body));
                patch::file(entry, body, counts)
            })
            .collect();
        Ok(json!({ "files": files, "truncated": patch.truncated }))
    }

    fn capped(&self, call: &Call) -> Result<Output, String> {
        run::run(&self.root, &Call { limit: Some(DIFF_LIMIT), ..*call })
    }
}
