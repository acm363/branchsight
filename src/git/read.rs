use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;
use std::time::UNIX_EPOCH;

use serde_json::{Value, json};

use super::Repo;

const REF_KINDS: [(&str, &str); 3] = [("head", "refs/heads/"), ("remote", "refs/remotes/"), ("tag", "refs/tags/")];
const REF_GROUPS: [(&str, &str); 3] = [("@local", "--branches"), ("@remotes", "--remotes"), ("@tags", "--tags")];

struct Change<'a> {
    path: &'a str,
    staged: bool,
    unstaged: bool,
    untracked: bool,
}

impl Repo {
    pub fn info(&self) -> Result<Value, String> {
        let head = self.git(&["rev-parse", "--verify", "-q", "HEAD"]).unwrap_or_default();
        let branch = self.git(&["branch", "--show-current"])?;
        Ok(json!({
            "path": self.root.to_string_lossy(),
            "name": self.root.file_name().map(|n| n.to_string_lossy().into_owned()),
            "head": head.trim(),
            "branch": branch.trim(),
            "remotes": self.remotes()?,
        }))
    }

    pub fn refs(&self) -> Result<Value, String> {
        let remotes = self.remotes()?;
        let out = self.git(&[
            "for-each-ref",
            "--sort=-creatordate",
            "--format=%(refname)%1f%(symref)%1f%(objectname)%1f%(*objectname)%1f%(creatordate:unix)%1f%(upstream:remotename)%1f%(upstream:lstrip=2)",
            "refs/heads",
            "refs/remotes",
            "refs/tags",
        ])?;
        let refs = out
            .lines()
            .filter_map(|line| {
                let [refname, symref, object, peeled, date, upstream_remote, upstream] = line.split('\x1f').collect::<Vec<_>>()[..] else {
                    return None;
                };
                if !symref.is_empty() {
                    return None;
                }
                let (kind, name) = REF_KINDS.iter().find_map(|(k, prefix)| refname.strip_prefix(prefix).map(|n| (*k, n)))?;
                let hash = if peeled.is_empty() { object } else { peeled };
                let mut entry = json!({ "name": name, "kind": kind, "hash": hash, "date": date.parse::<i64>().unwrap_or(0) });
                if !upstream.is_empty() {
                    entry["upstream"] = json!(upstream);
                    entry["upstreamRemote"] = json!(upstream_remote);
                }
                if let Some((remote, branch)) = (kind == "remote").then(|| split_remote(&remotes, name)).flatten() {
                    entry["remote"] = json!(remote);
                    entry["branch"] = json!(branch);
                }
                Some(entry)
            })
            .collect();
        Ok(Value::Array(refs))
    }

    pub fn log(&self, refs: &[String], limit: usize) -> Result<Value, String> {
        let history = self.history()?;
        let limit = format!("--max-count={limit}");
        let mut args = vec!["log", "--date-order", &limit, "--format=%H%x1f%P%x1f%an%x1f%at%x1f%s%x1e"];
        let (mut revisions, mut grouped) = (Vec::new(), false);
        for name in refs {
            match REF_GROUPS.iter().find(|(group, _)| group == name) {
                Some((_, option)) => {
                    args.push(option);
                    grouped = true;
                }
                None => revisions.push(name.as_str()),
            }
        }
        args.push("--end-of-options");
        if refs.is_empty() {
            revisions.push("HEAD");
        }
        if revisions.contains(&"HEAD") && self.unborn() {
            revisions.retain(|name| *name != "HEAD");
        }
        if !grouped && revisions.is_empty() {
            return Ok(json!({ "history": history, "commits": [] }));
        }
        args.extend(revisions);
        args.push("--");
        let out = self.git(&args)?;
        let commits = out
            .split('\x1e')
            .map(|record| record.trim_start_matches('\n'))
            .filter(|record| !record.is_empty())
            .map(|record| {
                let f: Vec<&str> = record.splitn(5, '\x1f').collect();
                json!({
                    "h": f[0],
                    "p": f.get(1).map_or(Vec::new(), |p| p.split_whitespace().collect::<Vec<_>>()),
                    "a": f.get(2).copied().unwrap_or(""),
                    "t": f.get(3).and_then(|t| t.parse::<i64>().ok()).unwrap_or(0),
                    "s": f.get(4).copied().unwrap_or(""),
                })
            })
            .collect::<Vec<_>>();
        Ok(json!({ "history": history, "commits": commits }))
    }

