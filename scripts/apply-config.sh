#!/usr/bin/env bash
# Point ~/.config/herdr/config.toml at the drip's tracked config, so config
# edits land in the repo. Backs up an existing untracked config first.
#
# THIS IS THE NON-NIXOS BOOTSTRAP. On a host running the drip's NixOS module
# it is not a shortcut, it is a way to break the box -- see the guard below.
set -euo pipefail

FORCE=0
for arg in "$@"; do
  case $arg in
    --force) FORCE=1 ;;
    *)
      echo "apply-config.sh: unknown argument '$arg'" >&2
      exit 2
      ;;
  esac
done

repo="$(cd "$(dirname "$0")/.." && pwd)"
src="$repo/config/herdr.toml"
target="$HOME/.config/herdr/config.toml"

# ---------------------------------------------------------------------------
# Module-managed host guard.
#
# Everything this script does, nix/plugins.nix already does declaratively and
# better: the config comes from the store, yolo-shell and bun are on the system
# PATH, the plugins are published under /etc/herdr-drip/plugins. Running the
# imperative bootstrap ON TOP of that does not merely duplicate it -- it
# SHADOWS it, and the shadow wins.
#
# The failure this prevents, in full (triforce-dev, 2026-08-12): the README's
# `nix profile add ...#yolo-shell` put a yolo-shell in ~/.nix-profile/bin,
# which the herdr server's PATH orders AHEAD of /run/current-system/sw/bin.
# The profile copy was pinned to an old rev whose yolo-shell was still a stub
# that ran `claude --dangerously-skip-permissions` directly, so every new pane
# and every split silently launched an UNROUTED claude -- no gumbo account
# pool, no per-launch session id. And because `nixos-rebuild` neither reads nor
# writes the imperative profile, no rebuild and no reboot could ever correct
# it; it survived both. The tell is two yolo-shells resolving to different
# store paths, which nothing surfaces on its own.
#
# So on a module-managed host: refuse. --force is there because a refusal you
# cannot override is its own trap, not because overriding is a good idea.
if ((!FORCE)) && { [[ -d /etc/herdr-drip/plugins ]] || [[ -x /run/current-system/sw/bin/yolo-shell ]]; }; then
  cat >&2 <<'EOF'
apply-config.sh: this host already runs the drip's NixOS module — refusing.

  /etc/herdr-drip/plugins/ or /run/current-system/sw/bin/yolo-shell is present,
  which means services.herdr-drip.plugins already manages the config, the
  plugins and yolo-shell from the store.

  Running this anyway layers an imperative copy over the declarative one. The
  copy is pinned to whatever rev it was installed from and takes PATH
  precedence, so the box keeps running the OLD yolo-shell through every
  rebuild and reboot — which on a gumbo host means panes launch an unrouted
  claude with no account pool. That is a real incident, not a hypothetical.

  Change config in the module instead (services.herdr-drip.plugins.settings),
  or `nix profile remove yolo-shell` first if you are undoing this.

  --force overrides, if you know why you want it.
EOF
  exit 1
fi

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
    echo "  worktree-graph and agent-state hooks (hosts using the drip's NixOS" >&2
    echo "  module get python3 injected for the claude hook and can ignore that" >&2
    echo "  one). Fix with" >&2
    echo "    nix profile add github:kurisu-agent/herdr-drip#herdr-drip-deps" >&2
    echo "  or restart the herdr server from a shell whose PATH provides them." >&2
  fi
fi

herdr server reload-config >/dev/null 2>&1 && echo "reloaded" || true
