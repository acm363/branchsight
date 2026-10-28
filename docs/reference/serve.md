# `branchsight serve`

```text
branchsight serve [--port N] [--root DIR]... [dir]
```

Serves the BranchSight page for every git repository under the roots, on loopback only.

| Argument | Default | Meaning |
|---|---|---|
| `--port N`, `-p N` | `7420` | port on `127.0.0.1` and `[::1]` |
| `--root DIR`, `-r DIR` | parent of `dir`'s repository, or `dir` | where to look for repositories; repeatable |
| `dir` | `.` | when a repository, the one a page opens without `?repo=` |
| `--help`, `-h` | | usage |

If another `branchsight` already listens on the port, the command prints the URL of `dir`'s repository and exits.

## Which repository a page opens

First match wins:

1. `?repo=/absolute/path` in the URL;
2. the host name: `http://<project>.localhost:7420` opens the project of that name under the roots;
3. `dir`, when it is a repository;
4. the last project opened in this browser;
5. otherwise the project picker.

## Requests it accepts

Only with a loopback `Host` header (`localhost`, `127.0.0.1`, `[::1]`, `*.localhost`); anything else gets `403`.
Every `/api/` request except `/api/ping` also needs the header `X-BranchSight: 1` and no `Sec-Fetch-Site` other
than `same-origin` or `none`, so another site cannot make git run from your browser, even to read. Every `/api/`
request takes `repo=<path>`, defaulting to `dir`. Errors come back as `400` with git's message as plain text.

| Path | Method | Returns |
|---|---|---|
| `/api/context` | GET | version, roots, default repository |
| `/api/projects` | GET | repositories under the roots, most recently used first |
| `/api/repo` | GET | path, name, HEAD, current branch, remotes |
| `/api/refs` | GET | branches (with the `upstream` they track, its `upstreamRemote`, and how many commits they are `ahead` of it and `behind` it), remote branches, tags |
| `/api/commits?ref=…&limit=N` | GET | `commits` of the given refs, `HEAD` without any, and the `history` fingerprint they were read under; `@local`, `@remotes`, `@tags` select them all |
| `/api/commit?h=<hash>` | GET | `header`, plus the changes against the first parent |
| `/api/compare?from=<hash>&to=<hash>` | GET | the changes of `git diff from to` |
| `/api/filediff?kind=…&path=…` | GET | `patch`, one file's diff with the whole file as context, and `truncated`; `kind` is `commit` (with `h`), `compare` (with `from`, `to`), `staged`, `unstaged` or `untracked`; a renamed file takes both paths |
| `/api/worktree` | GET | `staged` and `unstaged` changes, `untracked` paths |
| `/api/state` | GET | `history`, a fingerprint of the refs, HEAD and shallow boundary; `worktree`, one of `git status` and the changed files; `status`, the staged, unstaged, untracked and conflicted counts; `operation`, the `merge`, `rebase`, `cherry-pick` or `revert` in progress, or null |
| `/api/fetch` | POST | `git fetch --all --prune` |
| `/api/checkout?ref=…` | POST | switch to a branch, a remote branch, a tag or a commit |
| `/api/merge?ref=…&mode=…&commit=0` | POST | merge a branch, remote branch, tag or commit into the current branch; `mode` is `fast-forward`, `merge-commit` or `squash`; `commit=0` stops before committing; conflicts answer `400` and leave the merge to resolve |
| `/api/pull?branch=<local>&from=<remote>/<branch>&mode=…` | POST | bring a local branch up to a remote one; `mode` is `merge`, `rebase` or `fast-forward`; a branch that is only behind is updated without switching to it |
| `/api/reset-to-remote?branch=<local>&from=<remote>/<branch>` | POST | make a local branch match a remote one; keeps uncommitted changes, or refuses |
| `/api/cherry-pick?h=<hash>&commit=0` | POST | copy a commit onto the current branch; `commit=0` only stages it; refused for a commit it already has |
| `/api/revert?h=<hash>&commit=0` | POST | add a commit undoing it; `commit=0` only stages the revert; refused for a commit outside its history |
| `/api/drop?h=<hash>` | POST | remove a commit of the current branch, rebasing the ones after it |
| `/api/rebase?ref=…` | POST | rebase the current branch on a branch, tag or commit |
| `/api/reset?h=<hash>&mode=…` | POST | reset the current branch; `mode` is `soft`, `mixed` or `hard` |
| `/api/continue` | POST | stage the resolved files and finish the operation in progress; refused while a file has conflict markers |
| `/api/abort` | POST | abort the operation in progress |
| `/api/push?ref=<branch>&remote=…&mode=…` | POST | push a local branch to the branch of the same name; `mode` is `normal`, `force-with-lease` or `force`; the branch then tracks what it was pushed to |
| `/api/delete-remote?ref=<remote>/<branch>` | POST | `git push <remote> --delete <branch>` |

*Changes* are `files`, each with `status` (`A`, `M`, `D`, `R`, `T`, `U` for a conflict…), `path`, `from` for a
rename, `add` and `del` (null for a binary file or a conflict) and its own `patch`, plus `truncated`. Patches are cut
at 4 MB in total; the file list and the counts are always complete. Time limits and prompts are described in
[what BranchSight changes](../explanation/safety.md#time-limits-and-prompts).
