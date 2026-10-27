# What BranchSight changes, and how it stays safe

BranchSight is a viewer first. Reading never changes a repository; six actions do, and only when you ask:

| Action | Changes | Can you undo it? |
|---|---|---|
| fetch | remote-tracking branches; `--prune` removes those deleted on the remote | yes, fetch again |
| checkout | your working copy and HEAD; git refuses if uncommitted work would be lost | yes, check out the previous branch |
| merge | your current branch and working copy; git refuses if uncommitted work would be overwritten | yes: `git merge --abort` while it is in progress, `git reset --hard ORIG_HEAD` right after it is committed |
| push | the branch on the remote; normal by default, force with lease or force only when you pick it | a normal push only adds commits; force with lease refuses if someone pushed since your last fetch; force asks once more, since what it overwrites is lost |
| delete remote branch | the branch on the remote, for everyone | not from BranchSight; asks for confirmation first |
| (nothing else) | BranchSight never commits on its own, resets or rebases; a merge commit is git's, made when you ask for one | |

## Inputs are checked before git sees them

A branch, tag or commit name from the page reaches git only after BranchSight has verified that it exists as that
kind of ref, remote or commit present in the repository, and every name is passed after `--end-of-options`, so git
never reads one as an option. git runs without a terminal, with its password prompts disabled and ssh in batch mode
(unless you set your own ssh command), so a request does not wait on a password; your credential helper, if it has
a window of its own, is the one exception. The end-to-end tests send option-like names such as `--orphan=x`, `--output=…` and
`--receive-pack=x` and check that they are refused.

## Time limits and prompts

These apply to the desktop app and to `branchsight serve` alike.

- git never prompts on a terminal or through an askpass program, and ssh runs in batch mode with a 15 s connect
  timeout unless you configured your own ssh command.
- A credential helper with its own window, such as Git Credential Manager, can still open it when you fetch, push
  or delete by hand; the automatic fetch never asks.
- A fetch slower than 1 KB/s for a minute is dropped.
- Reads stop after 2 minutes; fetch, checkout, merge, push and delete after 15.
- On Linux and macOS a stopped git first gets three seconds to remove its lock files. On Windows it is stopped at
  once: if a later command reports `index.lock: File exists`, delete that file once no git is running.

## Only your own machine can talk to it

The desktop app has no network port at all. `branchsight serve` listens on `127.0.0.1` and `[::1]` only and answers
only requests whose `Host` is a loopback name, so a web page you visit cannot reach it by pointing a domain name at
your machine (DNS rebinding). Every API call needs an `X-BranchSight` header, and actions also need `POST`. A page on
another site cannot add that header without the browser asking the server first, and BranchSight never says yes,
so a link, an image or a hidden form can neither change anything nor make git run in a folder it names: a
downloaded repository's own configuration could otherwise run commands. The page also ships a Content Security
Policy that only allows its own scripts.

## Your credentials stay yours

Fetching and deleting a remote branch use git's own credential handling: your SSH agent, your credential manager.
BranchSight does not store, read or transmit credentials.
