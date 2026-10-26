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
Every `/api/` request takes `repo=<path>`, defaulting to `dir`.

| Path | Method | Returns |
|---|---|---|
| `/api/context` | GET | version, roots, default repository |
| `/api/projects` | GET | repositories under the roots, most recently used first |
| `/api/repo` | GET | path, name, HEAD, current branch |
| `/api/refs` | GET | branches, remote branches, tags |
| `/api/log?ref=…&limit=N` | GET | commits of the given refs; `@local`, `@remotes`, `@tags` select them all |
| `/api/commit?h=<hash>` | GET | the commit and its diff against its first parent |
| `/api/compare?from=<hash>&to=<hash>` | GET | `git diff from to` |
| `/api/filediff?kind=…&path=…` | GET | one file's diff with the whole file as context; `kind` is `commit` (with `h`), `compare` (with `from`, `to`), `staged`, `unstaged` or `untracked`; a renamed file takes both paths |
| `/api/status` | GET | counts of staged, unstaged and untracked files |
| `/api/worktree` | GET | staged diff, unstaged diff, untracked paths |
| `/api/state` | GET | a fingerprint that changes with refs, HEAD and the working tree |
| `/api/fetch` | POST | `git fetch --all --prune` |
| `/api/checkout?ref=…` | POST | switch to a branch, a remote branch, a tag or a commit |
| `/api/delete-remote?ref=<remote>/<branch>` | POST | `git push <remote> --delete <branch>` |

Diffs are cut at 4 MB. Network commands run without prompts and stop after 90 s.
