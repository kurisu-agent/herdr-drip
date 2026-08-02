#!/usr/bin/env bash
# Link every plugin in this repo into the local herdr — the dev loop.
# `herdr plugin link` is idempotent per directory, so rerunning is safe.
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"

for manifest in "$repo"/*/herdr-plugin.toml; do
  dir="$(dirname "$manifest")"
  echo "linking $(basename "$dir")"
  herdr plugin link "$dir"
done
