use serde_json::{Value, json};

use super::Repo;
use super::run::{self, Call, DIFF_LIMIT};

const WHOLE_FILE: &str = "--unified=1000000";
const TRUNCATED: &str = "\n[branchsight: diff truncated]\n";

pub enum FileDiff<'a> {
    Commit(&'a str),
    Compare(&'a str, &'a str),
    Staged,
    Unstaged,
    Untracked,
}

impl Repo {
    pub fn show(&self, hash: &str) -> Result<Value, String> {
        self.check_commit(hash)?;
        let header = self.git(&["show", "--no-patch", "--format=fuller", hash, "--"])?;
        let mut changes = self.changes(&["show", "--format=", "--diff-merges=first-parent", hash])?;
        changes["header"] = json!(header);
        Ok(changes)
    }

    pub fn compare(&self, from: &str, to: &str) -> Result<Value, String> {
        self.check_commit(from)?;
        self.check_commit(to)?;
        self.changes(&["diff", from, to])
    }

    pub fn worktree(&self) -> Result<Value, String> {
        let untracked = self.git(&["ls-files", "--others", "--exclude-standard", "-z"])?;
        Ok(json!({
            "staged": self.changes(&["diff", "--cached"])?,
            "unstaged": self.changes(&["diff"])?,
            "untracked": untracked.split('\0').filter(|p| !p.is_empty()).collect::<Vec<_>>(),
        }))
    }

    pub fn file_diff(&self, which: FileDiff, paths: &[String]) -> Result<String, String> {
        if paths.is_empty() || paths.iter().any(String::is_empty) {
            return Err("a file path is required".into());
        }
        let specs: Vec<String> = paths.iter().map(|p| format!(":(literal){p}")).collect();
        let source: Vec<&str> = match which {
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
            FileDiff::Untracked => return self.untracked_diff(&paths[0], &specs[0]),
        };
        let args = [&source[..], &["--no-ext-diff", "-M", WHOLE_FILE, "--"], &specs.iter().map(String::as_str).collect::<Vec<_>>()[..]].concat();
        self.capped(&Call::new(&args))
    }

    fn untracked_diff(&self, path: &str, spec: &str) -> Result<String, String> {
        let listed = self.git(&["ls-files", "--others", "--exclude-standard", "-z", "--", spec])?;
        if !listed.split('\0').any(|p| p == path) {
            return Err(format!("{path} is not an untracked file"));
        }
        let args = ["diff", "--no-ext-diff", "--no-index", WHOLE_FILE, "--", "/dev/null", path];
        self.capped(&Call { accepted_exit: 1, ..Call::new(&args) })
    }

    fn changes(&self, source: &[&str]) -> Result<Value, String> {
        let listing = self.git(&[source, &["-M", "--raw", "--numstat", "-z", "--"]].concat())?;
        let patch_args = [source, &["-M", "--no-ext-diff", "--patch", "--"]].concat();
        let patch = run::run(&self.root, &Call { limit: Some(DIFF_LIMIT), ..Call::new(&patch_args) })?;
        Ok(json!({ "files": files(&listing), "patch": patch.text, "truncated": patch.truncated }))
    }

    fn capped(&self, call: &Call) -> Result<String, String> {
        let output = run::run(&self.root, &Call { limit: Some(DIFF_LIMIT), ..*call })?;
        Ok(if output.truncated { output.text + TRUNCATED } else { output.text })
    }
}

fn files(listing: &str) -> Vec<Value> {
    let mut tokens = listing.split('\0');
    let mut entries = Vec::new();
    let mut counts = Vec::new();
    while let Some(token) = tokens.next() {
        if let Some(raw) = token.strip_prefix(':') {
            let status = raw.rsplit(' ').next().and_then(|s| s.get(..1)).unwrap_or("M");
            let first = tokens.next().unwrap_or_default();
            let (from, path) = if matches!(status, "R" | "C") { (Some(first), tokens.next().unwrap_or_default()) } else { (None, first) };
            entries.push((status, from, path));
        } else if !token.is_empty() {
            let mut fields = token.splitn(3, '\t');
            let (added, deleted, path) = (fields.next(), fields.next(), fields.next().unwrap_or(""));
            if path.is_empty() {
                tokens.next();
                tokens.next();
            }
            counts.push((added.and_then(|n| n.parse::<u64>().ok()), deleted.and_then(|n| n.parse::<u64>().ok())));
        }
    }
    entries
        .into_iter()
        .enumerate()
        .map(|(i, (status, from, path))| {
            let (added, deleted) = counts.get(i).copied().unwrap_or((None, None));
            json!({ "status": status, "path": path, "from": from, "add": added, "del": deleted })
        })
        .collect()
}
