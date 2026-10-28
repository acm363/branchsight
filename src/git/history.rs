use super::Repo;
use super::run::{self, ACTION_TIMEOUT, Call};
use super::write::main_reason;

const SHORT: usize = 7;

impl Repo {
    pub fn merge(&self, name: &str, mode: &str, commit: bool) -> Result<String, String> {
        let how = match (mode, commit) {
            ("fast-forward", true) => vec![],
            ("fast-forward" | "merge-commit", false) => vec!["--no-ff", "--no-commit"],
            ("merge-commit", true) => vec!["--no-ff"],
            ("squash", _) => vec!["--squash"],
            _ => return Err(format!("unknown merge mode {mode}")),
        };
        self.named_or_commit(name)?;
        let into = self.current()?;
        if into == name {
            return Err(format!("{name} is the branch you are on"));
        }
        if self.contains("HEAD", name) {
            return Ok(format!("{into} already contains {name}"));
        }
        self.sequence("merge", &[&["merge", "--quiet", "--no-edit"], &how[..], &["--end-of-options", name]].concat())?;
        Ok(match (mode, commit) {
            ("squash", _) => format!("{name} is squashed into the staged changes of {into}: review and commit"),
            (_, false) => format!("{name} is merged into {into} but not committed: review and commit"),
            _ => format!("merged {name} into {into}"),
        })
    }

    pub fn pull(&self, branch: &str, from: &str, mode: &str) -> Result<String, String> {
        if !matches!(mode, "merge" | "rebase" | "fast-forward") {
            return Err(format!("unknown pull mode {mode}"));
        }
        let local = format!("refs/heads/{branch}");
        if !self.has_ref(&local) {
            return Err(format!("no local branch named {branch}"));
        }
        if !self.has_ref(&format!("refs/remotes/{from}")) {
            return Err(format!("no remote branch named {from}"));
        }
        if self.contains(&local, from) {
            return Ok(format!("{branch} already contains {from}"));
        }
        let switching = self.current()? != branch;
        if switching && self.contains(from, &local) {
            self.git(&["fetch", "--quiet", ".", &format!("refs/remotes/{from}:{local}")]).map_err(main_reason)?;
            return Ok(format!("brought {branch} up to {from}"));
        }
        if switching && mode == "fast-forward" {
            return Err(format!("{branch} and {from} have diverged: pull with merge or rebase instead"));
        }
        if switching {
            self.switch(&["--no-guess", "--end-of-options", branch])?;
        }
        let (operation, args, done) = match mode {
            "merge" => ("merge", vec!["merge", "--quiet", "--no-edit"], format!("merged {from} into {branch}")),
            "rebase" => ("rebase", vec!["rebase", "--quiet"], format!("rebased {branch} on {from}")),
            _ => ("merge", vec!["merge", "--quiet", "--ff-only"], format!("brought {branch} up to {from}")),
        };
        self.sequence(operation, &[&args[..], &["--end-of-options", from]].concat())
            .map_err(|e| if e.contains("Not possible to fast-forward") { format!("{branch} and {from} have diverged: pull with merge or rebase instead") } else { e })?;
        Ok(if switching { format!("switched to {branch}, then {done}") } else { done })
    }

    pub fn reset_to_remote(&self, branch: &str, remote: &str) -> Result<String, String> {
        let local = format!("refs/heads/{branch}");
        if !self.has_ref(&local) {
            return Err(format!("no local branch named {branch}"));
        }
        if !self.has_ref(&format!("refs/remotes/{remote}")) {
            return Err(format!("no remote branch named {remote}"));
        }
        if self.current()? == branch {
            let args = ["reset", "--quiet", "--keep", &format!("refs/remotes/{remote}"), "--"];
            run::run(&self.root, &Call { timeout: ACTION_TIMEOUT, ..Call::new(&args) }).map_err(main_reason)?;
        } else {
            self.git(&["branch", "--force", "--end-of-options", branch, &format!("refs/remotes/{remote}")]).map_err(main_reason)?;
        }
        Ok(format!("{branch} now matches {remote}"))
    }

    pub fn cherry_pick(&self, hash: &str, commit: bool) -> Result<String, String> {
        self.check_commit(hash)?;
        let into = self.current()?;
        let mut args = vec!["cherry-pick"];
        args.extend(self.mainline(hash)?);
        args.extend((!commit).then_some("--no-commit"));
        self.sequence("cherry-pick", &[&args[..], &["--end-of-options", hash]].concat())?;
        let short = short(hash);
        Ok(if commit { format!("picked {short} onto {into}") } else { format!("the changes of {short} are staged: review and commit") })
    }

