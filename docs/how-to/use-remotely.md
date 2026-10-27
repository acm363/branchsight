# Use BranchSight with WSL, SSH or a container

The desktop app reads repositories on the machine it runs on. When the repository lives somewhere else, run the same
page there with `branchsight serve` and open it in a browser.

## Inside WSL

In the WSL distribution, build or download the `branchsight` command, then:

```bash
branchsight serve --root ~/code
```

Open `http://localhost:7420` in your Windows browser; WSL forwards `localhost` to Windows.

## On a server over SSH

```bash
ssh -L 7420:localhost:7420 my-server 'branchsight serve --root ~/repos'
```

Then open `http://localhost:7420` on your own machine. The server listens on its loopback interface only, so the SSH
tunnel is the only way in.

## In a dev container or Codespace

Run `branchsight serve --root /workspaces` in the container and let your desktop editor forward port 7420 to
`localhost`. A Codespace opened in the browser forwards it under a `*.app.github.dev` name instead, which is not a
loopback name, so BranchSight answers `403`: use the Codespace from a desktop editor.

## Keep it running on Linux

`contrib/branchsight.service` is a systemd user unit for `branchsight serve` on port 7420, scanning your home
folder. It runs `~/.local/bin/branchsight`:

```bash
ln -s "$PWD/target/release/branchsight" ~/.local/bin/branchsight
systemctl --user enable --now "$PWD/contrib/branchsight.service"
systemctl --user restart branchsight          # after updating the binary
```

All options are in the [`branchsight serve` reference](../reference/serve.md).
