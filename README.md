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
`Ctrl+b a` as a second way to widen or narrow the agent list — the icon in its
header is the first — `Ctrl+b B`/`Ctrl+b O` for the bd board docked or in a
tab, `Alt+Shift+arrows` for pane focus), sidebar rows wired to the plugins'
`$since`, `$space` and `$worktree` tokens (**reply-age**, below), a tab bar
that hides itself while there is only one tab
(`hide_tab_bar_when_single_tab` — one tab is not a choice, so the row showing
it is a line of terminal spent on nothing), the colour scheme (below), and the
experimental kitty-graphics switch. Adopt it with:

```
./scripts/apply-config.sh
```

which backs up any existing config and symlinks `~/.config/herdr/config.toml`
into the repo, so future config edits are tracked here.

The config is path-free: keybindings reach the plugins through
`herdr plugin action invoke`, and `default_shell` is a PATH-resolved
`yolo-shell`. `apply-config.sh` links `scripts/yolo-shell` into
`~/.local/bin` unless something already provides it.

### The colour scheme

`[theme.custom]` in the curated config is our palette — Catppuccin Mocha —
mapped onto herdr's seventeen theme tokens. It is the same palette the zellij
theme renders from, because the two stack on one screen, but two of the
mappings are herdr's own and worth knowing:

- **the chrome is transparent, not painted.** `panel_bg` and `sidebar_bg` are
  `reset` — the terminal's own background — rather than `mantle`. herdr is not
  the outermost thing on screen here, so an opaque plane a shade off from the
  shell around it is a visible seam for nothing. `theme.transparentChrome =
  false` paints them instead.
- **the accent avoids herdr's state colours.** herdr spells seven colours by
  name and every one already means something — green is a finished agent,
  yellow a working one, red one that needs you — so an accent equal to any of
  them paints "focused" and "finished" alike. The rule is therefore *take the
  palette's `accent` role, unless that role is a hue herdr has already spent on
  a state*; ours points at green, so herdr's accent redirects to `lavender`,
  the one accent rung herdr has no token for. Point the role at something
  outside that vocabulary and herdr follows it unchanged. It costs the
  agreement with zellij's green `ribbon_selected`, which is the smaller loss —
  zellij has no agent states for its accent to collide with.

