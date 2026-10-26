# Settings and defaults

## Defaults

| Setting | Default |
|---|---|
| branches shown | all local + all remote branches |
| commit limit | 5,000 |
| theme | System |
| text size | 13 px (10 to 20) |
| remotes / tags in the branch picker | shown / hidden |
| auto-fetch | off; every 2 minutes when on |
| change detection | every 5 seconds while the window is visible |
| columns | graph sized for up to 10 lanes, author 130 px, date 108 px, hash 56 px |
| project discovery | the home folder, 3 levels deep, skipping hidden folders, `node_modules`, `target`, `dist`, `vendor` |

## Where your choices are kept

In the webview's local storage, never in the repositories:

| App | Location |
|---|---|
| desktop, Linux | `~/.local/share/com.acm363.branchsight/localstorage/` |
| desktop, macOS | `~/Library/WebKit/com.acm363.branchsight/` |
| desktop, Windows | `%LOCALAPPDATA%\com.acm363.branchsight\EBWebView\` |
| `branchsight serve` | your browser's storage for that address; `localhost:7420` and `127.0.0.1:7420` are separate |

| Scope | Keys |
|---|---|
| every project | `branchsight:theme`, `branchsight:fontSize`, `branchsight:cols`, `branchsight:recent`, `branchsight:last` |
| one project | `branchsight:<repository path>:` `sel` (branches), `limit`, `remote`, `tag`, `autoFetch`, `detailWidth` |

Deleting the folder or the keys returns BranchSight to its defaults.
