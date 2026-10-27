use std::env;

use super::Repo;
use super::read::split_remote;
use super::run::{self, Call, NETWORK_TIMEOUT};

const NON_INTERACTIVE_SSH: &str = "core.sshCommand=ssh -o BatchMode=yes -o ConnectTimeout=15";

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
        self.network(&["push", "--quiet", "--delete", "--end-of-options", &remote, &format!("refs/heads/{branch}")])?;
        Ok(format!("deleted {branch} on {remote}"))
    }

    fn switch(&self, args: &[&str]) -> Result<(), String> {
        self.git(&[&["switch", "--quiet"], args].concat()).map(drop)
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
        let config = self.uses_default_ssh().then_some(NON_INTERACTIVE_SSH);
        run::run(&self.root, &Call { config, timeout: NETWORK_TIMEOUT, ..Call::new(args) }).map(|output| output.text.trim().to_owned())
    }

    fn uses_default_ssh(&self) -> bool {
        let from_env = env::var_os("GIT_SSH_COMMAND").is_some() || env::var_os("GIT_SSH").is_some();
        let from_config = self.git(&["config", "--get", "core.sshCommand"]).is_ok_and(|value| !value.trim().is_empty());
        !from_env && !from_config
    }
}
