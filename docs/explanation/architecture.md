# Architecture

BranchSight is one Rust core with two ways in, and one web page on top.

```text
            desktop app (Tauri)                    branchsight serve
   ┌──────────────────────────────────┐   ┌───────────────────────────────┐
   │ webview ── IPC: invoke("api") ──┐ │   │ browser ── HTTP on loopback ─┐ │
   └─────────────────────────────────┼─┘   └──────────────────────────────┼─┘
                                     ▼                                    ▼
                               api::respond(method, "/api/…")    ← one routing table
                                     │
                          src/git/ ── runs the git binary
```

## One page, two transports

The page in `src/web/` does not know where it runs. In the desktop app it calls the Rust core through Tauri's IPC
channel; in a browser it calls the same routes over HTTP. Both end in `api::respond`, so the end-to-end tests,
which go through HTTP, also cover what the app does. The page is plain ES modules grouped by what they show:
`history/` the graph and its loading, `changes/` the files panel and the diff view, `header/` the pickers and the
menu, `actions/` the git commands and the right-click menu, `shared/` the small helpers every part uses. Pure
modules such as `history/graph.js` and `changes/diffview.js` are covered by the node tests.

The desktop app opens **no network port**: on a machine shared with other users, a port on `127.0.0.1` would let any
of them read your repositories. `serve` does open one, on loopback only, for the cases where the app cannot reach the
repository (see [use it remotely](../how-to/use-remotely.md)).

## The real git, not a re-implementation

Every operation runs your installed `git`, the way you would type it. Hooks, credentials, signing, `.gitconfig` and
odd repository layouts behave exactly as in your terminal, and BranchSight never needs to catch up with git's
on-disk formats. The cost is a process per request, a few milliseconds each; listing 5,000 commits takes about
40 ms.

## Drawing the graph

`git log --date-order` gives the commits and their parents. `src/web/history/graph.js` assigns each commit a lane in one
pass: a commit takes the lane that was waiting for it, its first parent continues on the same lane, and each other
parent opens or joins another one. Lanes are reused as soon as they are free, which keeps the graph narrow; on a
repository with 230 branches and 4,000 commits the widest point is 13 lanes. Only the rows on screen are drawn, on
a canvas, so scrolling stays smooth at any length.

## Staying current without a refresh button

Every five seconds, while the window is visible, the page asks for two fingerprints: one of the refs and HEAD, one
of `git status` and the size and modification time of each changed file. When the first changes, the page reloads
the graph; when only the second does, it refreshes the uncommitted-changes row and its panel. Both keep your scroll
position and the files you folded.
Polling costs about 30 ms of git work per check and works the same on every system, including network drives and
WSL, where file watchers are unreliable.