`surface0` follows from the first of those: with both chrome planes
transparent it is the only opaque plane the sidebar and tab bar have left, so
it takes `bg_alt` (mantle) and reads as slightly darker than the terminal
behind it, rather than the mid-grey `surface0` (#313244) would put there.

**Change colours in the palette, not in the TOML.** `nix/theme.nix` holds the
mapping and generates the block; `config/herdr.toml` carries the render of it
for the default palette, because `apply-config.sh` links that file verbatim and
the non-nix path needs a theme too. On the nix path the module regenerates the
tokens and outranks the file, so the two cannot drift into a disagreement that
matters — and that they agree at all is one command to check:

```
nix-instantiate --eval -E '
  let t = import ./nix/theme.nix; c = builtins.fromTOML (builtins.readFile ./config/herdr.toml);
  in (t.mkTheme { palette = t.defaultPalette; }).theme.custom == c.theme.custom'
```

Keeping that `true` is the whole job: while it was false — four values in the
TOML hand-set past what the generator could emit — a workstation reading the
file and a kart reading the generated config could not be made to look alike by
any amount of bumping (`dr-50bg — A kart's herdr uses the palette defaults
where the workstation uses four hand-set values, so their colour schemes cannot
match`). So it is also `nix flake check` now — `checks.<system>.theme-render`
compares the same two blocks plus `ui.accent`, and prints the keys that differ
instead of just failing.

**But note which palette that command renders.** It renders the *vendored
default*, and no fleet host reads it: nix-env's `nixosModules.claude` sets
`services.herdr-drip.plugins.theme.palette = lib.mkDefault <nix-env's
palette>`, and an option definition at `mkDefault` outranks an option
*default*, so every fleet host and every kart is themed from nix-env's palette
— role layer included. A fix made to `defaultPalette` alone therefore ships
nothing and still checks green, which is exactly how `dr-gfxc — A kart's herdr
accent is still the palette's green, not the workstation's lavender — the last
of dr-50bg's four keys` happened. `checks.<system>.theme-accent-rule` covers
that shape: it rebuilds nix-env's role layer over our rungs and asserts the
accent rule holds — the collision redirects, a non-colliding role is followed
verbatim, a palette with no lavender degrades rather than inventing a colour,
and `ui.accent` tracks the token. If nix-env's roles move, that check is the
thing to read next to them.

`theme.auto_switch` stays off on purpose: custom tokens are applied on top of
whichever base theme is selected, so following the terminal into light mode
would paint Mocha chrome onto Latte rather than switch flavours. Pass a Latte
palette instead (see the module option below).

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

The colour scheme is an option rather than a wall of hex, and it takes a
palette as **data** — colour name to hex, in the shape
`nix-env/lib/palette.nix` produces, so passing ours through is one line:

```nix
services.herdr-drip.plugins.theme.palette = inputs.nix-env.lib.${pkgs.system}.palette;
```

Ours is the **default**, though, so a host that says nothing comes up in it
anyway; the option exists so a different palette *can* be passed, not so one
has to be. `nix/theme.nix` maps whatever arrives onto herdr's tokens — role
aliases first (`accent`, `bg_alt`, `bg_surface`, `primary`, `secondary`),
falling back to the Catppuccin names for a palette with no role layer — so
re-tinting means editing one palette, not this repo. `theme.enable = false`
generates no theme at all.

In practice a fleet host never exercises that default: nix-env's `claude`
module already sets this option at `mkDefault`, which outranks the option's
own default. So on any host in the drip chain, "the palette" means nix-env's,
and changing `defaultPalette` in this repo changes what a bare `nix eval`
prints and nothing else.

One thing a palette cannot carry, because every value in one is a hue:
**transparency**. `theme.transparentChrome` (default `true`) is that knob —
on, `panel_bg` and `sidebar_bg` are herdr's `reset` and inherit the terminal's
background; off, they take `bg_alt`. It is an option rather than a rung
because "no colour" is not a darker member of a colour set, and without it the
generator could not emit the drip's own appearance at all.

That default is *vendored*, and it has to be: nix-env depends on
nix-claude-drip, which depends on this repo, so a `nix-env` flake input here
would close a cycle. `nix/theme.nix` therefore copies the Catppuccin Mocha
rungs it reads — upstream values under upstream names — and every one of them
is overwritten by name the moment a palette is passed in. Wiring it up
properly would mean either a fourth repo holding the palette alone, which both
sides depend on, or nix-env keeping the one-liner above; the one-liner is
cheaper and is what the fleet does.

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
| `~/.local/state/herdr-drip/sidebar-beads.txt` | What the beads plugin writes and the sidebar-beads patch reads — the only thing connecting them. Override with `$HERDR_DRIP_BEADS_FILE`. |
| `~/.local/state/herdr-drip/sidebar-beads.open` | Whether the beads rail is open, so a click survives a restart. Override with `$HERDR_DRIP_BEADS_OPEN_FILE`. |
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
  --tags --out <file>` from a `[[startup]]` hook and keeps it alive — retrying
  with backoff for as long as the server lives, and writing a status row into
  the same file while it cannot.
- **the `sidebar-accounts` hardcore patch** reads that one file and draws it.

The patch knows nothing about gumbo — it reads tagged lines
(`<severity><kind> <text>`) and paints them. Anything that writes that format
feeds the rail.

**No file and a dead feed are different facts, and the rail says which.** With
no file at all every function returns empty and the sidebar is byte-identical
to stock herdr — that is the box with no gumbo, and it stays silent. A file
that exists but is older than ten minutes is a watcher that died: its last
frame is not shown (hour-old headroom presented as current is the number you
would act on), and in its place one grey row says how old the feed is. The
watcher writes the same kind of row itself while it is failing, so `⚠ feed
down` in the rail means the plugin is up and gumbo is not answering. This is
what `dr-vsv2 — The gumbo accounts rail never shows in a kart: the sidebar
patch renders a state file that nothing in a kart writes` cost: on a
workstation you notice a rail that vanished, and in a kart nobody is looking.

The one state still indistinguishable from "no gumbo here" is the plugin's
startup hook never running at all — nothing writes the file, so there is
nothing to read. That is a lifecycle problem rather than a display one, and
the kart's copy of it is fixed in drift-rust's guest layer, where `herdr
server` now starts after the drip's plugins are linked.

Overrides, for a wider sidebar or a different cadence — set them in the herdr
**server's** environment:

```
HERDR_DRIP_ACCOUNTS_FILE      # default: $XDG_STATE_HOME/herdr-drip/sidebar-accounts.txt
HERDR_DRIP_ACCOUNTS_WIDTH     # default: 21 (fits herdr's 26-column sidebar)
HERDR_DRIP_ACCOUNTS_INTERVAL  # default: 5 (seconds between redraws, NOT between polls)
```

`gumbo` must be on the herdr server's PATH — it is not a dependency of this
repo, and a host without it simply gets no rail (the plugin says so once in its
log and exits, and that is the one failure it keeps quiet: with no gumbo
installed there was never a rail to be missing). With gumbo present, the
watcher retries for as long as the server lives — 30s doubling to 5 minutes,
never giving up — because in a kart the first attempts race the unit that
materialises gumbo's endpoint, and a permanent surrender there left the rail
dead until somebody restarted herdr. `herdr plugin action invoke sync --plugin
drip.gumbo-usage` redraws now, after a `gumbo login` or a `gumbo use`.

## beads: the board you are on, in the sidebar — and the board itself

> **Both halves of this plugin come from
> [herdr-beads](https://github.com/miiraheart/herdr-beads) (MIT).** That project
> put a full [`bd`](https://github.com/steveyegge/beads) board — list, table and
> kanban views, editing, dependency graphs — in a herdr pane, and the idea, the
> status vocabulary and the glyphs below are all its. The rail described here is
> a re-implementation for a surface it has no equivalent of: not a pane you open,
> but one line of sidebar that is always there, and a click to unfold the rest.
> The board is that project's own code, forked into `beads/board` — see
> [the board itself](#the-board-itself) below, and `beads/board/UPSTREAM.md`
> for the rev it was taken at.

Above the accounts rail, a summary line you can click:

```
──────────────────────
▾ beads             ◐3
◐ dr-09 injector retry
◐ dr-14 turnpike egress
◐ dr-02 kart lifecycle
… show more        +39
```

Click the summary and it folds back to the one line. The counts are
blocked/in-progress/open, in that order, and they are dropped from the **right**
as the sidebar narrows — so the blocked count is the last thing to go, being the
one worth interrupting yourself for.

**What is in progress, in this space.** Two words that are both defaults rather
than rules, and both of them narrow what the rail used to show:

- `statuses` is `in_progress` unless a host says otherwise. Five rows can
  answer one question well, and "what am I in the middle of" is the one worth
  the space; five of forty-two open beads, chosen by a sort, is a list nobody
  decided to look at. `HERDR_DRIP_BEADS_STATUSES=all` is the old whole board,
  and any comma-separated vocabulary is a rail of that instead.
- the board is the **focused workspace's**. The rail is drawn beside the list
  of spaces, so a board from a space you are not in is a rail about somebody
  else's repo sitting under your agents. Move to a space with no board and the
  rail goes away rather than falling back to one it can still find, which is
  what it used to do.

When more than one status reaches the rail it is ordered **worst first**:
blocked, then in progress, then open, each group by bd priority. That is
deliberately *not* herdr-beads' order, which opens with `open` and reads better
in a pane the size of a board. Here the list is truncated to whatever rows the
sidebar has left after the agent panel, and a list that drops its tail has to
keep the blocked rows at the top or the truncation lies.

**The last row opens the board** (`… show more`, with the number of beads that
did not fit pushed to the right edge). It is drawn as the final item of the
list rather than as a button under it, and it invokes the same `open-tab`
action `Ctrl+b O` does — so a second click on it puts the tab away again.

On a board with nothing in progress the rail says so rather than going away:

```
──────────────────────
▾ beads
• nothing in progress
… show more
```

That sentence is a row in the file like any other (`--`, the unknown status),
written by the plugin rather than known to herdr, so which words appear follow
the vocabulary the rail is filtered to. A rail that vanished instead would take
the way to the board with it, every time somebody closed their last bead.

The glyphs are herdr-beads' `status_glyph`, kept identical so the two read as
the same tool:

| Glyph | Status | | Glyph | Status |
| --- | --- | --- | --- | --- |
| `○` | open | | `❄` | deferred |
| `◐` | in progress | | `◆` | pinned |
| `●` | blocked | | `◇` | hooked |
| `✓` | closed | | `•` | anything else |

`✓` is in the reader's vocabulary but the plugin never sends it: closed beads
are filtered out before the file is written, because the rail is what is left
to do. It is there so anything else writing this format can use it.

One substitution: herdr-beads draws pinned as 📌, which is an emoji and two
terminal cells wide. In a 26-column sidebar every rail row is one glyph, one
space and the text, so a two-cell glyph shifts that row's text against its
neighbours. `◆` is the one-cell stand-in.

Collapse the sidebar and the rail collapses with it, to a single glyph in the
worst status on the board — there is the board, and this is its temperature.

Three pieces, the same shape gumbo-usage has:

- **[`bd`](https://github.com/steveyegge/beads)** owns the board. Nothing in
  the rail writes to it — the board pane below does, with the same `bd`.
- **`drip.beads`** (this plugin) asks herdr which workspace is focused and
  which of its panes is on a board, runs `bd list --json` in that pane's
  directory, and writes one line per bead to a file. Every call is an argv
  vector rather than a shell string, which is herdr-beads' discipline and worth
  keeping: bead titles are arbitrary text.
- **the `sidebar-beads` hardcore patch** reads that one file and draws it.

The rail **follows focus**, which the board pane resolves once at startup and
then keeps: move to another space and the rail is that space's board, while the
board you opened is still the board you opened. Within the space, focus decides
only among panes that are *on* a board — a pane that has cd'd to `/tmp` does
not blank a rail its neighbour can still fill, and `.beads` is looked for the
way `git` looks for `.git`, walking up. Across spaces it does not decide at
all: no board in this one is an empty rail, not a search of the others. That is
what the 15s poll is buying, and why it is not 1s — each tick spawns bun.

**Five rows, and the number they are five of.** The open rail asks for a row
per line it is given, so an uncapped board would push the agent panel down to
its floor; it draws five by default. But the counts on the summary line come
from a **totals line** the writer puts first — `#<blocked> <in progress>
<open>` — rather than from the rows, because counting five rows would report
the size of its own truncation. `#` is a character no bead line can begin with,
so both halves degrade cleanly on their own: a herdr without this patch drops
that line as malformed and counts rows the way it always did, and this patch
reading an older writer's file finds no totals and does the same.

The patch knows nothing about bd. It reads `<status><priority> <text>` lines
and that one totals line, and paints them; anything writing that format feeds
the rail, and when nothing does, every function in it returns empty and the
sidebar is byte-identical to stock herdr.

Overrides, in the herdr **server's** environment (or from nix — see
`beadsSettings` below):

