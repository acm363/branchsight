# What BranchSight changes, and how it stays safe

BranchSight is a viewer first. Reading never changes a repository; four actions do, and only when you ask:

| Action | Changes | Can you undo it? |
|---|---|---|
| fetch | remote-tracking branches; `--prune` removes those deleted on the remote | yes, fetch again |
| checkout | your working copy and HEAD; git refuses if uncommitted work would be lost | yes, check out the previous branch |
| delete remote branch | the branch on the remote, for everyone | not from BranchSight; asks for confirmation first |
| (nothing else) | BranchSight never commits, stages, resets, rebases or force-pushes | |

## Inputs are checked before git sees them

A branch, tag or commit name from the page reaches git only after BranchSight has verified that it exists as that
kind of ref, or is a full commit hash present in the repository. Names starting with `-` are refused, ref names are
passed after `--end-of-options`, and git runs with prompts disabled so a request can never wait on a password.
The end-to-end tests send option-like names such as `--orphan=x` and `--output=…` and check that they are refused.

## Only your own machine can talk to it

The desktop app has no network port at all. `branchsight serve` listens on `127.0.0.1` and `[::1]` only and answers
only requests whose `Host` is a loopback name, so a web page you visit cannot reach it by pointing a domain name at
your machine (DNS rebinding). Actions that change something require `POST` with an `X-BranchSight` header. A page
on another site cannot add that header without the browser asking the server first, and BranchSight never says
yes, so a link, an image or a hidden form cannot check out or delete anything.

## Your credentials stay yours

Fetching and deleting a remote branch use git's own credential handling: your SSH agent, your credential manager.
BranchSight does not store, read or transmit credentials.
