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
under zellij (`Ctrl+b r`/`d`/`x` for split/close, `Ctrl+b [` to flip a split,
`Alt+Shift+arrows` for pane focus), sidebar rows wired to the plugins'
`$worktree` token, and the experimental kitty-graphics switch. Adopt it with:

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

### Which launcher a pane gets

`yolo-shell` starts panes through, in order: `$YOLO_SHELL_LAUNCHER` if set,
else a `yolo` command if one is on PATH, else plain
`claude --dangerously-skip-permissions`. Nothing here knows what `yolo` is —
on our hosts it is [gumbo](https://github.com/kurisu-agent/gumbo)'s launcher,
which routes claude through a local multi-account gateway and stamps a
per-launch session id, but any wrapper works.

Set `YOLO_SHELL_LAUNCHER` in the herdr **server's** environment (panes inherit
it) to name one explicitly — a differently-named wrapper, an absolute path, or
`claude --dangerously-skip-permissions` to opt a host out of a `yolo` that
happens to be on its PATH. It is a command line, not a path: extra words ride
along as leading arguments.

```
YOLO_SHELL_LAUNCHER="/opt/bin/my-claude-wrapper --profile work" herdr
```

`yolo-shell` also cooperates with herdr's native agent session restore:
after a server restart, herdr assumes `default_shell` is a plain shell and
types each pane's claude command into it. Since our panes boot straight into
claude, the script peeks stdin before launching and replays that injected line
through the same launcher, so the pane comes back resumed instead of the line
landing in a fresh claude's chat box.

That replay matches the injected command by BASENAME. herdr injects whatever
path it launched the agent with, and since 0.8.0 that is an absolute one
(`/home/you/.claude/cc/current/claude --resume <id>`) — matching the bare word
`claude`, as this script used to, silently missed it and ran that path
directly. Panes looked fine and were simply unrouted: no gateway, no session
id. Anything that is not a claude still runs verbatim.

### Panes that were shells come back as shells

herdr only injects that line for panes that had an agent session; it respawns
every OTHER pane with a bare `default_shell` — so a pane you had dropped out of
claude to use as a terminal came back as a fresh claude, the restore quietly
overwriting it. herdr's own state can't tell that pane from a brand new one: it
clears `agent_session` when the agent exits, and the pane's label keeps claude's
stale terminal title.

So `yolo-shell` remembers, per pane, whether it is running the agent or a plain
shell, under `${XDG_STATE_HOME:-~/.local/state}/herdr-drip/panes/<session>/`.
On a spawn with nothing injected it starts claude as always — unless the record
says this pane was a shell, which only happens on a restore, since a brand new
pane has no record at all.

`$HERDR_PANE_ID` is the key. herdr persists each workspace's public pane numbers
and hands the same ones back after a restore, and the counter is monotonic —
closing a pane never frees its number for reuse — so a record can never be read
by a pane other than the one that wrote it. Named sessions have their own id
space, hence the per-session directory. Records for panes that are gone are
pruned after 30 days; every restore rewrites the ones still alive.

Records only exist for panes started by a `yolo-shell` that has this, so panes
already open when you adopt it come back as claude once, then record themselves.

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

### NixOS: the whole drip, declaratively