```
HERDR_DRIP_BEADS_FILE         # default: $XDG_STATE_HOME/herdr-drip/sidebar-beads.txt
HERDR_DRIP_BEADS_OPEN_FILE    # default: alongside it, sidebar-beads.open
HERDR_DRIP_BEADS_INTERVAL     # default: 15 (seconds between passes)
HERDR_DRIP_BEADS_ROWS         # default: 5 rows on the rail
HERDR_DRIP_BEADS_LIMIT        # default: 40 beads written (the outer ceiling; the smaller wins)
HERDR_DRIP_BEADS_STATUSES     # default: in_progress. Comma-separated, or `all` for the whole board
HERDR_DRIP_BEADS_SHOW_CLOSED  # 1 to include closed beads (costs a second bd call)
HERDR_DRIP_BEADS_CWD          # pin the board to one directory, instead of following the focused space
HERDR_DRIP_BEADS_CONFIG       # default: $HERDR_PLUGIN_CONFIG_DIR/config.json
HERDR_DRIP_BD_BIN             # default: bd
```

The last two are where the rail stops being alone. `config.json` in the
directory herdr gives every plugin is **one file the rail and the board both
read** — `statuses` (an order to the board, a filter to the rail),
`show_closed`, and `rail_rows` — so the two surfaces cannot disagree about what
a board is. One key, two defaults, because they are two surfaces: unset, the
board opens on everything in its own order and the rail shows what is in
progress. Set it and both obey it. Environment beats file, so the table above
is still the quick way to change one. `beads/board/src/config.rs` is the format's description.

`sidebar-beads.open` is read once, the first time the sidebar draws, and written
on every click. So editing it from outside sets what the **next** server starts
with, not what this one is showing — clicking is the only way to fold a running
rail. That is also why there is no keybinding for it, where `drip.agent-scope`
has one: its state file is re-read by a plugin action on every invocation, and
this one is not.

`bd` need not be installed: a host without it gets no rail and nothing to turn
off, the same way a host without gumbo gets no accounts rail.
`herdr plugin action invoke sync --plugin drip.beads` refreshes now.

On a NixOS host the module supplies it, because "not installed" turned out to
be the common case rather than the edge one: `bd` is typically a per-repo tool
that only exists inside a devshell, so the herdr **server** — which is what
runs this plugin — never sees it, and the rail stays empty on a box whose
repos all have boards. `services.herdr-drip.plugins.beadsPackage` is spliced
onto the PATH of this plugin's commands only (the treatment python3 gets in
`nix/claude-agent-state.nix`, and for the reason `environment.systemPackages`
gives for refusing it: one consumer, so one PATH). It defaults to the beads
this flake pins.

Override it on a host that already has a `bd` — a drift-rust circuit should
pass `inputs.drift-rust.packages.${pkgs.system}.bd`. Two beads of different
versions on one box is a real hazard, not tidiness: the first write by the
newer one forward-migrates the shared on-disk Dolt schema and the older one
then refuses to read it at all, with no downgrade. Which is also why the pin
here moves in step with drift-rust's, never on its own. **One bd for both
surfaces**, for that same reason: the rail only ever read, and the board
writes.

