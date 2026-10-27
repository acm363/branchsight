use super::Repo;
use super::read::split_remote;
use super::run::{self, Call, NETWORK_TIMEOUT};

const PUSH_MODES: [(&str, Option<&str>); 3] = [("normal", None), ("force-with-lease", Some("--force-with-lease")), ("force", Some("--force"))];

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

    pub fn push(&self, branch: &str, remote: &str, mode: &str) -> Result<String, String> {
        let (_, flag) = PUSH_MODES.iter().find(|(name, _)| *name == mode).ok_or(format!("unknown push mode {mode}"))?;
        if !self.has_ref(&format!("refs/heads/{branch}")) {
            return Err(format!("no local branch named {branch}"));
        }
        if !self.remotes()?.iter().any(|r| r == remote) {
            return Err(format!("no remote named {remote}"));
        }
        let tracked = self.git(&["rev-parse", "--verify", "--quiet", "--end-of-options", &format!("{branch}@{{upstream}}")]).is_ok();
        let refspec = format!("refs/heads/{branch}:refs/heads/{branch}");
        let mut args = vec!["push", "--quiet"];
        args.extend(flag);
        args.extend((!tracked).then_some("--set-upstream"));
        args.extend(["--end-of-options", remote, &refspec]);
        self.network(&args).map_err(|e| match e {
            e if e.contains("(stale info)") => format!("someone pushed to {remote}/{branch} since your last fetch: fetch and look first"),
            e if e.contains("[rejected]") => format!("{remote}/{branch} has commits you do not have: fetch them, or push with lease"),
            e => e,
        })?;
        Ok(match flag {
            Some(_) => format!("pushed {branch} to {remote} ({mode})"),
            None => format!("pushed {branch} to {remote}"),
        })
    }

    fn switch(&self, args: &[&str]) -> Result<(), String> {
        let args = [&["switch", "--quiet"], args].concat();
        run::run(&self.root, &Call { timeout: None, ..Call::new(&args) }).map(drop)
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
        run::run(&self.root, &Call { timeout: Some(NETWORK_TIMEOUT), ..Call::new(args) }).map(|output| output.text.trim().to_owned())
    }
}
