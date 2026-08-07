# herdr-drip

A custom drip for [herdr](https://herdr.dev) — the plugins we run, bundled
into a single repo.

Herdr installs plugins with the `owner/repo/subdir` shorthand, so one repo can
carry the whole set: **each top-level directory here is one plugin** (a
directory with a `herdr-plugin.toml` manifest and the commands it launches).

## Install

Each plugin installs individually from this repo:

```
herdr plugin install kurisu-agent/herdr-drip/hello
```

## Develop

Link your working tree instead of installing — changes are live, no reinstall:

```
herdr plugin link ~/Code/herdr-drip/hello
```

or link every plugin in the repo at once:

```
./scripts/link-all.sh
```

Note that `herdr plugin link` does NOT run the manifest's `[[build]]` steps —
build your tree by hand before testing. Useful while iterating:

```
herdr plugin action list --plugin drip.hello
herdr plugin action invoke drip.hello.greet
herdr plugin log list --plugin drip.hello
```

## Config

`config/herdr.toml` is the drip's curated herdr config — keybindings layered
under zellij (`Ctrl+b r`/`d`/`x` for split/close, `Alt+Shift+arrows` for pane
focus), sidebar rows wired to the plugins' `$worktree` token, and the
experimental kitty-graphics switch. Adopt it with:

```
./scripts/apply-config.sh
```

which backs up any existing config and symlinks `~/.config/herdr/config.toml`
into the repo, so future config edits are tracked here.

The config is path-free: keybindings reach the plugins through
`herdr plugin action invoke`, and `default_shell` is a PATH-resolved
`yolo-shell`. `apply-config.sh` links `scripts/yolo-shell` into
`~/.local/bin` unless something already provides it — nix users can take it
from the flake instead:

```
nix profile add github:kurisu-agent/herdr-drip#yolo-shell
```

Caveat: `default_shell` (and every other bare command in the config and
plugin manifests) is resolved against the herdr **server's** PATH, not your
shell's — and a server launched by systemd or nix usually has no
`~/.local/bin`, so the fallback link only works on hosts whose server was
started from a shell that has it. `apply-config.sh` reads the running
server's PATH and warns when a needed command won't be visible.

## Nix

Every runtime dependency the drip's plugins and integrations need, in one
profile add (all of these resolve against the server's PATH, see above):

```
nix profile add github:kurisu-agent/herdr-drip#herdr-drip-deps
```

That is `yolo-shell` (default_shell), `bun` (worktree-graph's build and
pane command), and `python3` (herdr's claude agent-state hook execs an
inline python heredoc — and silently no-ops without it).

### NixOS + nix-claude-drip: keeping the claude integration alive

`herdr integration install claude` adds a `hooks.SessionStart` entry to
`~/.claude/settings.json` — and nix-claude-drip installs that file by
wholesale overwrite on every rebuild and boot, silently erasing the entry
while `herdr integration status` keeps reporting `current` (it only stats
the hook script). Hosts running `services.claude-code` should import the
drip's module instead of installing by hand:

```nix
# flake input: herdr-drip.url = "github:kurisu-agent/herdr-drip";
imports = [ herdr-drip.nixosModules.claude-agent-state ];
services.herdr-drip.claudeAgentState.enable = true;
```

The module declares the hooks entry through `services.claude-code.settings`
(so the overwrite carries it instead of destroying it), injects a nix-store
`python3` into that one hook command's PATH (the script hard-requires it
but nothing else should see it), and re-runs `herdr integration install
claude` on activation whenever herdr reports the hook script missing or
outdated — so the script side tracks herdr's integration version rather
than pinning a copy that would go stale.

## Adding a plugin

Copy `hello/` to a new top-level directory and edit:

- `id` — namespaced as `drip.<name>`; herdr qualifies action ids globally as
  `<plugin-id>.<action-id>`.
- Commands can be any executable — bash, node, python, a compiled binary.
  There is no SDK; the herdr CLI (at `$HERDR_BIN_PATH`) is the API.
- Put user-editable config under `$HERDR_PLUGIN_CONFIG_DIR` and runtime state
  under `$HERDR_PLUGIN_STATE_DIR`; the source checkout (`$HERDR_PLUGIN_ROOT`)
  is read-only once installed.

Manifest sections: `[[build]]` (run at install), `[[startup]]` (after session
restore), `[[actions]]` (user-invokable), `[[events]]` (hooks on herdr
events), `[[panes]]` (terminal pane UIs), `[[link_handlers]]` (URL click
interceptors). See [the plugin docs](https://herdr.dev/docs/plugins/).
