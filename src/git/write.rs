use std::env;

use super::Repo;
use super::read::split_remote;
use super::run::{self, ACTION_TIMEOUT, Call};

const NON_INTERACTIVE_SSH: &str = "core.sshCommand=ssh -o BatchMode=yes -o ConnectTimeout=15";
const STALLED_HTTP: [&str; 2] = ["http.lowSpeedLimit=1000", "http.lowSpeedTime=60"];

impl Repo {
    pub fn fetch(&self) -> Result<String, String> {
        self.network(&["fetch", "--all", "--prune", "--quiet"])
    }

    pub fn checkout(&self, name: &str) -> Result<String, String> {
        if self.has_ref(&format!("refs/heads/{name}")) {
            self.switch(&["--no-guess", "--end-of-options", name])?;
            return Ok(format!("switched to {name}"));
        }
        let tag = format!("refs/tags/{name}");
        if self.has_ref(&tag) {
            self.switch(&["--detach", "--end-of-options", &tag])?;
            return Ok(format!("detached HEAD at tag {name}"));
        }
        if self.check_commit(name).is_ok() {
            self.switch(&["--detach", "--end-of-options", name])?;
            return Ok(format!("detached HEAD at {}", name.get(..10).unwrap_or(name)));
        }
        let (_, branch) = self.split_remote_branch(name)?;
        if self.has_ref(&format!("refs/heads/{branch}")) {
            self.switch(&["--no-guess", "--end-of-options", branch])?;
            return Ok(format!("switched to the existing local {branch}"));
        }
        self.switch(&["--create", branch, "--track", "--end-of-options", name])?;
        Ok(format!("created {branch} tracking {name} and switched to it"))
    }

    pub fn delete_remote_branch(&self, name: &str) -> Result<String, String> {
        let (remote, branch) = self.split_remote_branch(name)?;
        self.network(&["push", "--quiet", "--delete", "--end-of-options", &remote, &format!("refs/heads/{branch}")]).map_err(main_reason)?;
        Ok(format!("deleted {branch} on {remote}"))
    }

    pub fn push(&self, branch: &str, remote: &str, mode: &str) -> Result<String, String> {
        let flag = match mode {
            "normal" => None,
            "force-with-lease" => Some("--force-with-lease"),
            "force" => Some("--force"),
            other => return Err(format!("unknown push mode {other}")),
        };
        let local = format!("refs/heads/{branch}");
        if !self.has_ref(&local) {
            return Err(format!("no local branch named {branch}"));
        }
        if !self.remotes()?.iter().any(|r| r == remote) {
            return Err(format!("no remote named {remote}"));
        }
        let upstream = self.git(&["for-each-ref", "--format=%(upstream:remotename)%1f%(upstream:remoteref)", &local])?;
        let tracks_it = upstream.trim_end() == format!("{remote}\x1f{local}");
        let refspec = format!("{local}:{local}");
        let mut args = vec!["push", "--quiet"];
        args.extend(flag);
        args.extend((!tracks_it).then_some("--set-upstream"));
        args.extend(["--end-of-options", remote, &refspec]);
        self.network(&args).map_err(|e| match e {
            e if e.contains("(stale info)") => format!("someone pushed to {remote}/{branch} since your last fetch: fetch and look first"),
            e if e.contains("[rejected]") => format!("{remote}/{branch} has commits you do not have: fetch them, or push with lease"),
            e => main_reason(e),
        })?;
        let how = if mode == "normal" { String::new() } else { format!(" ({mode})") };
        let tracking = if tracks_it { String::new() } else { format!("; it now tracks {remote}/{branch}") };
        Ok(format!("pushed {branch} to {remote}{how}{tracking}"))
    }

    fn switch(&self, args: &[&str]) -> Result<(), String> {
        let args = [&["switch", "--quiet"], args].concat();
        run::run(&self.root, &Call { timeout: ACTION_TIMEOUT, ..Call::new(&args) }).map(drop)
    }

    fn has_ref(&self, full: &str) -> bool {
        self.git(&["show-ref", "--verify", "--quiet", full]).is_ok()
    }

    fn split_remote_branch<'a>(&self, name: &'a str) -> Result<(String, &'a str), String> {
        if !self.has_ref(&format!("refs/remotes/{name}")) {
            return Err(format!("no branch named {name}"));
        }
        split_remote(&self.remotes()?, name).ok_or_else(|| format!("{name} belongs to no configured remote"))
    }

    fn network(&self, args: &[&str]) -> Result<String, String> {
        let mut config = STALLED_HTTP.to_vec();
        config.extend(self.uses_default_ssh().then_some(NON_INTERACTIVE_SSH));
        run::run(&self.root, &Call { config: &config, timeout: ACTION_TIMEOUT, ..Call::new(args) }).map(|output| output.text.trim().to_owned())
    }

    fn uses_default_ssh(&self) -> bool {
        let from_env = env::var_os("GIT_SSH_COMMAND").is_some() || env::var_os("GIT_SSH").is_some();
        !from_env && self.git(&["config", "--get", "core.sshCommand"]).is_err()
    }
}

fn main_reason(error: String) -> String {
    let noise = |line: &&str| line.starts_with("To ") || line.starts_with("hint:") || line.starts_with("error: failed to push some refs");
    let kept: Vec<&str> = error.lines().filter(|line| !noise(line)).collect();
    if kept.is_empty() { error } else { kept.join("\n") }
}
