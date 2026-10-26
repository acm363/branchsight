<p align="center"><img src="assets/icon.svg" width="96" alt="BranchSight icon"></p>

# BranchSight

A dense git commit graph — one line per commit, about 50 on a screen — filtered to the branches you pick, with
the files and diff of each commit. A desktop app for Linux, Windows and macOS, in the spirit of VS Code's Git
Graph, without the editor.

BranchSight reads your repositories and changes them only when you ask: fetch, checkout, or deleting a remote
branch (after a confirmation). It needs `git` on the `PATH`, and uses your own credentials for the network.

## Install

Download from [Releases](https://github.com/acm363/branchsight/releases):

| System | File |
|---|---|
| Any Linux distribution | `.AppImage` (make it executable, run it) |
| Fedora, RHEL, openSUSE | `.rpm` |
| Debian, Ubuntu | `.deb` |
| Windows | `.msi` or `-setup.exe` |
| macOS | `.dmg` |

Builds are not code-signed yet: macOS asks you to confirm the first launch (right-click → Open), Windows
SmartScreen shows "More info → Run anyway".

## Use

The app opens on the project picker: type to filter the repositories found under your home folder (three levels
deep), or type a path. `branchsight-app <dir>` opens a repository directly.

| | |
|---|---|
| **Menu** (top left) | open project, close project, text size |
| **Branches** | starts on all local + all remote branches; type to filter, `enter` or click to toggle |
| **Columns** | drag a column edge to resize it, double-click it to reset |
| **Commit** | click or `enter`: files and diff (merges against their first parent); fold each file, or all of them; double-click closes it |
| **Right-click** | checkout a branch (a remote one gets a local tracking branch), a tag or a commit (detached); copy a name or hash; delete a remote branch. The view keeps its filter and scrolls to the new HEAD |
| **Limit** | show the last 10 to 20,000 commits, or all |
| **Hash** | click it, or press `c`, to copy the full hash |
| **Fetch** | `git fetch --all --prune`; tick **auto** to repeat it every 2 minutes |

Local commits, checkouts and branch changes show up on their own within 5 seconds.

## Remote, WSL and containers: `branchsight serve`

When the repository lives where the app cannot reach it — inside WSL, on a server over SSH, in a dev container —
run the same page from there and open it in a browser:

```bash
branchsight serve                      # http://localhost:7420, every repo under the parent of the current one
branchsight serve --root ~/code --port 7500
ssh -L 7420:localhost:7420 my-server   # then open http://localhost:7420 locally
```

The server listens on loopback only and answers loopback host names only (`localhost`, `127.0.0.1`, `[::1]`,
`*.localhost`), so a web page cannot reach it through DNS rebinding. `http://<project>.localhost:7420` opens the
project of that name. On Linux, `contrib/branchsight.service` keeps it running as a systemd user service.

## Development

```bash
scripts/check.sh      # unit + end-to-end tests, graph layout tests, desktop app build
```

| Path | Role |
|---|---|
| `src/git.rs` | the git commands; only `fetch`, `checkout` and the remote branch deletion write |
| `src/server.rs` | the routes, shared by the app and `serve`, and the loopback-only guard |
| `src/projects.rs` | repository discovery under the roots |
| `src/web/` | the page; `graph.js` computes the lanes |
| `app/` | the Tauri shell: the page talks to the Rust core through Tauri's IPC, with no network port |
| `tests/` | end-to-end tests against throwaway repositories, layout tests (`node --test`) |

The desktop app needs the system webview's development files to build on Linux:

```bash
sudo dnf install webkit2gtk4.1-devel openssl-devel librsvg2-devel libxdo-devel   # Fedora
sudo apt install libwebkit2gtk-4.1-dev libssl-dev librsvg2-dev libxdo-dev        # Debian / Ubuntu
cd app && npx @tauri-apps/cli@2 build                                            # bundles in target/release/bundle
```

CI runs the tests on Linux, macOS and Windows and compile-checks the app on every push. Installers are built on
demand (Actions → ci → Run workflow → bundles) and by pushing a `v*` tag, which drafts a release with them.

## License

[MIT](LICENSE) © 2026 Acm_363 — free to download, use, modify and fork; keep the copyright notice.
