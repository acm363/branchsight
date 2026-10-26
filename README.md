<p align="center"><img src="assets/icon.svg" width="96" alt="BranchSight icon"></p>

# BranchSight

A dense git commit graph for Linux, Windows and macOS — one line per commit, about 50 on a screen — filtered to
the branches you pick, with the files and diff of each commit. In the spirit of VS Code's Git Graph, without the
editor.

![The BranchSight graph](docs/images/graph.png)

- **Read history fast**: every local and remote branch by default, a fuzzy branch picker, labels in each lane's
  colour, a limit from 10 commits to all of them.
- **Look inside**: the files and diff of a commit, the comparison of any two commits or branches (`ctrl`+click),
  your uncommitted work split into staged, not staged and untracked.
- **Read a diff properly**: a full-window diff view, *FROM* one side *TO* the other with branch names, whole files
  side by side, next and previous change across every file.
- **Act from the graph**: right-click to check out a branch, a tag or a commit, copy a name or a hash, or delete a
  branch on the remote.
- **Always current**: commits, checkouts and file edits made elsewhere show up within five seconds.
- **Your git, your machine**: it runs your own `git`, opens no network port, and changes a repository only when you
  ask.

![The diff view](docs/images/diff-view.png)

## Install

Download the AppImage, `.rpm`, `.deb`, `.msi` or `.dmg` from
[Releases](https://github.com/acm363/branchsight/releases), or [build it from source](docs/how-to/install.md).
BranchSight needs `git` on the `PATH`.

## Documentation

- New here? Take the [first look at a repository](docs/tutorials/first-look.md).
- Getting something done: [compare branches](docs/how-to/compare.md),
  [review uncommitted changes](docs/how-to/review-uncommitted-changes.md),
  [check out and clean up branches](docs/how-to/checkout-and-clean-up.md),
  [use it with WSL, SSH or a container](docs/how-to/use-remotely.md),
  [customise the view](docs/how-to/customise-the-view.md).
- Looking something up: [interface](docs/reference/interface.md), [`branchsight serve`](docs/reference/serve.md),
  [settings and defaults](docs/reference/settings.md).
- Understanding it: [architecture](docs/explanation/architecture.md),
  [what it changes and how it stays safe](docs/explanation/safety.md).

## Development

```bash
scripts/check.sh      # unit and end-to-end tests, graph layout tests, desktop app build
```

| Path | Role |
|---|---|
| `src/git.rs` | the git commands |
| `src/server.rs` | the routes shared by the app and `serve`, and the loopback-only guard |
| `src/projects.rs` | repository discovery |
| `src/web/` | the page; `graph.js` computes the lanes |
| `app/` | the Tauri shell |
| `tests/` | end-to-end tests against throwaway repositories, layout tests (`node --test`) |
| `scripts/` | checks, the demo repository and the documentation screenshots |

CI runs the tests on Linux and Windows on every push; pushing a `v*` tag tests on macOS too and drafts a release
with the installers.

## Contributing

Bug reports, ideas and pull requests are welcome: see [CONTRIBUTING.md](CONTRIBUTING.md). Every pull request is
reviewed by the maintainer, [@acm363](https://github.com/acm363), before it lands in `main`.

<a href="https://github.com/acm363/branchsight/graphs/contributors"><img src="https://contrib.rocks/image?repo=acm363/branchsight" alt="Contributors"></a>

## License

[MIT](LICENSE) © 2026 Acm_363 — free to download, use, modify and fork; keep the copyright notice.
