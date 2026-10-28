# Install BranchSight

BranchSight needs `git` on your `PATH`.

## From a release

Download the file for your system from [Releases](https://github.com/acm363/branchsight/releases):

| System | File | Install |
|---|---|---|
| Any Linux | `BranchSight_<version>_amd64.AppImage` | `chmod +x` it and run it |
| Fedora, RHEL, openSUSE | `BranchSight-<version>-1.x86_64.rpm` | `sudo dnf install ./BranchSight-*.rpm` |
| Debian, Ubuntu | `BranchSight_<version>_amd64.deb` | `sudo apt install ./BranchSight_*.deb` |
| Windows | `BranchSight_<version>_x64_en-US.msi` or `…-setup.exe` | run it |
| macOS | `BranchSight_<version>_aarch64.dmg` | open it, drag BranchSight to Applications |

Releases are not code-signed yet. On macOS, right-click the app and choose **Open** the first time; on Windows,
SmartScreen asks you to choose **More info → Run anyway**.

## The `branchsight serve` command

The installers above contain the desktop app only (`branchsight-app`). To serve the page to a browser, over SSH, in
WSL or in a container, download the command for your system from the same release, name it `branchsight` and put it
on your `PATH`:

```bash
mkdir -p ~/.local/bin
curl -L -o ~/.local/bin/branchsight https://github.com/acm363/branchsight/releases/latest/download/branchsight-cli-linux-x86_64
chmod +x ~/.local/bin/branchsight
branchsight serve --root ~/code
```

On macOS use `branchsight-cli-macos-arm64`, then `xattr -d com.apple.quarantine ~/.local/bin/branchsight` once, since
it is not signed. On Windows rename `branchsight-cli-windows-x86_64.exe` to `branchsight.exe` and run
`branchsight serve` from a terminal. If `~/.local/bin` is not on your `PATH`, add it to your shell profile.

Then open `http://localhost:7420`. If it answers `cannot listen on 127.0.0.1:7420: Address already in use`, another
program holds the port: pick another with `--port 7421`.

## Upgrade a Linux package to a build with the same version

`dnf install` skips a package whose version is already installed. To replace it with a newer build of the same
version:

```bash
sudo dnf reinstall ./BranchSight-*.rpm
```

## From source

You need Rust (stable) and Node 20 or later. On Linux, also the system webview's development files:

```bash
sudo dnf install webkit2gtk4.1-devel openssl-devel librsvg2-devel libxdo-devel   # Fedora
sudo apt install libwebkit2gtk-4.1-dev libssl-dev librsvg2-dev libxdo-dev        # Debian, Ubuntu
```

Then build the app and its installers:

```bash
git clone https://github.com/acm363/branchsight.git
cd branchsight
scripts/check.sh                          # tests, then a build of the app
cd app && npx @tauri-apps/cli@2 build     # installers in target/release/bundle/
```

`cargo build --release` alone builds `branchsight`, the command behind
[`branchsight serve`](../reference/serve.md).
