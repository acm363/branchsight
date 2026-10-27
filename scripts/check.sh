#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

for tool in git cargo node; do
  command -v "$tool" >/dev/null || { echo "missing: $tool" >&2; exit 1; }
done

cargo test
node --test tests/js/*.test.mjs

if [[ "$(uname)" != Linux ]] || pkg-config --exists webkit2gtk-4.1 2>/dev/null; then
  cargo build --release -p branchsight-app
else
  echo "skipped the desktop app build: webkit2gtk-4.1 development files are not installed (see README)" >&2
fi
