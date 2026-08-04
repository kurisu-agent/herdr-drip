#!/usr/bin/env bash
# Point ~/.config/herdr/config.toml at the drip's tracked config, so config
# edits land in the repo. Backs up an existing untracked config first.
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
src="$repo/config/herdr.toml"
target="$HOME/.config/herdr/config.toml"

mkdir -p "$(dirname "$target")"
if [[ -e $target && ! -L $target ]]; then
  cp "$target" "$target.bak"
  echo "backed up existing config to $target.bak"
fi
ln -sf "$src" "$target"
echo "herdr config -> $src"

herdr server reload-config >/dev/null 2>&1 && echo "reloaded" || true