    pub fn state(&self) -> Result<Value, String> {
        let porcelain = self.git(&["status", "--porcelain=v2", "-z", "--untracked-files=all"]).ok();
        let (worktree, status) = porcelain
            .as_deref()
            .map(|out| {
                let changes = changes(out);
                let stamps: Vec<(u128, u64)> = changes.iter().map(|change| stamp(&self.root.join(change.path))).collect();
                (digest(&(out, stamps)), counts(&changes))
            })
            .unzip();
        Ok(json!({ "history": self.history()?, "worktree": worktree, "status": status }))
    }

    fn unborn(&self) -> bool {
        self.git(&["rev-parse", "--verify", "-q", "HEAD"]).is_err()
            && self.git(&["symbolic-ref", "-q", "HEAD"]).is_ok_and(|target| self.git(&["show-ref", "--verify", "-q", target.trim()]).is_err())
    }

    fn history(&self) -> Result<String, String> {
        let refs = self.git(&["for-each-ref", "--format=%(HEAD)%(objectname)%(refname)"])?;
        let head = self.git(&["rev-parse", "--verify", "-q", "HEAD"]).unwrap_or_default();
        Ok(digest(&(refs, head, stamp(&self.shallow))))
    }

    pub(super) fn remotes(&self) -> Result<Vec<String>, String> {
        Ok(self.git(&["remote"])?.lines().map(str::to_owned).collect())
    }
}

fn stamp(path: &Path) -> (u128, u64) {
    fs::symlink_metadata(path).map_or((0, 0), |meta| {
        let modified = meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos());
        (modified, meta.len())
    })
}

pub(super) fn split_remote<'a>(remotes: &[String], name: &'a str) -> Option<(String, &'a str)> {
    remotes
        .iter()
        .filter_map(|remote| name.strip_prefix(remote.as_str()).and_then(|rest| rest.strip_prefix('/')).map(|branch| (remote.clone(), branch)))
        .max_by_key(|(remote, _)| remote.len())
}

fn changes(porcelain: &str) -> Vec<Change<'_>> {
    let mut entries = porcelain.split('\0').filter(|entry| !entry.is_empty());
    let mut changes = Vec::new();
    while let Some(entry) = entries.next() {
        let field_count = match entry.as_bytes()[0] {
            b'1' => 9,
            b'2' => 10,
            b'u' => 11,
            b'?' => {
                changes.push(Change { path: &entry[2..], staged: false, unstaged: false, untracked: true });
                continue;
            }
            _ => continue,
        };
        if field_count == 10 {
            entries.next();
        }
        let fields: Vec<&str> = entry.splitn(field_count, ' ').collect();
        let (Some(xy), Some(path)) = (fields.get(1).map(|xy| xy.as_bytes()), fields.get(field_count - 1)) else { continue };
        let changed = |i: usize| xy.get(i).is_some_and(|b| *b != b'.');
        changes.push(Change { path, staged: changed(0), unstaged: changed(1), untracked: false });
    }
    changes
}

fn counts(changes: &[Change]) -> Value {
    let count = |f: fn(&Change) -> bool| changes.iter().filter(|c| f(c)).count();
    json!({ "staged": count(|c| c.staged), "unstaged": count(|c| c.unstaged), "untracked": count(|c| c.untracked) })
}

fn digest(value: &impl Hash) -> String {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}
