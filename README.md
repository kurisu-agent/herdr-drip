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
`$worktree` token, a tab bar that hides itself while there is only one tab
(`hide_tab_bar_when_single_tab` — one tab is not a choice, so the row showing
it is a line of terminal spent on nothing), and the experimental
kitty-graphics switch. Adopt it with:

```
./scripts/apply-config.sh
```

which backs up any existing config and symlinks `~/.config/herdr/config.toml`
into the repo, so future config edits are tracked here.

The config is path-free: keybindings reach the plugins through
`herdr plugin action invoke`, and `default_shell` is a PATH-resolved
`yolo-shell`. `apply-config.sh` links `scripts/yolo-shell` into
`~/.local/bin` unless something already provides it.

**On NixOS, use the module instead — not a profile install.** The module
(below) puts `yolo-shell` on the system PATH, and `apply-config.sh` refuses to
run on a host where it is already active.

> **Never `nix profile add …#yolo-shell` on a module-managed host.** The herdr
> server's PATH orders `~/.nix-profile/bin` **ahead of**
> `/run/current-system/sw/bin`, so an imperatively installed `yolo-shell` wins
> — and it stays pinned to the rev it was installed from. When that pinned
> copy is older than the flake, every new pane and split runs whatever
> `yolo-shell` used to be. On triforce-dev (2026-08-12) that was a stub
> calling `claude --dangerously-skip-permissions` directly, so panes launched
> an **unrouted** claude: no gumbo account pool, no per-launch session id.
> `nixos-rebuild` neither reads nor writes the imperative profile, so this
> survived a rebuild *and* a reboot. Diagnose with
> `nix profile list | grep yolo`; fix with `nix profile remove yolo-shell`.

For a non-NixOS host with no other source of it, the flake still packages one:

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

### Panes that were born shells

A pane herdr spawned with `HERDR_DRIP_PANE_KIND=shell` in its launch env skips
claude entirely and lands in your interactive shell. That is what the
**shell-panes** hardcore patch sets on new workspaces, new tabs, worktrees it
opens and the pane menu's `Shell` splits — the drip's herdr sets one
variable, and everything that knows what a shell is lives here.

It is read before the stdin peek above, not folded into it: that peek spends
half a second waiting for a resume line which cannot arrive for a pane created
a moment ago (herdr injects one only for a pane that *had* an agent session),
and half a second on every new workspace is a tax for nothing. The variable is
then unset, so the pane's own environment does not hand a herdr-internal
marker to everything you run in it, and the pane records itself as a shell —
which is what brings it back as one after a restore, since herdr persists no
launch env.

Nothing here is required: with no such variable set, yolo-shell behaves
exactly as it always did.

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
plugin registered **from the herdr-drip revision the consumer locked**
(bumping the input bumps the plugins), keeps `~/.config/herdr/config.toml`
a symlink to the drip's config (curated defaults + your overrides, below),
and puts `yolo-shell` + `bun` on the system PATH the herdr server resolves
against. `python3` stays off the PATH — the claude-agent-state module below
injects it scoped to the one hook that needs it.

**Nothing is fetched at activation.** Each plugin is a store path built from
the flake source, published at `/etc/herdr-drip/plugins/<name>` and registered
with `herdr plugin link` — so the module provisions a host that has never run
herdr and has no route to GitHub, the herdr server does not have to be up, and
a dirty checkout works exactly like a clean one. The one manifest `[[build]]`
in the drip (worktree-graph's `bun install`) is a fixed-output derivation,
`nix/worktree-graph-deps.nix`; when `bun.lock` moves, rebuild it and paste the
hash nix reports:

```
nix build github:kurisu-agent/herdr-drip#worktree-graph-node-modules
```

Plugins from outside this repo can ride the same mechanism — store path,
published under `/etc`, linked, never fetched:

```nix
services.herdr-drip.plugins.extraPlugins.my-plugin = {
  id = "acme.my-plugin";              # stated, not read: see the option docs
  path = inputs.acme-plugins + "/my-plugin";
};
```

and `services.herdr-drip.plugins.plugins = [ ]` provisions none of the drip's
own, leaving you to `herdr plugin install` whatever you like by hand.

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
wins — that is the dev loop, recognised by the plugin root being a mutable
path rather than a store one), plugins not named in `plugins`, or a
config.toml symlinked outside the store (`apply-config.sh`'s link into a
checkout stays). `link-all.sh` and `apply-config.sh` remain the working-tree
dev loop this module defers to.

