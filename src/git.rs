use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const MAX_DIFF_BYTES: usize = 4 * 1024 * 1024;
const FETCH_TIMEOUT: Duration = Duration::from_secs(90);
const REF_KINDS: [(&str, &str); 3] = [("head", "refs/heads/"), ("remote", "refs/remotes/"), ("tag", "refs/tags/")];
const REF_GROUPS: [(&str, &str); 3] = [("@local", "--branches"), ("@remotes", "--remotes"), ("@tags", "--tags")];

pub struct Repo {
    pub root: PathBuf,
}

impl Repo {
    pub fn open(dir: &Path) -> Option<Repo> {
        let top = git(dir, &["rev-parse", "--show-toplevel"]).ok()?;
        Some(Repo { root: PathBuf::from(top.trim()) })
    }

    fn git(&self, args: &[&str]) -> Result<String, String> {
        git(&self.root, args)
    }

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
        let out = self.git(&[
            "for-each-ref",
            "--sort=-creatordate",
            "--exclude=refs/remotes/*/HEAD",
            "--format=%(refname)%1f%(objectname)%1f%(*objectname)%1f%(creatordate:unix)",
            "refs/heads",
            "refs/remotes",
            "refs/tags",
        ])?;
        let refs = out
            .lines()
            .filter_map(|line| {
                let f: Vec<&str> = line.split('\x1f').collect();
                let (kind, name) = REF_KINDS.iter().find_map(|(k, p)| f[0].strip_prefix(p).map(|n| (*k, n)))?;
                let hash = if f[2].is_empty() { f[1] } else { f[2] };
                Some(json!({ "name": name, "kind": kind, "hash": hash, "date": f[3].parse::<i64>().unwrap_or(0) }))
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
            .map(|r| r.trim_start_matches('\n'))
            .filter(|r| !r.is_empty())
            .map(|r| {
                let f: Vec<&str> = r.splitn(5, '\x1f').collect();
                json!({
                    "h": f[0],
                    "p": f[1].split_whitespace().collect::<Vec<_>>(),
                    "a": f[2],
                    "t": f[3].parse::<i64>().unwrap_or(0),
                    "s": f.get(4).copied().unwrap_or(""),
                })
            })
            .collect();
        Ok(Value::Array(commits))
    }

    pub fn fingerprint(&self) -> Result<String, String> {
        let refs = self.git(&["for-each-ref", "--format=%(HEAD)%(objectname)%(refname)"])?;
        let head = self.git(&["rev-parse", "--verify", "-q", "HEAD"]).unwrap_or_default();
        let mut hasher = DefaultHasher::new();
        (refs, head).hash(&mut hasher);
        Ok(format!("{:016x}", hasher.finish()))
    }

    pub fn fetch(&self) -> Result<String, String> {
        self.network(&["fetch", "--all", "--prune", "--quiet"])
    }

    pub fn checkout(&self, name: &str) -> Result<String, String> {
        if self.has_ref(&format!("refs/heads/{name}")) {
            self.git(&["switch", "--quiet", "--no-guess", name])?;
            return Ok(format!("switched to {name}"));
        }
        let (_, branch) = self.split_remote_branch(name)?;
        if self.has_ref(&format!("refs/heads/{branch}")) {
            self.git(&["switch", "--quiet", "--no-guess", branch])?;
            return Ok(format!("switched to the existing local {branch}"));
        }
        self.git(&["switch", "--quiet", "--create", branch, "--track", name])?;
        Ok(format!("created {branch} tracking {name} and switched to it"))
    }

    pub fn delete_remote_branch(&self, name: &str) -> Result<String, String> {
        let (remote, branch) = self.split_remote_branch(name)?;
        self.network(&["push", "--quiet", &remote, "--delete", &format!("refs/heads/{branch}")])?;
        Ok(format!("deleted {branch} on {remote}"))
    }

    fn has_ref(&self, full: &str) -> bool {
        !full.contains("/-") && self.git(&["show-ref", "--verify", "--quiet", full]).is_ok()
    }

    fn split_remote_branch<'a>(&self, name: &'a str) -> Result<(String, &'a str), String> {
        if !self.has_ref(&format!("refs/remotes/{name}")) {
            return Err(format!("no branch named {name}"));
        }
        self.git(&["remote"])?
            .lines()
            .filter_map(|remote| name.strip_prefix(remote).and_then(|r| r.strip_prefix('/')).map(|b| (remote.to_owned(), b)))
            .max_by_key(|(remote, _)| remote.len())
            .ok_or_else(|| format!("{name} belongs to no configured remote"))
    }

    fn network(&self, args: &[&str]) -> Result<String, String> {
        let mut child = command()
            .arg("-C")
            .arg(&self.root)
            .args(["-c", "core.sshCommand=ssh -o BatchMode=yes -o ConnectTimeout=15"])
            .args(args)
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        let started = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                break status;
            }
            if started.elapsed() > FETCH_TIMEOUT {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("git {} timed out after {}s", args[0], FETCH_TIMEOUT.as_secs()));
            }
            thread::sleep(Duration::from_millis(100));
        };
        let mut stderr = String::new();
        if let Some(mut pipe) = child.stderr.take() {
            let _ = pipe.read_to_string(&mut stderr);
        }
        if status.success() { Ok(stderr.trim().to_owned()) } else { Err(stderr.trim().to_owned()) }
    }

    pub fn show(&self, hash: &str) -> Result<String, String> {
        if !(4..=64).contains(&hash.len()) || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err("invalid hash".into());
        }
        let mut text = self.git(&[
            "show",
            "--diff-merges=first-parent",
            "--format=fuller",
            "--stat=200",
            "--summary",
            "--patch",
            "-M",
            hash,
            "--",
        ])?;
        if text.len() > MAX_DIFF_BYTES {
            let mut cut = MAX_DIFF_BYTES;
            while !text.is_char_boundary(cut) {
                cut -= 1;
            }
            text.truncate(cut);
            text.push_str("\n\n[branchsight: diff truncated]\n");
        }
        Ok(text)
    }
}

#[cfg(windows)]
fn command() -> Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut command = Command::new("git");
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

#[cfg(not(windows))]
fn command() -> Command {
    Command::new("git")
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = command()
        .arg("-C")
        .arg(dir)
        .args(["-c", "core.quotepath=off", "-c", "color.ui=never"])
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_owned())
    }
}
