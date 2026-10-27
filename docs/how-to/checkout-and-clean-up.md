# Check out and clean up branches

Everything here starts with a right-click on a branch label, a tag or a commit row.

![The branch menu](../images/branch-menu.png)

## Switch to a branch

Choose **Checkout `<branch>`**.

- On a local branch, BranchSight switches to it.
- On a remote branch such as `origin/feature/sync`, it creates the local `feature/sync` tracking it and switches to
  it. If that local branch already exists, it switches to it and brings it up to the remote one when it is only
  behind; when both have commits of their own, it changes nothing and says so, so that you can merge instead.

Git refuses when your uncommitted changes would be overwritten; the reason appears at the bottom of the window.

## Look at a tag or an old commit

Choose **Checkout tag `<tag>` (detached)** or **Checkout commit `<hash>` (detached)**. HEAD then points at that
commit without being on a branch, which is how git lets you inspect old states. Check out a branch to leave it.

After any checkout the graph keeps your branch selection and scrolls to the new HEAD.

## Merge a branch into yours

Choose **Merge `<branch>` into `<current>`…** on a branch, a remote branch, a tag or a commit, then pick how:

- **Fast-forward when possible** moves your branch forward if it has no commits of its own, and otherwise creates a
  merge commit, as `git merge` does;
- **Always create a merge commit** keeps a merge commit even when a fast-forward was possible (`--no-ff`);
- **Squash** stages all of the branch's changes as one, for you to commit (`--squash`).

Tick **Don't commit** to stop before the merge commit and review the result in **Uncommitted changes** first. When
the branches conflict, the merge stops: the conflicted files appear in **Uncommitted changes** marked `U`; resolve
them in your editor and commit, or run `git merge --abort` to go back.

## Push a branch

Choose **Push `<branch>`…**, or **Push `<branch>` to a remote…** when it has no upstream yet. Pick the remote and how
to push:

- **Normal** only adds commits, and is refused when the remote has commits you do not have;
- **Force with lease** overwrites the remote branch unless someone pushed to it since your last fetch;
- **Force** overwrites it whatever it holds, after one more confirmation.

The branch is pushed to the remote branch of the same name, and tracks it from then on, as with `git push -u`. When it
tracked something else, such as the `origin/main` it was started from, the dialog says so before you push.

## Delete a branch on the remote

Choose **Delete remote branch `<remote>/<branch>`**, then confirm. This runs
`git push <remote> --delete <branch>` with your own credentials: the branch disappears for everyone using that
remote, and BranchSight cannot undo it. Your local branch, if any, is kept.

## Copy a name or a hash

**Copy branch name** copies the name without the remote, ready for `git switch`. **Copy commit hash** copies the full
hash; clicking a hash in the Commit column, or pressing `c`, does the same.