It **does** take over a config.toml it did not put there, because a deployment
should run the config its flake pin describes. Two things can be in the way and
`adoptConfig` decides how far it goes:

- a **plain file** is herdr's own — it writes this file itself, stamping
  `onboarding = false` when onboarding completes and again from the theme and
  sound pickers — so on any host where herdr ran before this module first did,
  the file already exists. It is moved to `config.toml.bak` (timestamped rather
  than overwriting an existing backup) and replaced.
- a **symlink outside the store** is `apply-config.sh`'s, pointing into a
  checkout. Under the default `adoptConfig = "always"` the **link** is
  replaced; the checkout's file is never followed, written, or removed. This
  matters more than it looks: a checkout that stops being pulled silently pins
  that host's herdr to whatever the drip looked like the day someone ran the
  script, and the only symptom is settings that quietly do not apply.

**On a host the drip is developed on, set `adoptConfig = "plain-file"`** — that
keeps the linked checkout, which is the dev loop, while still rescuing a host
from herdr's own file. `"never"` leaves anything that exists alone and reports
the mismatch on stderr; `manageConfig = false` opts out of the config entirely.

`herdr-drip.nixosModules.default` is both halves — this module plus the
claude-agent-state module below.

### Where everything lands

Every path the two modules read, write, or publish. `~/.config/herdr` is
whatever `$XDG_CONFIG_HOME/herdr` resolves to, and `~/.local/state` likewise
`$XDG_STATE_HOME` — herdr honours both.

Written by nix, owned by the system:

| Path | What |
| --- | --- |
| `/etc/herdr-drip/plugins/<name>` | Symlink to each provisioned plugin's store path. Also what keeps those paths from being garbage-collected out from under a running herdr — nothing else in the system closure refers to them. |
| `/run/current-system/sw/bin/yolo-shell` | `default_shell`, resolved off the herdr **server's** PATH. |
| `/run/current-system/sw/bin/bun` | worktree-graph's pane command. |
| `herdr-drip-plugins-<user>.service` | Per-user oneshot backstop, for hosts with no systemd user manager. Same script as the user activation script `herdrDripPlugins`. |
| `herdr-drip-claude-agent-state-<user>.service` | The same, for the claude-agent-state module (`herdrClaudeAgentState`). |

Written by the modules, per user:

| Path | What |
| --- | --- |
| `~/.config/herdr/config.toml` | Symlink to the generated config in the store. |
| `~/.config/herdr/config.toml.bak` | Whatever was there before adoption. Timestamp-suffixed if a `.bak` already exists — never overwritten. |
| `~/.config/herdr/plugins.json` | herdr's plugin registry. The module adds/updates one entry per provisioned plugin (as `link`s) and reads it to recognise what it must not touch. |
| `~/.config/herdr/plugins/github/<id>-<hash>/` | herdr's fetched checkouts. The module creates none of these any more, and **deletes** one only when replacing a registration it can prove was its own (`kurisu-agent/herdr-drip`, under this directory). A third-party checkout is never removed. |
| `~/.claude/settings.json` | claude-agent-state's `hooks.SessionStart` entry, declared through nix-claude-drip so its rewrites carry it. |
| `~/.claude/hooks/herdr-agent-state.sh` | Installed by `herdr integration install claude`, re-run on activation when herdr reports it missing or outdated. |

Written by herdr and the drip's own runtime pieces — the modules do not manage
these, but this is where a plugin's state actually is:

| Path | What |
| --- | --- |
| `~/.config/herdr/plugins/config/<plugin-id>/` | Per-plugin config dir (`$HERDR_PLUGIN_CONFIG_DIR`, `herdr plugin config-dir <id>`). |
| `~/.local/state/herdr/plugins/<plugin-id>/` | Per-plugin state dir (`$HERDR_PLUGIN_STATE_DIR`). This is why a read-only store path works as a plugin root: nothing writes into the plugin directory itself. |
| `~/.local/state/herdr-drip/sidebar-accounts.txt` | What gumbo-usage writes and the sidebar-accounts patch reads — the only thing connecting them. Override with `$HERDR_DRIP_ACCOUNTS_FILE`. |
| `~/.local/state/herdr-drip/panes/<session>/<pane-id>` | yolo-shell's record of whether a pane is running the agent or a plain shell, so session restore puts back what was there. |
| `~/.config/herdr/{herdr.sock,*.log,session*.json,.plugins.lock}` | herdr's own. Never touched. |

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