`services.herdr-drip.plugins.beadsSettings` sets the table above declaratively
— an attrset of those variables, wrapped onto this plugin's commands and its
board binary and nothing else, the same scoping `beadsPackage` gets. They are
set as *defaults*, so a variable exported before launching herdr still wins:
a host's config says what the box looks like, and that export is somebody
changing a setting for an afternoon.

### The board itself

Ctrl+b **B** docks it on the left of the current tab; Ctrl+b **O** opens it as
a whole tab. Both toggle — press again and the pane you opened is closed. The
dock lands at about a quarter of the tab in List view; the tab opens in Kanban
with the detail pane up.

```
 List   Table   Kanban    scope:repo
▾ ○ Open (42)
  P1 B dr-2rge       dev.sh all runs no ni…
  P1 · dr-0127       The phone terminal wi…
▾ ◐ In Progress (2)
  P1 · dr-uo4x       mkfs.erofs fragment d…
```

`?` lists the keys. It reads *and writes*: `c` claims, `x` closes with a
reason, `p` re-prioritises, `a` and `e` create and edit, `s` sets a status.
`q` quits — and closes its pane, which is this fork's change and not a small
one: herdr treats a pane's process exiting and a pane closing as two different
events, so upstream's `q` left a dead pane holding its slot.

Three entrypoints over one binary (`beads/herdr-plugin.toml`): `dock`, `tab`,
and a floating `popup` that is declared but bound to no key, because a popup
has no pane id in `pane list` and nothing could find it again to close it.
Placement in the manifest is only a default — the launcher's `--placement`
decides — so what an entrypoint really fixes is its `--mode`, the board's own
idea of how much room it has. `--mode` understands `dock` and nothing else,
which is why the *tab* entrypoint asks for `popup` and is not a typo.

**Which repo's board** it opens is the question the rail answers every 15
seconds, answered once at startup instead: the launcher passes the focused
pane's directory as `HERDR_DRIP_BEADS_CWD`, and the binary re-derives it from
`herdr pane list` anyway — walking up for `.beads` the way `bd` does, and
resolving to *nothing* rather than to the best of a bad lot when no pane is on
a board. That last part matters in a tab, where the board is itself the
focused pane and has nobody else's focus to read.

`nix/beads-board.nix` builds it (`nix build .#beads-board`, then
`--selftest` for a headless dump), and `nix/drip-plugins.nix` splices the
binary into the plugin directory at `board/target/release/herdr-beads` —
cargo's own output path, so a store build and a working tree put it in the same
place. There is no `[[build]]` in the manifest on purpose: `herdr plugin link`
never runs one, and a store path has no toolchain to run it with. **On a linked
working tree, `cargo build --release` in `beads/board/` by hand**, once.

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
- **sidebar-beads** — the beads rail above, between the agent list and the
  accounts rail, with a summary line that opens and closes it. Same reason as
  `sidebar-accounts`: there is no plugin surface that reaches the sidebar. It
  lands ABOVE the accounts rail by *sequence* rather than arithmetic — each
  carve takes rows off the bottom of what it is handed, so running the beads
  carve after the accounts carve puts it between the agents and the accounts.
  The open/closed flag is a file rather than herdr session state because the
  carve happens in `expanded_sidebar_sections`, which is handed no `AppState`
  to read it from.
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
- **sidebar-quiet-chrome** — the sidebar's section labels go: `new` and `menu`
  under the workspace list, `agents` over the agent panel, and the
  `grouped`/`priority` tag in its corner. Each names something the rows below
  it already show, and a 26-column sidebar has no columns to spend on
  captions. Every one is a string literal in the renderer, so no plugin can
  reach them. See below.
- **sidebar-auto-split** — the divider between the workspace list and the
  agent panel follows the list instead of a dragged ratio, so opening and
  closing spaces resizes both sections on its own. See below.
- **sidebar-scope-icon** — a clickable icon in the agent panel's header
  showing which scope the agent list is in, and flipping it when clicked. The
  visible half of **drip.agent-scope**; it moves into the cell
  `sidebar-quiet-chrome` emptied. See below.
- **agent-scope-family** — "this space" means this space *and the rest of its
  repo*: a repo row shows its worktrees' agents and a worktree shows its
  siblings'. herdr models worktrees as sibling workspaces, and the agent view's
  filter language has no repo field to say so with. See below.
- **rename-presets** — the rename dialog (and the one `New Tab` and `New Space`
  open) offers twelve named kinds of work under its input in a grid four wide,
  each behind a glyph, each applied by one click. A plugin's `[[actions]]` reach
  the palette and the keybindings; a `Mode`'s modal is compiled in and has no
  list to contribute a row to. See below.

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

### sidebar-quiet-chrome: the sidebar stops narrating itself

Five words the sidebar repeated at you every frame, each naming something the
rows under it already show:

| Label | Where | What it named |
| --- | --- | --- |
| `new` | footer, left | the `+` you already clicked |
| `menu` | footer, right | the launcher you already clicked |
| `agents` | over the agent panel | a list of agents |
| `grouped` / `priority` | agent panel corner | the sort, which the order shows |
| `accounts` | over the accounts rail | rows of percents and reset clocks |

A 26-column sidebar has no columns to spend on captions, and none of these is
reachable from a plugin — they are `&'static str` literals in
`src/ui/sidebar.rs` (the last one in our own `sidebar-accounts.rs`, where it
cost the rail two rows: the title and the blank line under it).

**Only what is drawn changes; every rect stays.** The click targets are
computed by separate geometry functions — `sidebar_new_button_rect`,
`global_launcher_rect`, `agent_panel_toggle_rect` — and those are untouched,
so the footer zones still work and the corner still cycles the sort when
clicked. That asymmetry is also what keeps herdr's own tests green:
`clicking_agent_panel_toggle_switches_sort` clicks the rect the geometry
function returns, not the pixels. The menu's attention dot survives its label:
`●` is a signal, not chrome.