    pub fn revert(&self, hash: &str, commit: bool) -> Result<String, String> {
        self.check_commit(hash)?;
        let into = self.current()?;
        let mut args = vec!["revert", "--no-edit"];
        args.extend(self.mainline(hash)?);
        args.extend((!commit).then_some("--no-commit"));
        self.sequence("revert", &[&args[..], &["--end-of-options", hash]].concat())?;
        let short = short(hash);
        Ok(if commit { format!("reverted {short} on {into}") } else { format!("the revert of {short} is staged: review and commit") })
    }

    pub fn drop_commit(&self, hash: &str) -> Result<String, String> {
        self.check_commit(hash)?;
        let into = self.current()?;
        let short = short(hash);
        if !self.contains("HEAD", hash) {
            return Err(format!("{short} is not in the history of {into}"));
        }
        if !self.mainline(hash)?.is_empty() {
            return Err(format!("{short} is a merge; revert it instead"));
        }
        let parent = format!("{hash}^");
        if self.git(&["rev-parse", "--verify", "-q", &parent]).is_err() {
            return Err(format!("{short} is the first commit; it cannot be dropped"));
        }
        self.sequence("rebase", &["rebase", "--quiet", "--rebase-merges", "--onto", &parent, hash])?;
        Ok(format!("dropped {short} from {into}"))
    }

    pub fn rebase(&self, onto: &str) -> Result<String, String> {
        self.named_or_commit(onto)?;
        let into = self.current()?;
        if into == onto {
            return Err(format!("{onto} is the branch you are on"));
        }
        self.sequence("rebase", &["rebase", "--quiet", "--end-of-options", onto])?;
        Ok(format!("rebased {into} onto {}", short(onto)))
    }

    pub fn reset(&self, hash: &str, mode: &str) -> Result<String, String> {
        if !matches!(mode, "soft" | "mixed" | "hard") {
            return Err(format!("unknown reset mode {mode}"));
        }
        self.check_commit(hash)?;
        let into = self.current()?;
        let flag = format!("--{mode}");
        run::run(&self.root, &Call { timeout: ACTION_TIMEOUT, ..Call::new(&["reset", "--quiet", &flag, hash, "--"]) }).map_err(main_reason)?;
        Ok(format!("reset {into} to {} ({mode})", short(hash)))
    }

    fn sequence(&self, operation: &str, args: &[&str]) -> Result<(), String> {
        let Err(error) = run::run(&self.root, &Call { timeout: ACTION_TIMEOUT, ..Call::new(args) }) else { return Ok(()) };
        let conflicts = self.git(&["diff", "--name-only", "--diff-filter=U", "-z"])?;
        let conflicts: Vec<&str> = conflicts.split('\0').filter(|p| !p.is_empty()).collect();
        if conflicts.is_empty() {
            return Err(main_reason(error));
        }
        let finish = if operation == "merge" { "commit".to_owned() } else { format!("run git {operation} --continue") };
        Err(format!("{operation} stopped on conflicts in {}: resolve them and {finish}, or run git {operation} --abort", conflicts.join(", ")))
    }

    fn current(&self) -> Result<String, String> {
        let branch = self.git(&["branch", "--show-current"])?.trim().to_owned();
        Ok(if branch.is_empty() { "HEAD".to_owned() } else { branch })
    }

    fn contains(&self, descendant: &str, ancestor: &str) -> bool {
        self.git(&["merge-base", "--is-ancestor", "--end-of-options", ancestor, descendant]).is_ok()
    }

    fn mainline(&self, hash: &str) -> Result<Vec<&'static str>, String> {
        let parents = self.git(&["rev-list", "--parents", "-n", "1", "--end-of-options", hash])?;
        Ok(if parents.split_whitespace().count() > 2 { vec!["-m", "1"] } else { vec![] })
    }

    fn named_or_commit(&self, name: &str) -> Result<(), String> {
        let named = ["refs/heads/", "refs/remotes/", "refs/tags/"].iter().any(|kind| self.has_ref(&format!("{kind}{name}")));
        if named || self.check_commit(name).is_ok() { Ok(()) } else { Err(format!("nothing named {name}")) }
    }
}

fn short(name: &str) -> &str {
    let hash = name.len() > SHORT && name.chars().all(|c| c.is_ascii_hexdigit());
    if hash { &name[..SHORT] } else { name }
}
