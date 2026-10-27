use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
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
        }))
    }

    pub fn refs(&self) -> Result<Value, String> {
        let remotes = self.remotes()?;
        let out = self.git(&[
            "for-each-ref",
            "--sort=-creatordate",
            "--format=%(refname)%1f%(symref)%1f%(objectname)%1f%(*objectname)%1f%(creatordate:unix)",
            "refs/heads",
            "refs/remotes",
            "refs/tags",
        ])?;
        let refs = out
            .lines()
            .filter_map(|line| {
                let [refname, symref, object, peeled, date] = line.split('\x1f').collect::<Vec<_>>()[..] else { return None };
                if !symref.is_empty() {
                    return None;
                }
                let (kind, name) = REF_KINDS.iter().find_map(|(k, prefix)| refname.strip_prefix(prefix).map(|n| (*k, n)))?;
                let hash = if peeled.is_empty() { object } else { peeled };
                let mut entry = json!({ "name": name, "kind": kind, "hash": hash, "date": date.parse::<i64>().unwrap_or(0) });
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
        let limit = format!("--max-count={limit}");
        let mut args = vec!["log", "--date-order", &limit, "--format=%H%x1f%P%x1f%an%x1f%at%x1f%s%x1e"];
        let (groups, names): (Vec<&str>, Vec<&str>) = refs.iter().map(String::as_str).partition(|r| r.starts_with('@'));
        for group in groups {
            let (_, option) = REF_GROUPS.iter().find(|(g, _)| *g == group).ok_or(format!("unknown ref group {group}"))?;
            args.push(option);
        }
        args.push("--end-of-options");
        if refs.is_empty() {
            args.push("HEAD");
        }
        args.extend(names);
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
            .collect();
        Ok(Value::Array(commits))
    }

    pub fn state(&self) -> Result<Value, String> {
        let refs = self.git(&["for-each-ref", "--format=%(HEAD)%(objectname)%(refname)"])?;
        let head = self.git(&["rev-parse", "--verify", "-q", "HEAD"]).unwrap_or_default();
        let worktree = self.porcelain().ok();
        let status = worktree.as_deref().map(|out| counts(&changes(out)));
        let worktree = worktree.map(|out| self.worktree_digest(&out));
        Ok(json!({ "history": digest(&(refs, head)), "worktree": worktree, "status": status }))
    }

    pub fn status(&self) -> Result<Value, String> {
        Ok(counts(&changes(&self.porcelain()?)))
    }

    pub(super) fn remotes(&self) -> Result<Vec<String>, String> {
        Ok(self.git(&["remote"])?.lines().map(str::to_owned).collect())
    }

    fn porcelain(&self) -> Result<String, String> {
        self.git(&["status", "--porcelain=v2", "-z", "--untracked-files=normal"])
    }

    fn worktree_digest(&self, porcelain: &str) -> String {
        let stamps: Vec<(u128, u64)> = changes(porcelain)
            .iter()
            .map(|change| {
                fs::symlink_metadata(self.root.join(change.path)).map_or((0, 0), |meta| {
                    let modified = meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos());
                    (modified, meta.len())
                })
            })
            .collect();
        digest(&(porcelain, stamps))
    }
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
