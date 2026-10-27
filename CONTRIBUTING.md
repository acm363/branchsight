# Contributing to BranchSight

Thank you for helping. Bug reports, ideas, documentation and code are all welcome, and every contributor is
credited.

## Report a bug or suggest a feature

Open an [issue](https://github.com/acm363/branchsight/issues/new/choose) and pick the form that fits. For a bug,
the version (☰ › Help) and the steps to reproduce it matter most. Leave out anything private, such as the names of
your repositories or branches.

## Propose a change

1. Fork the repository and create a branch from `main`.
2. Make the change, with its tests and documentation.
3. Run `scripts/check.sh`; it must pass.
4. Open a pull request. The maintainer reviews every pull request before it is merged into `main`; small, focused
   pull requests are reviewed fastest.

For anything larger than a fix, open an issue first so we can agree on the approach before you spend time on it.

## Set up

You need Rust (stable), Node 20 or later and git. To build the desktop app on Linux, also install the webview's
development files (see [Install](docs/how-to/install.md#from-source)).

```bash
scripts/check.sh                                     # every test, then a build of the app
cargo run -- serve                                   # the page on http://localhost:7420, reloaded by hand
cd app && npx @tauri-apps/cli@2 dev                  # the desktop app
```

The [architecture](docs/explanation/architecture.md) explains how the pieces fit together.

## Conventions

- **Tests first for behaviour.** Server behaviour is tested end to end in `tests/` (`history.rs`, `diffs.rs`,
  `actions.rs`, `serve.rs`) against throwaway repositories; page logic that needs no screen lives in plain modules
  tested in `tests/js` with `node --test`. Tests are written as `// Given.` / `// When.` / `// Then.`.
- **Safety.** Anything that changes a repository needs a `POST` route, validated inputs and an end-to-end test that
  tries to abuse it; see [what BranchSight changes](docs/explanation/safety.md).
- **Comments.** Only where a reader would otherwise get the code wrong; the reasoning goes in the commit message.
- **Commits.** [Conventional Commits](https://www.conventionalcommits.org/) subjects (`feat:`, `fix:`, `docs:`…),
  a short body saying why when it is not obvious.
- **Documentation.** `docs/` follows [Diátaxis](https://diataxis.fr/); update the page that matches the change, and
  the screenshots when the interface changes.

## Sign your work

Sign off every commit with `git commit -s`. It adds a `Signed-off-by:` line certifying that you wrote the change, or
otherwise have the right to submit it, under the project's licence: the
[Developer Certificate of Origin](https://developercertificate.org/).

## Licence

BranchSight is released under the [MIT licence](LICENSE). By contributing, you agree that your contribution is
released under the same licence. You keep the copyright on what you write, and your name stays in the history and
in the list of contributors.
