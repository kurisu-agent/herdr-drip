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
