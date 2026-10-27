# Cherry-pick, revert, drop, rebase or reset

Everything here starts with a right-click on a commit row, and applies to the branch you have checked out. Each
action asks first; the ones that rewrite history say so before you confirm.

| Choose | What happens | git |
|---|---|---|
| **Cherry pick…** | copies that commit onto your branch as a new commit | `git cherry-pick` |
| **Revert…** | adds a commit that undoes it, keeping history as it is | `git revert` |
| **Drop…** | removes it from your branch; the commits after it are rewritten | `git rebase --onto <commit>^ <commit>` |
| **Rebase `<branch>` on this commit…** | replays your branch's own commits on top of it; they get new hashes | `git rebase` |
| **Reset `<branch>` to this commit…** | moves your branch back or forward to it | `git reset` |

Cherry pick and Revert offer **Don't commit**, which stages the result for you to review in
**Uncommitted changes** and commit yourself. Rebase is also offered on a branch label, and Drop works on any commit
of your branch except a merge or the first one.

## Pick how far a reset goes

- **Soft** keeps every change since that commit, staged, ready to commit again;
- **Mixed** keeps them as well, not staged: the default;
- **Hard** discards them, uncommitted work included, after one more confirmation.

A reset only moves your branch: the commits it leaves behind stay reachable through `git reflog` for a while, but
uncommitted work discarded by a hard reset is gone.

## When git stops on a conflict

Cherry pick, revert, drop and rebase stop on the first conflict. The conflicted files appear in
**Uncommitted changes** marked `U`. Resolve them in your editor, then run the command the message gives, such as
`git cherry-pick --continue`, or `--abort` to go back to where you were.

Drop and rebase rewrite commits: if the branch was already pushed, the remote needs a push with
**Force with lease** afterwards, see [push a branch](checkout-and-clean-up.md#push-a-branch).