## gumbo-usage: how much Claude you have left, in the sidebar

Under the agent list, one rail per Anthropic account: a traffic-light dot, the
5h window's meter and when it resets, the 7d window under it.

```
──────────────────────
 accounts

▸● hext2   ▰▰▱▱▱  11m
        7d ▰▰▰▰▱   5d
 ● hext3   ▱▱▱▱▱ 3h31
        7d ▱▱▱▱▱   6d
 ○ hext1   inference
```

Two markers can appear in the column left of the dot, and they answer different
questions:

- **`▸` — the pick.** Where gumbo's strategy would put a session started right
  now. It is a moving target by design: `balance` spreads new sessions by
  assigned-key count, so opening one session usually moves the marker to
  another account.
- **`⚑` — a pin.** `gumbo use <name>` overrides the strategy entirely, so this
  says both where the next session goes and why. `gumbo use auto` clears it.
  A pinned account never shows `▸` — the pin outranks it.

Neither marker says where the sessions you already have are running: gumbo
keeps a session on its account for as long as that account is viable (moving
one would re-pay its whole prompt cache), so a running session's account is its
own fact. `gumbo sessions` is what lists those.

Two colours per account row, split at the meter's first cell. The **dot, the
marker and the name** are the account, so they take the worst of its rows: a
walled 5h or 7d turns them red. The **meter and its reset** are one window, so
they keep their own grade — which is why a red name can sit beside an honest
green 5h meter, with the red explained by the 7d row under it. One dot cannot
say "roomy hour, spent week", and of the two answers only the pessimistic one
is safe to start a session on.

Collapse the sidebar and the rail collapses with it, to a numbered dot per
account in the same three columns the agents use.

Three pieces, deliberately:

