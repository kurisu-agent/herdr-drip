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

# default_shell is the PATH-resolved `yolo-shell`; link the repo's copy in
# unless something (e.g. the drip's nix flake) already provides it.
if ! command -v yolo-shell >/dev/null; then
  mkdir -p "$HOME/.local/bin"
  ln -sf "$repo/scripts/yolo-shell" "$HOME/.local/bin/yolo-shell"
  echo "yolo-shell -> ~/.local/bin/yolo-shell"
fi

# Bare commands in the config and plugin manifests — default_shell's
# yolo-shell, worktree-graph's bun, the claude hook's python3 — resolve
# against the herdr SERVER's PATH, not this shell's, and a server launched
# by systemd or nix typically has no ~/.local/bin. Check the live server's
# environment (Linux only) and say so, instead of leaving panes that fail
# to spawn with a confusing error.
server_pid=$(pgrep -f 'herdr server' 2>/dev/null | head -n1 || true)
if [[ -n $server_pid && -r /proc/$server_pid/environ ]]; then
  server_path=$(tr '\0' '\n' <"/proc/$server_pid/environ" | sed -n 's/^PATH=//p')
  missing=()
  for cmd in yolo-shell bun python3; do
    PATH=$server_path command -v "$cmd" >/dev/null || missing+=("$cmd")
  done
  if ((${#missing[@]})); then
    echo "WARNING: not on the herdr server's PATH: ${missing[*]}" >&2
    echo "  yolo-shell missing means new panes fail to spawn; bun/python3 break" >&2
    echo "  worktree-graph and the claude agent-state hook. Fix with" >&2
    echo "    nix profile add github:kurisu-agent/herdr-drip#herdr-drip-deps" >&2
    echo "  or restart the herdr server from a shell whose PATH provides them." >&2
  fi
fi

herdr server reload-config >/dev/null 2>&1 && echo "reloaded" || true
