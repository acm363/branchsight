# Update the screenshots

The images in `docs/images/` are generated from a demo repository with fictional authors, so they never show a real
project. Regenerate them after a visual change:

```bash
cargo build --release
demo=$(mktemp -d)
scripts/demo-repo.sh "$demo"
node scripts/screenshots.mjs target/release/branchsight "$demo" docs/images
rm -rf "$demo"
```

The script starts `branchsight serve` on port 7499 with the demo folder as its only root, drives a headless
Chromium (`chromium-browser` on the `PATH`) and writes one PNG per scene at twice the screen resolution. Scenes,
window size and theme are listed at the top of `scripts/screenshots.mjs`.