- **[gumbo](https://github.com/kurisu-agent/gumbo) `watch`** does the reading.
  It caches to `~/.cache/gumbo` and only goes to Anthropic's usage endpoint
  when that cache has aged out, so the rail redraws every 5s while the
  endpoint is read every 5 minutes — which matters, because that endpoint's
  rate limit is per IP and losing it blinds every account on the box at once.
  The two cadences are independent on purpose: the meters change slowly, but
  `▸` moves the moment a new session is placed, and only the daemon knows.
- **`drip.gumbo-usage`** (this plugin) runs `gumbo watch --format compact
  --tags --out <file>` from a `[[startup]]` hook and keeps it alive.
- **the `sidebar-accounts` hardcore patch** reads that one file and draws it.

The patch knows nothing about gumbo — it reads tagged lines
(`<severity><kind> <text>`) and paints them. Anything that writes that format
feeds the rail, and when nothing does, every function in it returns empty and
the sidebar is byte-identical to stock herdr. A file older than ten minutes
counts as nothing: a watcher that died leaves its last frame behind, and
hour-old headroom shown as current is the number you would act on.

Overrides, for a wider sidebar or a different cadence — set them in the herdr
**server's** environment:

```
HERDR_DRIP_ACCOUNTS_FILE      # default: $XDG_STATE_HOME/herdr-drip/sidebar-accounts.txt
HERDR_DRIP_ACCOUNTS_WIDTH     # default: 21 (fits herdr's 26-column sidebar)
HERDR_DRIP_ACCOUNTS_INTERVAL  # default: 5 (seconds between redraws, NOT between polls)
```

`gumbo` must be on the herdr server's PATH — it is not a dependency of this
repo, and a host without it simply gets no rail (the plugin says so once in its
log and exits). `herdr plugin action invoke sync --plugin drip.gumbo-usage`
redraws now, after a `gumbo login` or a `gumbo use`.

## Hardcore plugins — patches on herdr itself

Some of our opinions have no plugin surface to land on: sidebar chrome,
built-in labels, behavior compiled into the binary. Those become **hardcore
plugins** — source patches on herdr, curated in `nix/herdr-patches.nix` the
same way the plugin directories curate everything else. Current set:

- **sidebar-version** — the workspace-list header says `herdr <version>`
  instead of the hardcoded `" spaces"`, so the running version is visible
  somewhere in the UI.
- **sidebar-accounts** — the accounts rail above. herdr's plugin surface
  cannot reach the sidebar at all: pane placements are
  overlay/popup/split/tab/zoomed, and the sidebar's own row tokens are
  per-workspace and per-agent with nothing global under them. Note that the
  rows are carved off the agent panel in herdr's `*_sidebar_sections`, not in
  the renderer — those functions are what the click hit-testing and the scroll
  metrics ask where the agent panel is, so carving anywhere else would look
  right and mis-route every click on the rail.
- **last-close-quits** — closing the last pane stops the server, so closing
  your way out of herdr is how you reload it. See below.
- **shell-panes** — a new workspace or tab opens a terminal, not an agent.
  Splitting is what asks for claude. See below.
- **pane-menu** — the pane's right-click menu, rewritten: renamed rows in three
  separated groups, `New Tab` / `New Space` added, a shell and an agent split
  each way, three rows dropped, and a right-aligned icon column across every
  context menu. A plugin's `[[actions]]` reach the palette and the keybindings,
  never herdr's context menus, whose items are a `&'static str` list compiled
  into the binary. See below.
- **single-pane-borders** — a lone pane keeps its frame. Not the
  `ui.pane_borders` setting, which is already on: herdr ANDs it with a
  hardcoded `pane_count() > 1`. See below.

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

Loud is not the same as quick, though: the build fails in `postPatch`, which
is minutes of rust compile away from the mistake that caused it. Run the same
postPatch alone, against a herdr checkout, in about a second:

```
./scripts/check-herdr-patches.sh [path-to-herdr-checkout]   # default ~/Code/herdr
```

It lifts the body out of `nix/herdr-patches.nix` rather than keeping a copy,
applies it to a throwaway tree, names the first anchor that does not match,
and prints the patched hunks. Point it at the revision your consumer has
locked, not just at `main` — an anchor can match a checkout that is ahead of
the pin and still fail the build that ships. What it does not do is typecheck:
it says the anchors matched, not that the result compiles.

Note the running herdr server keeps its old binary across a rebuild — the
patch (like any herdr bump) appears after the server restarts, which is what
`last-close-quits` below makes a gesture rather than a chore.

### last-close-quits: closing the last pane is the reload

Close panes until none are left and herdr goes away entirely — client and
server. The next `herdr` is a new server, so it is running the binary you just
rebuilt.

- **Stock herdr sits in the empty state.** With the last workspace gone the
  sidebar empties and the server stays up, holding its session, its sockets
  and its old binary. Nothing short of `herdr server stop` from another
  terminal ends it, so a rebuilt herdr never reaches the screen.
- **It fires on both ways out.** The close pane/tab/workspace commands
  (`Ctrl+b x` and friends) and the pane's process simply exiting — `exit`,
  Ctrl+D, the agent quitting — are two different code paths in herdr, and both
  count as closing the last shell.
- **It is the same shutdown `server stop` performs.** The patch sets the same
  `should_quit` that the API method sets, so clients get `ServerShutdown`, the
  sockets are removed, and every terminal is restored the way a clean quit
  restores it.
- **Detach is unaffected** — `Ctrl+b d` still leaves the server running with
  everything in it. Detaching is how you keep a session; closing is how you
  end one. This patch only makes the second one mean what it says.
- **It cannot fire while a pane is left**, and it cannot fire on a server that
  has not opened a workspace yet: both anchors sit inside a close, not inside
  a check for an empty list.
- **The next launch is clean, not empty.** herdr saves no snapshot for an
  empty workspace list — it clears the session file instead — and seeds a
  workspace from the startup cwd when it finds none, so the new server opens
  one pane wherever you ran it rather than restoring the nothing you left.

The obvious caveat: your session is gone, because you closed it. Panes you
want back should be detached from, not closed.

### shell-panes: a new workspace is a terminal, a split is an agent

`default_shell` is `yolo-shell`, and yolo-shell starts claude — so before this
patch, every gesture that made a pane made an agent. Clicking `+` for
somewhere to run `git log` opened a claude session, and on a gumbo host that
is an account assignment and a prompt cache you did not ask for.

The split is by gesture, not by pane:

| Gesture | What starts |
| --- | --- |
| New workspace — the sidebar's `+`, the `new_workspace` key, the name prompt | shell |
| New tab | shell |
| `New worktree`, `Open worktree...`, and the workspace herdr seeds at startup | shell |
| `Agent Right` / `Agent Down`, and `Ctrl+b r`/`d` | claude |
| `Shell Right` / `Shell Down` (**pane-menu**) | shell |
| A session restore | whatever that pane was |

Asking for an agent is now a deliberate two-pane gesture, and the shell you
land in is a normal one — type `yolo` (or `claude`) and you have an agent in
that pane after all.

**The mechanism is herdr's own, and the patch knows nothing about shells.**
Every pane-spawning API call already carries a launch env to the new pane
(`herdr pane split --env`, `workspace create --env`); the patch sets exactly
one variable on the paths above:

```
HERDR_DRIP_PANE_KIND=shell
```

`yolo-shell` reads it and execs your interactive shell instead of the
launcher. A host whose `default_shell` is a plain shell inherits one unused
variable and behaves exactly like stock herdr — the same replaceability rule
`sidebar-accounts` follows.

**The API mostly keeps stock behaviour.** Two of the three anchors are
herdr's *TUI-side* mutations (`tui.workspace.create`, `tui.tab.create` and
friends), so a plugin or a `herdr workspace create` still gets an agent, and
one that wants a shell asks with `--env HERDR_DRIP_PANE_KIND=shell`; an
explicit `env` from the caller is never overwritten. The exception is opening
a worktree, which is one code path with no `env` on it at all — so
`herdr worktree open` gets a shell too, whoever ran it.

**It does not survive a restore, and does not need to.** herdr persists no
launch env, so a restored pane arrives without the variable — what carries
"this pane is a terminal" across a server restart is the per-pane record
yolo-shell already keeps for panes you dropped out of claude by hand (see
[Panes that were shells come back as shells](#panes-that-were-shells-come-back-as-shells)).
A shell pane records itself the moment it starts.

Nothing else on the pane menu starts anything: `Swap with focused pane`, `Zoom`
and `Close` rearrange panes that already exist. What the menu offers, and what
each row is called, is **pane-menu** below.

### pane-menu: the whole menu, renamed and reordered

```
 New Tab             
 New Space           
 ────────────────────
 Zoom                
 ────────────────────
 Agent Right         
 Agent Down          
 Shell Right         
 Shell Down          
 ────────────────────
 Close               
```

Three groups: what makes something new, what rearranges what is already there,
and the four ways to split. `Split right`/`Split down` are gone as names —
with **shell-panes** upstream, a split is the only gesture that starts claude,
so the rows that do it say `Agent` and the rows that do not say `Shell`.

**`New Tab` and `New Space` are not on herdr's pane menu at all.** They live on
the tab and sidebar menus, which is a trip to the sidebar for the two things
you most often want next to the pane you are looking at. Both reuse herdr's own
entry points, so they inherit its name prompts (`prompt_new_workspace_name` and
the tab-name dialog) and, through **shell-panes**, open shells rather than
agents.

**The icons are right-aligned, which is why the labels no longer carry them.**
herdr draws each row as `Line::from(item)`, so anything a row wants to say has
to be *in* its string — and a string cannot know how wide the popup will be.
`context_menu_rect` sizes that popup from the longest item, so the pad between
label and glyph is only knowable one frame at a time, in the renderer. Hence a
row builder (`nix/context-menu-render.rs`) and a vocabulary
(`nix/context-menu-items.rs`) instead of cleverer labels.

| Glyph | Codepoint | Name | Row |
| --- | --- | --- | --- |
|  | `U+EB23` | `cod-multiple_windows` | `New Tab` (and the tab menu's `New tab`) |
|  | `U+EB7F` | `cod-window` | `New Space` |
|  | `U+EB4C` | `cod-screen_full` | `Zoom` |
|  | `U+EBCB` | `cod-arrow_swap` | `Swap with focused pane` |
|  | `U+EB56` | `cod-split_horizontal` | `Agent Right`, `Shell Right` |
|  | `U+EB57` | `cod-split_vertical` | `Agent Down`, `Shell Down` |
|  | `U+EB94` | `cod-menu` | `Use Herdr right-click menu` |
|  | `U+EA76` | `cod-close` | `Close`, `Close group` |
|  | `U+EA73` | `cod-edit` | `Rename` |
|  | `U+EA80` | `cod-new_folder` | `New worktree` |
|  | `U+EAF7` | `cod-folder_opened` | `Open worktree...` |
|  | `U+EA81` | `cod-trash` | `Delete worktree checkout...` |
|  | `U+EAB4` | `cod-chevron_down` | `Expand` |
|  | `U+EAB7` | `cod-chevron_up` | `Collapse` |

All Codicons, from the `U+EA60`–`U+EBEB` block every Nerd Font since v2.3
carries — one family, so the column shares a weight, and off the Material
Design plane (`U+F0000`+) whose codepoints moved wholesale between Nerd Fonts
v2 and v3. The workspace, git-workspace and tab menus are in the table because
the renderer is shared: an icon column that appears on one menu and not the
next reads as a bug rather than as a choice. The words carry the meaning on
their own, so a terminal whose font lacks the block draws boxes and every row
still reads.

**A separator is not a row you can land on.** herdr's items are a flat
`Vec<&'static str>` with no notion of a rule, so being unselectable takes two
patches: the hit test refuses a separator (which backs both the click that
activates a row and the hover that highlights one, so a pointer crossing a rule
leaves the highlight where it was), and the keyboard steps over it
(`drip_menu_move`, because `MenuListState` is shared with menus that have no
separators and cannot see the items).

**`pane-menu-trim` used to be a patch of its own and is now an outcome.**
`drip_pane_menu` names every row it wants, so `Rename pane`, `Clear pane name`
and `Send right-clicks to pane` are absent by omission rather than removed by a
later pass. The reasons stand: a pane's label is the agent's terminal title,
which says what the pane is doing *now*, and a manual name freezes it at
whatever was true when you typed it (`herdr pane rename <pane_id>
<label>|--clear` still does it); and passthrough means this menu no longer
opens on this pane, so the click that turns it on is the click that hides the
way back. Only the *set* half went — `Use Herdr right-click menu` is carried
through whenever herdr offers it, because the CLI can still put a pane in
passthrough and the menu should remain the way out.

**The conditional rows stay herdr's decision.** `drip_pane_menu` is handed the
list stock just finished building and maps it to ours, so `Swap with focused
pane` still appears only mid-swap and the passthrough exit only in passthrough
— we decide order and wording, herdr decides what is on offer. One anchor, and
a herdr that grows a new conditional row tells us by dropping it, not by
mangling the order.

One cost, paid knowingly: herdr's own tests find menu rows by position or by
the string `Close pane`, and that row is now `Close`. The nix build does not
run them (`doCheck = false`) and neither does `check-herdr-patches.sh`, so
nothing here catches it — but a `cargo test` on a patched checkout would fail.

### single-pane-borders: a lone pane keeps its frame

**This one is not a setting, though it looks like it should be.** `ui.pane_borders`
and `ui.pane_outer_borders` are both true by default, and herdr ANDs them with
a hardcoded `pane_count() > 1` — so the only pane in a workspace has no frame
whatever the config says, and no key turns that off. A plugin cannot reach it
either: it is the renderer, three expressions deep in `src/ui/panes.rs`. Hence
a hardcore patch.

The frame is what says which pane has focus and where a pane ends, and a
workspace that opens with one pane spends its first minutes with neither.
Splitting in order to get a border is a poor trade.

The flag is patched where it is **defined**, not at its five use sites, so no
condition is rewritten: everything else `multi_pane` gates is already a no-op
for a lone pane — `pane_to_right`/`pane_below` find no neighbour, and the gap
shrink is keyed on having found one. The one use that is not about chrome is
`should_dim`, which greys *unfocused* panes; with one pane in the layout that
pane is the focused one, so it cannot fire either.

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
