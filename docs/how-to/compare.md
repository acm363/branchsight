# Compare two branches or commits

Use this to see everything that would land if one branch were merged into another, or what changed between two
releases.

1. Click the first commit, for example the tip of `main`.
2. Hold `ctrl` (`cmd` on macOS) and click the second one, for example the tip of `feature/tags`.

   Or right-click the second commit and choose **Compare with selected**.

The panel shows the diff from the older of the two to the newer, whichever you clicked first. Both rows are
highlighted while the comparison is open.

![Comparing two commits](../images/compare.png)

## Read the comparison file by file

Click **Open diff view** at the top of the panel, or `⤢` on a file. The diff view fills the window and labels both
sides: **FROM** the older commit, **TO** the newer one, each with its branch name when a branch points there.

![The diff view](../images/diff-view.png)

- **↓ Next change** (`n` or `F7`) and **↑ Previous change** (`p` or `shift+F7`) jump from one block of changes to the
  next; past the last change of a file they continue in the next file.
- **◀ ▶** (`[` `]`) and the file selector move between files.
- **Side by side** shows the whole old file on the left and the whole new file on the right; **Unified** shows one
  column. Inside a modified line, the part that changed is highlighted.
- `esc` closes the view.

To compare against a tag, click the commit carrying the tag. To end the comparison, click any commit, press `esc`,
or double-click a row.

The comparison is the result of `git diff <older> <newer>`: the end states of the two commits, whatever happened in
between. To see commit by commit what a branch adds, select only that branch in the branch picker and walk
through it with `↓`.
