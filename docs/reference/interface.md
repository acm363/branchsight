# Interface

## Header

| Element | Role |
|---|---|
| ☰ | menu: File, View, Help |
| project chip | the open project; click it or press `p` to switch |
| branch picker | which branches the graph shows; type to filter, `enter` or click to toggle |
| commit count | commits shown; `(limit)` when the limit cut the history |
| limit | 10, 20, 100, 500, 1k, 5k (default), 20k, all |
| terminal (desktop app) | open or hide the [terminal panel](#terminal-desktop-app); next to the limit |
| ⇣ fetch | `git fetch --all --prune` |
| auto | fetch every 2 minutes while the window is visible |
| ⟳ | reload now |

## Menu

| Menu | Item |
|---|---|
| File | Open project… · Close project (next launch starts on the picker) |
| View | Text size `−` `+` `reset` · Theme `System` `Light` `Dark` · Diff panel `Right` `Bottom` · Terminal `Bottom` `Right` `Left` (desktop app) · Language |
| Help | keyboard and mouse shortcuts · version |

## Keyboard

| Key | Action |
|---|---|
| `p` | open a project |
| `b` or `/` | open the branch picker |
| `↑` `↓` or `k` `j` | previous, next commit |
| `page up` `page down` `home` `end` | move faster |
| `enter` | files and diff of the selected commit |
| `c` | copy the selected commit's hash |
| `f` | fetch |
| `r` | reload |
| `ctrl`+`j` (`cmd`+`j`) | open a terminal, or another one (desktop app) |
| `ctrl`+`shift`+`j` | open the system terminal on the repository (desktop app) |
| `esc` | close the diff view, the open menu, the picker or the diff panel |

## Mouse

| Gesture | Action |
|---|---|
| click a commit | files and diff |
| double-click a commit | close the diff panel |
| `ctrl`+click (`cmd`+click) a commit | compare it with the selected one |
| click a hash | copy it |
| right-click | the menu below |
| drag a column edge, the panel edge | resize; double-click a column edge resets it |

## Right-click menu

| On | Items |
|---|---|
| any commit | Compare with selected (when another commit is selected) · Checkout commit (detached) · Cherry pick, when the current branch does not have it · Revert, Drop, when it does · Merge into the current branch · Rebase the current branch on it, both when the branch does not have it yet · Reset the current branch to it · Copy commit hash |
| local branch | Checkout · where it stands against its remote branch, with Sync (pull it in), Push or, once diverged, Reset to the remote branch · Merge into the current branch · Rebase the current branch on it · Push (normal, force with lease or force, to the remote of your choice), unless the remote branch it tracks is already on the same commit · Copy branch name · Delete remote branch, when its remote is on the same commit |
| remote branch | Checkout (switches to its local branch, or creates it) · the same sync choices as its local branch, when there is one · Merge into the current branch · Rebase the current branch on it · Copy branch name · Delete remote branch |
| tag | Checkout tag (detached) · Merge into the current branch · Copy tag name |

## Operation bar

Shown above the graph while a merge, rebase, cherry pick or revert is stopped: what is in progress, how many files
are still in conflict, **Continue** and **Abort**.

## Graph

| Mark | Meaning |
|---|---|
| filled label | local branch, `name \| remote` when its remote is on the same commit |
| outlined label | remote branch alone |
| dashed label | tag |
| ringed dot | HEAD |
| hollow dot | merge commit |
| dashed row and lane | uncommitted changes |
| dotted line | the parent is beyond the commit limit |

## Diff panel

| Element | Role |
|---|---|
| title | the commit, the comparison (`older → newer`) or *Uncommitted changes*; click a hash to copy it |
| Collapse all / Expand all | fold or unfold every file |
| file list | click a file to unfold it and jump to it |
| file header | stays pinned while scrolling; click it or `▾` to fold the file; `⤢` opens it in the diff view |
| Open diff view | the diff view on the first file; double-clicking a file in the list opens it on that file |

## Diff view

| Element | Role |
|---|---|
| FROM / TO | the two sides: a commit's parent and the commit, the older and the newer commit of a comparison, or HEAD, the index and the working tree; with the branch name when one points there |
| ◀ ▶ and the file selector, `[` `]` | previous, next file |
| ↑ Previous change, ↓ Next change, `p` `n`, `shift+F7` `F7` | previous, next block of changes, continuing into the neighbouring file |
| Side by side / Unified | two columns with the whole files, or one column; remembered |
| divider between the sides | drag it to give one side more room, double-click it to reset; remembered |
| highlight inside a line | the part of a modified line that changed |
| N unmodified lines | lines more than 3 away from a change, folded; click to unfold them |
| `esc` or ✕ Close | back to the graph |

## Terminal (desktop app)

Only in the desktop app. `branchsight-cli serve` ships no terminal: a shell over HTTP would be remote code
execution on the host, so the commands and the terminal assets are compiled into the app alone.

| Element | Role |
|---|---|
| panel | docked bottom, right or left (View menu); drag its edge to resize; the size and side are remembered |
| tab | one shell; click to focus, ✕ or middle-click to close; closing the last one hides the panel |
| **+** | a new terminal with the selected shell (`ctrl`+`j` does the same) |
| shell selector | the shells found on the `PATH` (bash, zsh, pwsh, git-bash…); choosing one opens a terminal with it; the default is your login shell |

`ctrl`+`shift`+`j` opens your system terminal on the repository instead of the panel.
