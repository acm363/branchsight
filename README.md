<p align="center"><img src="assets/icon.svg" width="96" alt="BranchSight icon"></p>

# BranchSight

A dense git commit graph — one line per commit, about 50 on a screen — filtered to the branches you pick, with
the files and diff of each commit. In the spirit of VS Code's Git Graph, without the editor.

One Rust core, three ways to use it:

| | What it is | Good for |
|---|---|---|
| **Desktop app** | a native window (Tauri) | daily use on Linux, Windows, macOS |
| **Browser** | `branchsight` serves the same page on `http://localhost:7420` | a browser tab, an IDE's browser pane, a background service |
| **Terminal** | `bsight`, an fzf + tig wrapper | SSH sessions, no GUI at all |

Requires `git` on the `PATH`. Every view reads your repositories locally; nothing leaves the machine except
`git fetch` when you ask for it.

## Install

**Desktop app** — download from [Releases](https://github.com/acm363/branchsight/releases):
`.AppImage` (any Linux distribution), `.rpm`, `.deb`, `.msi` / `.exe` (Windows), `.dmg` (macOS).
Builds are not code-signed yet: macOS asks you to confirm the first launch (right-click → Open), Windows
SmartScreen shows "More info → Run anyway".

**Browser version / CLI** — a release binary (`branchsight-cli-*`), or from source:

```bash
cargo build --release
ln -s "$PWD/target/release/branchsight" ~/.local/bin/branchsight
```

## Usage

### Desktop app

Opens the last project, or the project picker. Projects are found by scanning your home directory three levels
deep (hidden folders, `node_modules`, `target`, `dist`, `vendor` skipped); type a path in the picker to open any
other folder. `branchsight-app <dir>` opens that repository directly.

### Browser

```bash
branchsight                          # serves every repo under the parent of the current one, on :7420
branchsight --root ~/code --port 7500
```

Run in a repository while a branchsight already listens on the port, it prints that repository's URL and exits.
To keep it running, as a systemd user service on Linux:

```bash
systemctl --user enable --now "$PWD/branchsight.service"
systemctl --user restart branchsight        # after a rebuild
```

Which repository a page shows, first match wins:

1. `?repo=/abs/path` in the URL;
2. the host name: `http://<project>.localhost:7420` opens the project named `<project>`;
3. the directory branchsight was started in, when it is a repository;
4. the last project opened in this browser;
5. otherwise the project picker.

The server answers loopback host names only (`localhost`, `127.0.0.1`, `[::1]`, `*.localhost`), so a web page
cannot reach it through DNS rebinding.

### Keys

| Key | Action |
|---|---|
| `p` | project picker |
| `b` or `/` | branch picker: type to filter, `enter` / click toggles, `esc` closes |
| `↑` `↓` `j` `k` | move |
| `enter` / click | files and diff of the commit (merges: against the first parent) |
| click the hash, or `c` | copy the full hash |
| `f` | `git fetch --all --prune`; tick **auto** to repeat it every 2 minutes |
| `r` | reload (local changes are picked up within 5 s anyway) |
| `esc` | close the diff |

The branch picker starts on **all local branches** + **all remote branches**; the selection lives in the URL
(`?ref=…`) and is remembered per repository.

### Terminal: `bsight`

```bash
sudo dnf install fzf tig          # or apt / brew
ln -s "$PWD/bsight" ~/.local/bin/bsight
bsight                               # pick branches, then graph
bsight main origin/main              # graph straight away
```

Picker: `tab` select, `alt-a` / `alt-d` all / none, `alt-t` add tags, `enter` show. Graph: `enter` diff,
`b` back to the picker, `q` quit.

## Development

```bash
scripts/check.sh      # unit + end-to-end tests, graph layout tests, desktop app build
```

| Path | Role |
|---|---|
| `src/git.rs` | the git commands, all read-only except `fetch` |
| `src/server.rs` | HTTP routes and the loopback-only guard |
| `src/projects.rs` | repository discovery under the roots |
| `src/web/` | the page; `graph.js` computes the lanes |
| `app/` | the Tauri shell: starts the server on a free loopback port and opens a window on it |
| `tests/` | end-to-end tests against throwaway repositories, layout tests (`node --test`) |

The desktop app needs the system webview's development files to build on Linux:

```bash
sudo dnf install webkit2gtk4.1-devel openssl-devel librsvg2-devel libxdo-devel   # Fedora
sudo apt install libwebkit2gtk-4.1-dev libssl-dev librsvg2-dev libxdo-dev        # Debian / Ubuntu
cd app && npx @tauri-apps/cli@2 build                                            # bundles in target/release/bundle
```

CI runs the tests on Linux, macOS and Windows and compile-checks the app on every push (about a minute when
cached). Installers are built on demand (Actions → ci → Run workflow → bundles) and by pushing a `v*` tag,
which drafts a release with them attached.

## License

[MIT](LICENSE) © 2026 Acm_363 — free to download, use, modify and fork; keep the copyright notice.
