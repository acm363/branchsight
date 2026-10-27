use std::env;

use super::Repo;
use super::read::split_remote;
use super::run::{self, ACTION_TIMEOUT, Call};

const NON_INTERACTIVE_SSH: &str = "core.sshCommand=ssh -o BatchMode=yes -o ConnectTimeout=15";
const STALLED_HTTP: [&str; 2] = ["http.lowSpeedLimit=1000", "http.lowSpeedTime=60"];
const UNATTENDED: [&str; 1] = ["credential.interactive=false"];
const NOISE: [&str; 5] = ["To ", "hint:", "error: failed to push some refs", "Warning: Permanently added", "warning: redirecting to"];
const REASONS: [&str; 4] = [" ! ", "fatal:", "error:", "remote: error"];

impl Repo {
    pub fn fetch(&self, unattended: bool) -> Result<String, String> {
        let quiet = if unattended { &UNATTENDED[..] } else { &[] };
        self.network(&["fetch", "--all", "--prune", "--quiet"], &[&STALLED_HTTP[..], quiet].concat()).map_err(main_reason)
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
            return self.checkout_existing(branch, name);
        }
        self.switch(&["--create", branch, "--track", "--end-of-options", name])?;
        Ok(format!("created {branch} tracking {name} and switched to it"))
    }

    fn checkout_existing(&self, branch: &str, remote: &str) -> Result<String, String> {
        let contains = |a: &str, b: &str| self.git(&["merge-base", "--is-ancestor", "--end-of-options", b, a]).is_ok();
        let local = format!("refs/heads/{branch}");
        let (behind, ahead) = (contains(remote, &local), contains(&local, remote));
        if !behind && !ahead {
            return Err(format!("{branch} and {remote} have diverged: check out {branch} and merge {remote} into it, or check out the commit itself"));
        }
        self.switch(&["--no-guess", "--end-of-options", branch])?;
        if ahead {
            let extra = if behind { "already at" } else { "which is ahead of" };
            return Ok(format!("switched to {branch}, {extra} {remote}"));
        }
        let args = ["merge", "--quiet", "--ff-only", "--end-of-options", remote];
        run::run(&self.root, &Call { timeout: ACTION_TIMEOUT, ..Call::new(&args) })
            .map_err(|e| format!("switched to {branch} but could not bring it up to {remote}: {}", main_reason(e)))?;
        Ok(format!("switched to {branch} and brought it up to {remote}"))
    }

    pub fn delete_remote_branch(&self, name: &str) -> Result<String, String> {
        let (remote, branch) = self.split_remote_branch(name)?;
        self.network(&["push", "--quiet", "--delete", "--end-of-options", &remote, &format!("refs/heads/{branch}")], &[]).map_err(main_reason)?;
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
        self.network(&args, &[]).map_err(|e| match e {
            e if e.contains("(stale info)") => format!("someone pushed to {remote}/{branch} since your last fetch: fetch and look first"),
            e if e.contains("[rejected]") => format!("{remote}/{branch} has commits you do not have: fetch them, or push with lease"),
            e => main_reason(e),
        })?;
        let how = if mode == "normal" { String::new() } else { format!(" ({mode})") };
        let tracking = if tracks_it { String::new() } else { format!("; it now tracks {remote}/{branch}") };
        Ok(format!("pushed {branch} to {remote}{how}{tracking}"))
    }

    pub fn merge(&self, name: &str, mode: &str, commit: bool) -> Result<String, String> {
        let how = match (mode, commit) {
            ("fast-forward", true) => vec![],
            ("fast-forward" | "merge-commit", false) => vec!["--no-ff", "--no-commit"],
            ("merge-commit", true) => vec!["--no-ff"],
            ("squash", _) => vec!["--squash"],
            _ => return Err(format!("unknown merge mode {mode}")),
        };
        self.mergeable(name)?;
        let into = self.git(&["branch", "--show-current"])?.trim().to_owned();
        let into = if into.is_empty() { "HEAD".to_owned() } else { into };
        if into == name {
            return Err(format!("{name} is the branch you are on"));
        }
        if self.git(&["merge-base", "--is-ancestor", "--end-of-options", name, "HEAD"]).is_ok() {
            return Ok(format!("{into} already contains {name}"));
        }
        let args = [&["merge", "--quiet", "--no-edit"], &how[..], &["--end-of-options", name]].concat();
        run::run(&self.root, &Call { timeout: ACTION_TIMEOUT, accepted_exit: 1, ..Call::new(&args) })?;
        let conflicts = self.git(&["diff", "--name-only", "--diff-filter=U", "-z"])?;
        let conflicts: Vec<&str> = conflicts.split('\0').filter(|p| !p.is_empty()).collect();
        if !conflicts.is_empty() {
            return Err(format!("merging {name} stopped on conflicts in {}: resolve them and commit, or run git merge --abort", conflicts.join(", ")));
        }
        Ok(match (mode, commit) {
            ("squash", _) => format!("{name} is squashed into the staged changes of {into}: review and commit"),
            (_, false) => format!("{name} is merged into {into} but not committed: review and commit"),
            _ => format!("merged {name} into {into}"),
        })
    }

    fn mergeable(&self, name: &str) -> Result<(), String> {
        let named = ["refs/heads/", "refs/remotes/", "refs/tags/"].iter().any(|kind| self.has_ref(&format!("{kind}{name}")));
        if named || self.check_commit(name).is_ok() { Ok(()) } else { Err(format!("nothing named {name} to merge")) }
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

    fn network(&self, args: &[&str], extra: &[&str]) -> Result<String, String> {
        let mut config = extra.to_vec();
        config.extend(self.uses_default_ssh().then_some(NON_INTERACTIVE_SSH));
        run::run(&self.root, &Call { config: &config, timeout: ACTION_TIMEOUT, ..Call::new(args) }).map(|output| output.text.trim().to_owned())
    }

    fn uses_default_ssh(&self) -> bool {
        let from_env = env::var_os("GIT_SSH_COMMAND").is_some() || env::var_os("GIT_SSH").is_some();
        !from_env && self.git(&["config", "--get", "core.sshCommand"]).is_err()
    }
}

fn main_reason(error: String) -> String {
    let starts = |line: &str, prefixes: &[&str]| prefixes.iter().any(|p| line.starts_with(p));
    let useful = |line: &&str| !starts(line, &NOISE) && !line.trim_start_matches("remote:").trim_matches(['=', ' ']).is_empty();
    let (mut reasons, details): (Vec<&str>, Vec<&str>) = error.lines().filter(useful).partition(|line| starts(line, &REASONS));
    reasons.extend(details);
    if reasons.is_empty() { error } else { reasons.join("\n") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reason_comes_first_and_noise_is_dropped() {
        // Given.
        let stderr = "Warning: Permanently added 'host' to the list of known hosts.\nremote: =====\nremote: lint failed: 3 errors\nremote:\nTo host:repo.git\n ! [remote rejected] b -> b (pre-receive hook declined)\nerror: failed to push some refs to 'host:repo.git'";

        // When.
        let reason = main_reason(stderr.to_owned());

        // Then.
        assert_eq!(reason, " ! [remote rejected] b -> b (pre-receive hook declined)\nremote: lint failed: 3 errors");
    }

    #[test]
    fn an_error_without_known_lines_is_kept_whole() {
        // Then.
        assert_eq!(main_reason("To host\nhint: retry".to_owned()), "To host\nhint: retry");
    }
}