The agent-view label goes too, and has to — **drip.agent-scope** below keeps a
view active permanently, so the corner would otherwise read `filtered` on
every frame forever, in the cell the sort label just vacated. The blanking is
of the *fallback*, so a view that sets a real label can still say so.

### sidebar-auto-split: the divider follows the list

Stock herdr sizes the two sidebar sections from `sidebar_section_split`, a
ratio you drag and the session persists. Nothing updates it, so every space
opened or closed leaves it where it was: the workspace list scrolls under a
half-empty agent panel, or crowds it out, until someone drags the divider
back. The number it should hold is not a preference — it is however many rows
the list needs, and the list already knows.

`drip_auto_section_split` (`nix/sidebar-auto-split.rs`) computes exactly that:
each entry's `workspace_row_height` plus its `workspace_entry_gap`, under the
two header rows and above the one footer row, mirroring the arithmetic in
`compute_workspace_list_areas`. One write in `compute_view_internal` plants it
in `app.sidebar_section_split` **before any geometry is derived**, so the
renderer, the click hit-testing and the scroll metrics all read the same value
they always did and stay consistent for free.

No plugin can do this: the ratio is an `AppState` field, and the sidebar has
no plugin surface at all.

**Dragging is retired by consequence, not by surgery.** The divider's hit test
and its setter are left exactly as they are — herdr's
`dragging_sidebar_section_divider_sets_split_ratio` drives them directly and
still passes — but the next view pass recomputes the field, so a drag never
survives to a drawn frame. That is the cheapest honest way to disable the
gesture; unpicking the mouse path would mean rewriting code herdr tests.

Two clamps matter and both stay: `sidebar_section_heights` keeps each section
at least 3 rows, so a list longer than the sidebar simply scrolls (exactly as
a stock sidebar too short for its list does), and below 6 rows it ignores the
ratio entirely, so the computation returns the current value untouched rather
than churning a field nobody reads. The ratio guard itself widens from
`(0.1, 0.9)` to `(0.0, 1.0)` — a three-space list on a tall sidebar should not
be forced to hold a tenth of it.

### sidebar-scope-icon: the toggle you can see

One cell in the agent panel's header, showing which scope the list is in and
flipping it when clicked:

| Glyph | Codepoint | Name | Means |
| --- | --- | --- | --- |
|  | `U+EB7F` | `cod-window` | this space's agents |
|  | `U+EB23` | `cod-multiple_windows` | every space's |

Both are Codicons, the block the pane menu's four glyphs already come from —
and this drip already spends `cod-window` on `New Space` and
`cod-multiple_windows` on `New Tab`, so a window has meant a space here since
that patch landed. One window is this space, more than one is every space:
existing vocabulary, no new glyph to learn. Deliberately **not** the Material
Design plane `sidebar-version`'s drop lives in, whose codepoints moved
wholesale in Nerd Fonts v3 and render as tofu on an older patched font — a
one-column cell has nothing to degrade into. The cell also keeps herdr's own
colour rule for it: accent while a view is active, neutral otherwise. That
used to make scope read twice over; since **reply-age** the drip keeps a view
active in *both* scopes (the sort has nowhere else to live), so the colour is
now constant and the glyph carries the whole message on its own.

**It lands in the cell `sidebar-quiet-chrome` emptied, and that cell was
already dead.** `on_agent_panel_sort_toggle` — herdr's hit test for the sort
label — returns false whenever an agent view is active, and drip.agent-scope
keeps one active by default. So on a default drip host that corner was a
control that could not be clicked; this gives it something to do.

**herdr's click routing is real and this uses it rather than inventing any.**
Every clickable thing in the sidebar is the same three pieces: a `*_rect`
geometry function in `ui/sidebar.rs`, an `on_*` hit test in
`app/input/sidebar.rs`, and an arm in `handle_mouse` that asks them in order.
All three are patched, none is bypassed:

- the **rect** (`agent_panel_toggle_rect`) is sized to the glyph instead of to
  `grouped`/`priority`, so the region you can click is the glyph you can see
  rather than the seven blank columns left of it;
- the **hit test** loses the `|| self.agent_view_override.is_some()` guard —
  that guard is there because a filtered panel has no sort to cycle, and the
  cell now toggles the filter itself, which is exactly what stays meaningful
  while a view is active;
- the **arm** calls `drip_toggle_agent_scope`, which sets or clears the same
  view, on the same source, that the plugin sets over the socket.
  `handle_agent_view_set` is validation plus a field and two scroll resets, and
  those three are all reachable from `AppState` — which matters, because
  `handle_mouse` is `impl AppState` and has no `App` to dispatch an API call
  with.

**Known gap: clicking the cell drops the reply-age sort.** `drip_toggle_agent_scope`
builds its view with `sort: Vec::new()` and its `all` arm sets
`agent_view_override = None`, both of which predate the sort existing — so the
click reaches the right SCOPE and the wrong ORDER, in either direction. The
plugin's own paths are unaffected (`Ctrl+b a`, the three actions, and the
`[[startup]]` re-apply after every session restore all go through
`bin/agent-view.js`), so the list comes back sorted the next time any of those
runs. Closing it properly is two lines in this file — carry `SORT` in the
`current` arm and set the sort-only view under `plugin:drip.reply-age` in the
`all` arm, mirroring `bin/agent-view.js` — and one rebuild of herdr, which is
why it is written down here rather than done in passing.

The state and glyphs live in `nix/agent-scope-icon.rs`, appended to
`app/state.rs` rather than to the sidebar: the renderer and `app::input` both
have to name them, `app::input` is private, and `app::state` is the one module
either side can reach — the same constraint `pane-menu-labels.rs` works under.

**The cost, stated plainly:** like the pane menu's `Close pane` before it, this
breaks one of herdr's own tests. `clicking_agent_panel_toggle_switches_sort`
clicks this cell and asserts the sort changed, which it no longer does. The nix
build does not run herdr's tests (`doCheck = false`) and neither does
`check-herdr-patches.sh`, so nothing here catches it — a `cargo test` on a
patched checkout would fail exactly there. The sort itself is not lost:
`ui.agent_panel_sort` still sets it, and its label was already invisible. The
mouse just no longer cycles it.

