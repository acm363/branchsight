# Check out and clean up branches

Everything here starts with a right-click on a branch label, a tag or a commit row.

![The branch menu](../images/branch-menu.png)

## Switch to a branch

Choose **Checkout `<branch>`**.

- On a local branch, BranchSight switches to it.
- On a remote branch such as `origin/feature/sync`, it creates the local `feature/sync` tracking it and switches to
  it; if that local branch already exists, it switches to it as it is.

Git refuses when your uncommitted changes would be overwritten; the reason appears at the bottom of the window.

## Look at a tag or an old commit

Choose **Checkout tag `<tag>` (detached)** or **Checkout commit `<hash>` (detached)**. HEAD then points at that
commit without being on a branch, which is how git lets you inspect old states. Check out a branch to leave it.

After any checkout the graph keeps your branch selection and scrolls to the new HEAD.

## Push a branch

Choose **Push `<branch>`…**, or **Push `<branch>` to a remote…** when it has no upstream yet. Pick the remote and how
to push:

- **Normal** only adds commits, and is refused when the remote has commits you do not have;
- **Force with lease** overwrites the remote branch unless someone pushed to it since your last fetch;
- **Force** overwrites it whatever it holds, after one more confirmation.

A branch pushed for the first time tracks the remote branch from then on, as with `git push -u`.

## Delete a branch on the remote

Choose **Delete remote branch `<remote>/<branch>`**, then confirm. This runs
`git push <remote> --delete <branch>` with your own credentials: the branch disappears for everyone using that
remote, and BranchSight cannot undo it. Your local branch, if any, is kept.

## Copy a name or a hash

**Copy branch name** copies the name without the remote, ready for `git switch`. **Copy commit hash** copies the full
hash; clicking a hash in the Commit column, or pressing `c`, does the same.