On a host running
[nix-claude-drip](https://github.com/kurisu-agent/nix-claude-drip), the
standard procedure is one knob upstream — the drip pins this repo and
composes everything:

```nix
services.claude-code.herdr.enable = true;
```

That installs nix-claude-drip's pinned herdr **with the hardcore-plugin
patch set below applied**, and enables both of this repo's modules: the
plugin provisioning and the claude-agent-state hook keepalive.

A host wiring herdr itself imports the modules directly instead:

```nix
# flake input: herdr-drip.url = "github:kurisu-agent/herdr-drip";
imports = [ herdr-drip.nixosModules.plugins ];
services.herdr-drip.plugins.enable = true;
```

Either way, on every rebuild and boot the plugins module keeps each drip
plugin installed **pinned to the herdr-drip rev the consumer locked**
(bumping the input bumps the plugins), keeps `~/.config/herdr/config.toml`
a symlink to the drip's config (curated defaults + your overrides, below),
and puts `yolo-shell` + `bun` on the system PATH the herdr server resolves
against. `python3` stays off the PATH — the claude-agent-state module below
injects it scoped to the one hook that needs it.

Every curated setting is a **default, not a mandate**: the managed config is
generated from `services.herdr-drip.plugins.settings` (freeform TOML as Nix
values) with `config/herdr.toml` layered underneath key by key, so
overriding one setting keeps all the others:

```nix
services.herdr-drip.plugins.settings = {
  ui.tab_bar_position = "top"; # curated default: "bottom"
  theme.name = "gruvbox";
};
```

Lists override wholesale — `keys.command` is one value, not one per entry —
and `lib.mkForce` on a subtree drops its curated contents entirely. The
generated file is comment-free; the commentary lives in the tracked
`config/herdr.toml`, which is still exactly what `apply-config.sh` links on
non-nix hosts (where overriding means editing your linked checkout).

It never touches: plugins linked from a working tree (`herdr plugin link`
wins — that is the dev loop), third-party plugins, a plugin's
enabled/disabled state (except that a rev bump reinstalls in place, which
re-enables), or a config.toml that is a plain file or a symlink outside the
store (`apply-config.sh`'s link into a checkout stays). `link-all.sh` and
`apply-config.sh` remain the working-tree dev loop this module defers to.

`herdr-drip.nixosModules.default` is both halves — this module plus the
claude-agent-state module below.

### NixOS + nix-claude-drip: keeping the claude integration alive

`herdr integration install claude` adds a `hooks.SessionStart` entry to
`~/.claude/settings.json` — and nix-claude-drip installs that file by
wholesale overwrite on every rebuild and boot, silently erasing the entry
while `herdr integration status` keeps reporting `current` (it only stats
the hook script). `services.claude-code.herdr.enable` (above) turns this
module on automatically; a host wiring it by hand imports it directly
instead of installing by hand:

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

## flip-split: why the round trip

`Ctrl+b [` flips the split holding the focused pane — two columns become two
rows and back. herdr has no rotate or transpose command, and the obvious
routes are both dead ends: `layout.apply` can express any orientation but
builds a **new** tab with fresh terminals and closes the old one, which would
kill the agents running in those panes, and `pane move` within the same tab
short-circuits as a no-op (`PaneMoveReason::SameTab`).

What does work is a round trip through a scratch tab:

```
herdr pane move <second> --new-tab
herdr pane move <second> --tab <original> --target-pane <first> --split down
```

Both hops cross a tab boundary, so neither is a no-op, and herdr re-parents
the **live** pane rather than respawning it, so the agents keep running. The
scratch tab deletes itself when its last pane leaves, and omitting `--focus`
leaves focus where it was.

The plugin always moves the second (right/bottom) pane onto the first, so the
visible order survives regardless of which pane is focused. It only acts when
the split's two children are both panes — a move relocates one pane, not a
subtree — so inside a nested layout it flips the leaf pair you are in and
says so when the sibling is a whole group.

Note that `Ctrl+b [` is herdr's default copy-mode key; the config unbinds
copy mode to free it.

## Hardcore plugins — patches on herdr itself

Some of our opinions have no plugin surface to land on: sidebar chrome,
built-in labels, behavior compiled into the binary. Those become **hardcore
plugins** — source patches on herdr, curated in `nix/herdr-patches.nix` the
same way the plugin directories curate everything else. Current set:

- **sidebar-version** — the workspace-list header says `herdr <version>`
  instead of the hardcoded `" spaces"`, so the running version is visible
  somewhere in the UI.

They apply as one function, so every host gets the identical set:

```nix
herdr-drip.lib.patchHerdr herdrPkg
```

nix-claude-drip's herdr knob applies it by default to whatever
`services.claude-code.herdr.package` resolves to (opt out with
`herdr.dripPatches = false`) — so a host overriding the package supplies an
**unpatched** build and lets the module patch it; applying the set twice is
a build error by design.

Rules for adding one (they live as comments in the file too): patch with
`substituteInPlace --replace-fail` or a context patch so a herdr bump that
breaks the patch **fails the build loudly** instead of silently shedding it;
give each patch a one-paragraph story (what it changes, why it can't be a
real plugin); and when herdr grows a surface for it, graduate it into a
plugin directory.

Note the running herdr server keeps its old binary across a rebuild — the
patch (like any herdr bump) appears after the server restarts.

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