### agent-scope-family: a repo and its worktrees are one space

Standing on `beads-ui` and seeing none of the agents in its worktrees is the
bug this fixes. herdr models a worktree as a **sibling workspace**, not a child
of the repo it came from:

```
w6  beads-ui       repo_key=/home/dev/Code/demos/beads-ui/.git
w7  graph-view     repo_key=/home/dev/Code/demos/beads-ui/.git   (linked)
w8  epic-tree      repo_key=/home/dev/Code/demos/beads-ui/.git   (linked)
w9  health-panel   repo_key=/home/dev/Code/demos/beads-ui/.git   (linked)
```

Four workspaces sharing nothing but a `WorktreeSpaceMembership`, so a view
filtered on workspace id can never reach w7–w9 from w6 — and the repo row is
exactly where you stand to ask what the whole repo is doing. "This space" now
means this space **plus every workspace in the same repo family**, in both
directions: from the repo you see the worktrees, from a worktree you see the
repo and its siblings.

**Matched on `repo_key`, not `repo_root`**, for two reasons. It is what herdr
itself groups on — `space.key` backs the sidebar's own worktree grouping, its
collapse state (`collapsed_space_keys`) and its aggregate status, and the API
field is literally `repo_key: space.key.clone()` — so the agent list now groups
by the same rule as the workspace list right above it, and those two
disagreeing would be worse than either rule being wrong. It is also the more
precise key: herdr fills it from the repo's `.git` path, one identity for the
main checkout and every linked worktree, where `repo_root` is a plain directory
path a second clone elsewhere could coincide with.

**A workspace with no worktree record is untouched.** Membership is what the
widening keys on, and herdr only attaches one to a workspace that is part of a
worktree family — plain git checkouts like `herdr-drip` and `demos` carry none
at all. No membership, no widening, plain id match, exactly as before.

#### Why this is a patch and not the plugin

**The filter language cannot express it.** Its fields are `status`,
`workspace_id`, `tab_id`, `pane_id`, `agent`, `seen`, `state_change_seq` plus
free `token` lookups; its only *dynamic* values are the context vars
`current_workspace_id` and `current_tab_id`. There is no repo field and no repo
context, and `validate_field_value` whitelists context values to exactly those
two (field, context) pairs — so even a token carrying the repo key could only
ever be compared against a **static** string.

The documented escape is to resolve the sibling ids in the plugin and send
`in: [w6, w7, w8, w9]`, refreshing on `workspace.focused`. That hook does
exist, and the route was still rejected, for a reason worth writing down: **a
static id list cannot track what herdr actually filters against.**
`presented_workspace_idx` follows the sidebar *selection* while the sidebar is
being navigated (`Mode::Navigate` reads `app.selected`, not `app.active`), and
`WorkspaceFocused` is emitted from the pane-focus path — it never fires for
selection movement. The list would be stale precisely while you are arrowing
through spaces reading the agent panel. It would also spawn a process and a
socket round trip on every space switch, and show one frame of the previous
repo's agents each time.

Evaluated in place, none of that exists: the question is asked per render,
against the same presented workspace the stock context var uses.

The patch is **one `||` in front of the stock comparison**, so it can only
widen, never narrow — exact id equality still answers first for everything, and
the family test adds the rest only for our own view (by source), only for the
one filter shape our view sends, and only when both workspaces carry a
membership. Another plugin's `workspace_id == current_workspace_id` stays the
documented exact match; widening someone else's filter would be changing an API
meaning out from under them.

The plugin keeps sending that stock filter **unchanged**. This widens what the
filter means on a patched herdr rather than inventing a shape only a patched
herdr could parse — so drip.agent-scope on a stock herdr still works and simply
stays exact, which is what it did until now.

### rename-presets: twelve kinds of work, one click each

```
┌──────────────────────────────────────────────────────┐
│rename pane                                           │
│                                                      │
│ my-pane                                              │
│─ presets ────────────────────────────────────────────│
│   orch        impl        recon       spike      │
│   watch       review      debug       triage     │
│   docs        deploy      bench       misc       │
│                                                      │
│           ↵ save    ^c clear    esc cancel           │
└──────────────────────────────────────────────────────┘
```

The same dialog, plus a grid. The input is untouched: type a name and press
Enter exactly as before. **Clicking a preset cell is a select and a confirm in
one gesture** — it fills the field and saves, so the modal closes and the name
is applied without a second keystroke. That is the whole feature; naming a pane
`recon` for the hundredth time should not cost a word and an Enter.

| Preset | Glyph | For |
| --- | --- | --- |
|  orch | `cod-type_hierarchy` | the coordinator pane, driving workers |
|  impl | `cod-code` | feature work in a worktree |
|  recon | `cod-search` | a read-only look at code nobody is changing yet |
|  spike | `cod-beaker` | a time-boxed throwaway experiment |
|  watch | `cod-pulse` | a log tail, a watch loop |
|  review | `cod-checklist` | reading a diff before it lands |
|  debug | `cod-bug` | chasing one failure |
|  triage | `cod-inbox` | working the board down |
|  docs | `cod-book` | the prose rather than the code |
|  deploy | `cod-rocket` | putting it on a machine |
|  bench | `cod-dashboard` | measuring what happened when you did |
|  misc | `cod-ellipsis` | one of the others |

**Six characters is the ceiling on a name, and that is geometry rather than
taste.** Four cells across a 56-column modal is thirteen columns each, and a
glyph plus a space plus six characters is eight of them. `orchestration` was
affordable when a preset owned a whole row and nothing else; beside three others
it is not, and `orch` is what the tab bar was truncating it to anyway.

