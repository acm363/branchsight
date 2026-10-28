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
        self.switch(&["--no-guess", "--end-of-options", branch])?;
        let behind = self.git(&["merge-base", "--is-ancestor", "--end-of-options", remote, "HEAD"]).is_err();
        Ok(if behind {
            format!("switched to {branch}; {remote} has commits it does not have: sync to bring them in")
        } else {
            format!("switched to {branch}")
        })
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

    pub fn stage(&self, paths: &[String], all: bool) -> Result<String, String> {
        if all {
            self.git(&["add", "--all"]).map_err(main_reason)?;
            return Ok("staged every change".into());
        }
        let specs = literal_specs(paths)?;
        let refs: Vec<&str> = specs.iter().map(String::as_str).collect();
        self.git(&[&["add", "--"], &refs[..]].concat()).map_err(main_reason)?;
        Ok(listed("staged", paths))
    }

    pub fn unstage(&self, paths: &[String], all: bool) -> Result<String, String> {
        let target = if all { vec![".".to_owned()] } else { literal_specs(paths)? };
        let refs: Vec<&str> = target.iter().map(String::as_str).collect();
        let head = if self.unborn() { &["rm", "--cached", "--quiet", "-r", "--"][..] } else { &["reset", "--quiet", "--"][..] };
        self.git(&[head, &refs[..]].concat()).map_err(main_reason)?;
        Ok(if all { "unstaged everything".into() } else { listed("unstaged", paths) })
    }

    pub fn discard(&self, paths: &[String], all: bool) -> Result<String, String> {
        let specs = if all { vec![".".to_owned()] } else { literal_specs(paths)? };
        let born = !self.unborn();
        for spec in &specs {
            if born {
                if let Err(e) = self.git(&["restore", "--source=HEAD", "--staged", "--worktree", "--", spec]) {
                    if !e.contains("did not match") {
                        return Err(main_reason(e));
                    }
                }
            } else {
                self.git(&["rm", "--cached", "--quiet", "-r", "--ignore-unmatch", "--", spec]).ok();
            }
        }
        let refs: Vec<&str> = specs.iter().map(String::as_str).collect();
        self.git(&[&["clean", "--quiet", "-fd", "--"][..], &refs[..]].concat()).map_err(main_reason)?;
        Ok(if all { "discarded every change".into() } else { listed("discarded", paths) })
    }

    pub fn commit(&self, message: &str, amend: bool) -> Result<String, String> {
        let message = message.trim();
        if message.is_empty() {
            return Err("a commit message is required".into());
        }
        if self.operation() == Some("rebase") {
            return Err("a rebase is in progress: continue or abort it, then commit".into());
        }
        let mut args = vec!["commit", "--message", message];
        if amend {
            if self.unborn() {
                return Err("there is no commit to amend".into());
            }
            if self.operation().is_some() {
                return Err("finish the operation in progress before amending".into());
            }
            args.insert(1, "--amend");
        } else if self.operation().is_none() && self.git(&["diff", "--cached", "--quiet"]).is_ok() {
            return Err("nothing is staged to commit".into());
        }
        run::run(&self.root, &Call { timeout: ACTION_TIMEOUT, ..Call::new(&args) }).map_err(main_reason)?;
        let short = self.git(&["rev-parse", "--short", "HEAD"]).map(|s| s.trim().to_owned()).unwrap_or_default();
        Ok(if amend { format!("amended {short}") } else { format!("committed {short}") })
    }

    pub fn create_tag(&self, name: &str, hash: &str, message: &str) -> Result<String, String> {
        self.check_commit(hash)?;
        self.valid_ref_name("tags", name)?;
        if self.has_ref(&format!("refs/tags/{name}")) {
            return Err(format!("tag {name} already exists"));
        }
        let message = message.trim();
        let mut args = vec!["tag"];
        if !message.is_empty() {
            args.extend(["--annotate", "--message", message]);
        }
        args.extend(["--end-of-options", name, hash]);
        self.git(&args).map_err(main_reason)?;
        Ok(format!("created tag {name}"))
    }

    pub fn delete_tag(&self, name: &str) -> Result<String, String> {
        if !self.has_ref(&format!("refs/tags/{name}")) {
            return Err(format!("no tag named {name}"));
        }
        self.git(&["tag", "--delete", "--end-of-options", name]).map_err(main_reason)?;
        Ok(format!("deleted tag {name}"))
    }

    pub fn push_tag(&self, name: &str, remote: &str, delete: bool) -> Result<String, String> {
        if !delete && !self.has_ref(&format!("refs/tags/{name}")) {
            return Err(format!("no tag named {name}"));
        }
        if !self.remotes()?.iter().any(|r| r == remote) {
            return Err(format!("no remote named {remote}"));
        }
        let refspec = if delete { format!("refs/tags/{name}") } else { format!("refs/tags/{name}:refs/tags/{name}") };
        let mut args = vec!["push", "--quiet"];
        if delete {
            args.push("--delete");
        }
        args.extend(["--end-of-options", remote, refspec.as_str()]);
        self.network(&args, &[]).map_err(main_reason)?;
        Ok(if delete { format!("deleted tag {name} on {remote}") } else { format!("pushed tag {name} to {remote}") })
    }

    pub fn create_branch(&self, name: &str, hash: &str) -> Result<String, String> {
        self.check_commit(hash)?;
        self.valid_ref_name("heads", name)?;
        if self.has_ref(&format!("refs/heads/{name}")) {
            return Err(format!("branch {name} already exists"));
        }
        self.git(&["branch", "--end-of-options", name, hash]).map_err(main_reason)?;
        let short = self.git(&["rev-parse", "--short", "--end-of-options", hash]).map(|s| s.trim().to_owned()).unwrap_or_else(|_| hash.to_owned());
        Ok(format!("created branch {name} at {short}"))
    }

    fn valid_ref_name(&self, kind: &str, name: &str) -> Result<(), String> {
        let noun = if kind == "tags" { "tag" } else { "branch" };
        if name.is_empty() || name.starts_with('-') {
            return Err(format!("{name:?} is not a valid {noun} name"));
        }
        self.git(&["check-ref-format", &format!("refs/{kind}/{name}")]).map_err(|_| format!("{name:?} is not a valid {noun} name"))?;
        Ok(())
    }

    pub(super) fn switch(&self, args: &[&str]) -> Result<(), String> {
        let args = [&["switch", "--quiet"], args].concat();
        run::run(&self.root, &Call { timeout: ACTION_TIMEOUT, ..Call::new(&args) }).map(drop)
    }

    pub(super) fn has_ref(&self, full: &str) -> bool {
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

fn literal_specs(paths: &[String]) -> Result<Vec<String>, String> {
    if paths.is_empty() || paths.iter().any(String::is_empty) {
        return Err("a file path is required".into());
    }
    Ok(paths.iter().map(|p| format!(":(literal){p}")).collect())
}

fn listed(verb: &str, paths: &[String]) -> String {
    match paths {
        [one] => format!("{verb} {one}"),
        many => format!("{verb} {} files", many.len()),
    }
}

pub(super) fn main_reason(error: String) -> String {
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