**The glyph is part of the name, not decoration beside it.** The cell applies
its string verbatim, so what lands on the pane is ` recon` and the glyph is
what you then read in the sidebar and the tab bar — which is the point of naming
panes by kind at all. All twelve are Codicons from the U+EA60–U+EBEB block
(taken from upstream's `glyphnames.json` 3.5.0, not from memory), single width,
off the Material Design plane whose codepoints moved between Nerd Fonts v2 and
v3. A font missing them draws twelve boxes and twelve cells that still read,
because the word is right there.

**It is one modal doing five jobs**, so the presets appear wherever herdr asks
for a name: rename pane, rename tab, rename workspace, and the name prompts
`New Tab` and `New Space` open before creating one. Saving means whatever it
already meant in that mode — create the tab with this label, rename the pane —
because the click returns herdr's own `Save` action rather than a second path
that would have to re-learn all five.

**The list is baked into the patch.** A configurable one is a config surface, a
parser and a reload path for twelve strings that change about as often as the
menu's wording does; when they do change, they change here, in one commit, for
every host at once — the same bargain **pane-menu** makes.

The layout notes, since a modal that lies about where its own cells are is the
failure mode worth being careful about:

- The popup is 11 rows where stock is 7, and it is **derived** from the grid:
  twelve presets four across is three rows, so the height is the row count plus
  the chrome around it. A thirteenth preset takes the free cell on the last row
  and changes no number anywhere; a fourteenth grows the modal by one row on its
  own. Stock spelled that height as `56, 7` in **two** places — the renderer
  that draws it and `rename_modal_inner` that hit-tests it — and both now read
  one constant, so a later edit cannot move the drawn dialog without moving the
  clickable one.
- `save`/`clear`/`cancel` moved to the bottom, where every other herdr dialog
  puts them. Not by rewriting their placement — `centered_button_row` puts them
  three rows down from whatever rect it is handed, so both callers hand it the
  bottom four rows of the modal and stock's arithmetic is left alone.
- The cells the mouse can hit are the cells the renderer draws, from the same
  function. That mattered when a preset was a full-width row and it matters more
  in a grid, where a one-column disagreement is a click on the neighbouring
  word rather than a near miss.
- **The whole cell is the target**, not the eight columns the word occupies: the
  five columns after `orch` are nearer `orch` than anything else and there is
  nothing else in them. The two columns of lead at the modal's left edge are in
  no cell — they are the gutter every other row of the dialog keeps clear.
- On a terminal under 13 rows, or under 40 columns, the popup is clamped smaller
  than it asked for and four cells no longer fit. The same function answers "no
  cells" to the renderer and to the hit test, so the dialog degrades to stock's
  title/input/buttons rather than growing invisible live regions. Between 40
  columns and 56 the cells shrink evenly instead of overlapping.
- Clicking anywhere else in the modal still cancels it, which is what stock
  herdr does with every click that is not a button. The presets are asked
  *before* that fallthrough; asked after it, each cell would have been a cell
  that closes the dialog.

There is no keyboard path to the cells on purpose: the keyboard already has one,
and it is the input field. This is for the hand that is already on the mouse
because it just right-clicked the pane.

## agent-scope: the agent list is about this space

The sidebar's agent panel lists every agent in every space, which on a host
with a dozen spaces is a list you scroll rather than read. This plugin makes
it show the space you are looking at — and, on a patched herdr, the rest of
that space's repo family with it (**agent-scope-family**, above) — and widens
it back to everything on one click.

**The icon in the agent panel's header is the toggle** — one window () for
this space, two () for every space, so the control and the state indicator
are the same cell. That half is a hardcore patch (`sidebar-scope-icon`,
above); this plugin is the half that holds the scope and re-applies it. The
other ways in, none of them the one to reach for first:

```
Ctrl+b a                                             # secondary, and free
herdr plugin action invoke toggle --plugin drip.agent-scope
herdr plugin action invoke current --plugin drip.agent-scope
herdr plugin action invoke all --plugin drip.agent-scope
```

**This one is a real plugin, not a patch** — herdr has an API for exactly
this. `agent.view.set` takes a filter, and

```json
{"op": "eq", "field": "workspace_id", "value": {"context": "current_workspace_id"}}
```

is resolved by herdr *on every render*, against the workspace being presented
(the active one, or the selected one while navigating). So the plugin is
invoked once and the panel follows your focus from then on. `all` does not set
a filter that matches everything either — there is nothing left to filter on.

**But `all` no longer clears the view, and that is the one thing about this
plugin that changed when reply-age arrived.** herdr holds exactly one agent
view, so the ORDER of the list has to ride in the same object as the filter or
it cannot exist at all — and newest-reply-first is the drip's default now, not
a property of one scope. So `all` sets a view carrying only the sort:

```json
{"source": "plugin:drip.reply-age", "sort": [{"field": {"token": "since_key"}, "order": "asc"}]}
```

**Under reply-age's source, not this plugin's**, which is load-bearing twice
over. herdr clears a plugin-owned view when that plugin goes away, so a sort
over tokens nobody reports any more leaves with the reporter. And
`drip_scope_is_current()` — the thing **sidebar-scope-icon** asks to pick its
glyph — is `agent_view_override.source == "plugin:drip.agent-scope"`, so
reusing this plugin's source here would have left the header drawing the
one-window glyph over a list of every space: the icon lying about the only
thing it is there to say. If reply-age is not installed, `all` falls back to
the clear it used to do — there would be no `since_key` to sort on anyway.

The scope is one word in `$XDG_STATE_HOME/herdr-drip/agent-scope`
(`$HERDR_DRIP_AGENT_SCOPE_FILE` overrides), so a toggle survives the next
toggle and a restart. herdr does not persist an agent view, so `[[startup]]`
re-applies it after each session restore — which is also what makes
this-space-only the **default**: a server that has never seen this plugin has
no state file, and `apply` reads that as `current`.

That file is in the drip's state root rather than the plugin's own because the
plugin is not its only writer: **sidebar-scope-icon** writes it too, on every
click. One file is the whole contract between the two halves — the same
arrangement the accounts rail has with gumbo-usage, in the same state root,
and for the same reason: neither side needs to know the other exists.

Two details worth knowing:

- **It speaks the socket, not the CLI.** There is no `herdr agent view` verb —
  the agent view is API-only — so `bin/agent-view.js` writes one JSON line to
  the socket herdr hands every plugin command in `$HERDR_SOCKET_PATH`. That is
  `bun`, already a declared drip dependency (`flake.nix#herdr-drip-deps`) and
  the only interpreter on the server's PATH that speaks unix sockets. A host
  without it gets one line on stderr and an unchanged scope, the way
  gumbo-usage degrades without gumbo. There is no `[[build]]` — no
  node_modules, just the runtime.
- **The view is owned, and herdr knows it.** The source is
  `plugin:drip.agent-scope`, which herdr validates against the installed
  plugin list and clears on its own if the plugin is disabled or removed —
  so a disabled plugin cannot leave your agent panel filtered with nothing
  left to unfilter it. `all` clears only that source, never a view another
  plugin set.

## reply-age: how long since each agent said anything

Twelve agents in the sidebar and no way to tell which one just finished. This
plugin puts the answer on every row and sorts the list by it, newest reply at
the top:

```
◐ 12s  impl        ✳ 4m   dr-0157
  Reading src/app/agent_view.rs      Convert beads sidebar into workspace panel
```

**"Last reply" means the moment an agent last STOPPED working.** herdr's
`agent_status` reads `working` while a turn is in flight and lands on
`idle`/`done`/`blocked` when it is over, and the instant it lands is the
instant the reply is on your screen. So the number counts up from that
transition — not from when the turn started, and not from your last keystroke.
While an agent is working it keeps climbing from the agent's *previous* reply,
which is the point: an agent that has been grinding for forty minutes has not
said anything for forty minutes, and that is exactly what you want to notice.

**herdr has no clock on this, so the stamps are ours.** `AgentInfo` carries
`agent_status` and a monotonic `state_change_seq` and no timestamp anywhere.
Each tick reads `herdr agent list`, compares it against the last tick, and
stamps `now` on any agent that has ARRIVED at a stopped state — `seq` moved
and the status is one of the three. Reading the seq rather than only comparing
statuses is what catches a hop through `unknown`, which is detection losing
the thread rather than a turn ending. State lives in
`$HERDR_PLUGIN_STATE_DIR/stamps.json` so restarting the watcher does not reset
every row to "just now", and it is keyed by **terminal id, not pane id**: a
pane id is a slot herdr reuses, and keying on the slot would hand a brand new
agent the dead one's reply time.

A stamp is `now` at **tick** time rather than at transition time, so an age
reads up to one interval younger than it truly is. Nothing can recover the
difference — the transition left no timestamp behind either — and it stops
mattering as soon as the row reads in minutes, which is why the interval is
the only dial on it.

An agent seen for the first time is stamped `now`, and that is a guess — the
honest kind. There is nothing to ask, and on a cold start *every* pane gets
the same stamp, so they tie and the list keeps herdr's own order rather than
one row lying its way to the top. It corrects itself at that pane's next
reply.

**Three tokens, because one number has two jobs:**

| Token | Is | For |
| --- | --- | --- |
| `$since` | `now`, `12s`, `3m`, `1h20m`, `2d` | reading |
| `$since_key` | the same elapsed seconds, zero-padded to nine digits | sorting |
| `$space` | the workspace name — only while the list shows every space | telling two repos apart |

`since_key` is padded because **herdr compares token sort values as strings**
(`sort_value` returns `EvalValue::String`), so `"3"` would sort after `"10"`.
Fixed width makes lexicographic order and numeric order the same thing.
Ascending on it is smallest-elapsed-first, which is newest-reply-at-the-top.

Agents with no token — a pane the watcher has not reached, a non-agent kind,
or every row at once if the watcher died and the 30s TTLs ran out — land at
the **bottom**, and nothing here arranges that: herdr orders a missing sort
value last in *both* directions (`compare_optional_values` reverses for `desc`
only inside the both-present arm). Nothing should arrange it either — a
sentinel key meaning "unknown" would be indistinguishable from an agent that
genuinely replied that long ago.

### `$space`, and why this plugin reports it

The agent row used to carry `workspace`, and it was dead weight:
drip.agent-scope narrows the list to the space you are looking at by default,
so the column was one value repeated down every row. Replacing it with the age
is most of the point of this plugin.

That leaves one real tension. In **every space** the name stops being
redundant — it is then the only thing telling two repos' agents apart, and
`$worktree` does not cover it, because worktree-tokens reports nothing for a
main checkout on purpose. Rows are *config* (`[ui.sidebar.agents.rows_by_agent]`)
and the scope is a *view*, and nothing in herdr changes rows at runtime, so
they cannot be swapped when the scope flips.

They do not have to be. **A missing custom token is dropped from its row, and
a row left with nothing in it is dropped whole** — so a token reported in one
scope and cleared in the other *is* a column that comes and goes. `$space` is
that: the workspace label, reported only while drip.agent-scope's state file
says `all`. It is cleared explicitly rather than merely omitted, because pane
metadata tokens are patched rather than replaced and an omitted one would sit
there under its old TTL for another half minute.

It is reported by **this** plugin rather than by agent-scope, which owns the
scope, for one reason: this is the only watcher in the drip already paying for
a per-agent `herdr agent list` tick, and a second plugin polling the same list
every ten seconds to fill one column would be a poor trade. agent-scope's
`bin/scope` nudges `bin/sync` after a change so the column appears and
disappears with the toggle instead of on the tick; a host without this plugin
ignores that line. The scope file is already a contract between two writers
(the plugin and the sidebar-scope-icon patch); this is a third party that only
ever reads it.

### Knobs

```
HERDR_DRIP_REPLY_AGE_INTERVAL   watcher tick, seconds (default 10)
HERDR_DRIP_REPLY_AGE_TTL_MS     token lifetime (default 30000, three ticks)
```

Ten seconds because this is a clock on screen rather than a fact that changes
when something else does: under a minute the row reads in seconds, so a slower
tick shows you `12s` for half a minute, and a faster one spends two socket
calls on a number that did not move. The TTL is three ticks so a watcher that
dies lets the times **fade** rather than freezing a stale `2m` on screen
forever — the row drops to the bottom of the sort, which reads as "we don't
know" instead of as a lie.

`bin/sync` is one pass and the thing to invoke after a change rather than
waiting out the interval:

```
herdr plugin action invoke sync --plugin drip.reply-age
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
