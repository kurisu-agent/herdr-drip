# plugins — the whole drip, provisioned declaratively, without a network.
#
# By hand, a host is set up per user with `herdr plugin install
# kurisu-agent/herdr-drip/<dir>` for each plugin, `scripts/apply-config.sh`
# for the config, and a `nix profile add .#herdr-drip-deps` for the
# runtime deps. This module is that procedure as NixOS config — except that
# the install is not a fetch:
#
#   - every plugin in `plugins` is a STORE PATH (nix/drip-plugins.nix),
#     published at /etc/herdr-drip/plugins/<name> and registered with
#     `herdr plugin link`. The flake input decides which plugin code runs, so
#     `nix flake update herdr-drip` still moves it, but activation reaches
#     GitHub exactly never;
#   - keeps ~/.config/herdr/config.toml a symlink to a GENERATED config:
#     the curated config/herdr.toml layered under the host's `settings`
#     overrides, key by key — the curated values are defaults, not mandates;
#   - puts the runtime deps on the system PATH: yolo-shell (the config's
#     PATH-resolved default_shell) and bun (worktree-graph's pane command).
#     Commands resolve against the herdr SERVER's PATH, and
#     /run/current-system/sw/bin is on it even for a server already running
#     at switch time, so this lands without a restart.
#
# Why link and not install. `herdr plugin install` fetches a tarball, runs
# the manifest's [[build]], and needs both the network and a working server.
# Every one of those is a way for a rebuild to produce a different result on
# a different day, and the first two are things nix does better: the plugin
# source is already in the store as part of this flake, and the one [[build]]
# in the drip (worktree-graph's `bun install`) is nix/worktree-graph-deps.nix.
# What is left is `herdr plugin link`, which is pure registry bookkeeping —
# no fetch, no build, and it works with the server DOWN, because the CLI
# falls back to writing plugins.json itself (offline_plugin_link_response).
# So this module now provisions a host that has never run herdr and has no
# route to github, and a dirty checkout (which has no rev to pin to) works
# exactly like a clean one.
#
# Why /etc as well as the store path. Nothing else in the system closure
# refers to these store paths, so without an /etc entry the nix GC is free to
# collect a plugin that a running herdr still points at — a broken pane, not a
# warning. The indirection does NOT make the registry stable, though: herdr
# canonicalizes what it is given (manifest.rs, load_plugin_manifest calls
# .canonicalize()), so plugins.json records the resolved store path and every
# content change is a relink. That is why the check below compares against the
# RESOLVED path, and why a store path is the signature of one of our own
# links.
#
# The dev loop always wins: a `herdr plugin link`ed working tree is left alone
# silently (that is any local entry whose root is not one of ours), and a
# config.toml symlinked outside /nix/store (apply-config.sh's link into a
# checkout) is left alone with a note.
#
# Known limitations, by design:
#   - removing a name from `plugins` does not uninstall it;
#   - relinking re-ENABLES a drip plugin the user had `herdr plugin disable`d.
#
# Unlike claude-agent-state.nix this module needs nothing from
# nix-claude-drip — it only defaults `users` to claude-code's list when
# that module happens to be present.
{
  config,
  lib,
  pkgs,
  # This flake's own pinned beads, supplied by nixosModules.plugins so the
  # default works on a host that has never heard of bd. Absent (null) when
  # nix/plugins.nix is imported directly as a plain module rather than through
  # the flake, which keeps that path working exactly as it did before.
  beadsDefault ? null,
  ...
}:

let
  cfg = config.services.herdr-drip.plugins;

  dripPlugins = import ./drip-plugins.nix pkgs;

  # bd reaches the beads plugin and nothing else. Its only consumers are that
  # plugin's bin/ and its board binary, so it is scoped there rather than added
  # to systemPackages — see the comment on environment.systemPackages below,
  # and the identical call python3 gets in claude-agent-state.nix.
  #
  # ONE bd for both surfaces, deliberately. The rail only ever read; the board
  # writes (claim, close, priority), and the first write by a newer bd migrates
  # the on-disk schema of a live board that an older one then refuses — the
  # hazard `beadsPackage`'s description spells out. Two pins here would be that
  # hazard inside a single plugin.
  pluginRuntimeInputs = name: lib.optional (name == "beads" && cfg.beadsPackage != null) cfg.beadsPackage;

  # The beads plugin's settings, as environment. Both of its surfaces read
  # their knobs from the environment before the config file they share (see
  # beads/board/src/config.rs), so this is the declarative half of that file:
  # a host states what its rail and board should look like and no user has to
  # write JSON into a directory herdr manages. Scoped to the plugin's own
  # commands, like the bd above.
  pluginRuntimeEnv = name: if name == "beads" then cfg.beadsSettings else { };

  # Where the published plugin directories live. Also the marker this module
  # reads back out of the registry to tell its own links from a developer's.
  etcSubdir = "herdr-drip/plugins";
  etcRoot = "/etc/${etcSubdir}";

  # Install addressed a plugin by repo subdir (`flip-split`); the registry
  # addresses it by manifest id (`drip.flip-split`). Resolve the id from each
  # manifest at EVAL time, so a typo in `plugins` fails the build here rather
  # than failing at activation on every host, and the id stays authoritative
  # if the drip.<dir> convention ever changes.
  repoPlugins = map (name: {
    inherit name;
    id = (builtins.fromTOML (builtins.readFile (../. + "/${name}/herdr-plugin.toml"))).id;
    package = dripPlugins.mkPluginWith {
      runtimeInputs = pluginRuntimeInputs name;
      runtimeEnv = pluginRuntimeEnv name;
    } name;
  }) cfg.plugins;

  # `extraPlugins` states its ids rather than reading them, because the whole
  # point of that option is plugins this repo does not contain: reading the
  # manifest out of a derivation would make every evaluation of this module
  # build it first (import-from-derivation), which is exactly the kind of
  # thing a fleet's evaluator is usually configured to refuse.
  extraPluginList = lib.mapAttrsToList (name: plugin: {
    inherit name;
    inherit (plugin) id;
    package = plugin.path;
  }) cfg.extraPlugins;

  pluginList = repoPlugins ++ extraPluginList;

  settingsFormat = pkgs.formats.toml { };

  # The curated config as data. It enters the module system below as
  # leaf-level mkDefaults, so a host overrides one key and keeps the rest;
  # the tracked TOML stays the single source of truth for the non-nix path
  # (apply-config.sh links it verbatim).
  curatedSettings = builtins.fromTOML (builtins.readFile ../config/herdr.toml);

  # The colour scheme, generated from a palette rather than written out as
  # hex. See nix/theme.nix for the mapping and for why our palette is the
  # default rather than one option among several.
  dripTheme = import ./theme.nix;

  # The merged config as a store path. Its hash changes with its content,
  # which is exactly what the relink check keys off: a symlink to an OLD
  # store copy means a previous generation's config, and is retargeted.
  configFile = settingsFormat.generate "herdr.toml" cfg.settings;

  yoloShell = import ./yolo-shell.nix pkgs;

  resolveHerdr =
    if cfg.herdrPackage != null then
      ''
        herdr=${cfg.herdrPackage}/bin/herdr
      ''
    else
      ''
        herdr="$(command -v herdr || true)"
        if [ -z "$herdr" ]; then
          echo "herdr-drip: herdr not on PATH; plugins left as-is" >&2
          exit 0
        fi
      '';

  ensurePlugins = pkgs.writeShellScript "herdr-drip-plugins" ''
    set -eu
    # Just coreutils and jq now. The fetch that needed git, and the [[build]]
    # that needed bun, both happen at nix build time; the SERVER still gets
    # its bun from systemPackages below, for worktree-graph's pane command.
    export PATH=${
      lib.makeBinPath [
        pkgs.coreutils
        pkgs.jq
      ]
    }:/run/wrappers/bin:/run/current-system/sw/bin:$HOME/.nix-profile/bin:/etc/profiles/per-user/$(id -un)/bin''${PATH:+:$PATH}

    ${resolveHerdr}

    changed=

    ${lib.optionalString cfg.manageConfig ''
      # ---- config.toml ------------------------------------------------------
      # The generated config is authoritative, and there are two ways for
      # something else to be sitting where it goes.
      #
      # A PLAIN FILE is herdr's own: it writes this file itself — completing
      # onboarding stamps `onboarding = false` through a plain fs::write, as do
      # the theme and sound pickers — so on any host where herdr ran before
      # this module first did, config.toml already exists. Leaving it alone
      # means the curated config never lands, with nothing to show for it but a
      # line on activation's stderr.
      #
      # A SYMLINK OUTSIDE THE STORE is apply-config.sh's, pointing at a
      # checkout. That is the dev loop on a box being developed on, and stale
      # cruft everywhere else — a checkout that stops being pulled silently
      # pins herdr to whatever the drip looked like the day someone ran that
      # script. Adopting it replaces the LINK only; the file in the checkout is
      # never followed, written, or removed.
      #
      # `adoptConfig` picks how far that goes; "always" is the default because
      # a deployment should run the config its flake pin describes.
      target="$HOME/.config/herdr/config.toml"
      expected=${configFile}
      if [ -L "$target" ]; then
        dest="$(readlink "$target")"
        if [ "$dest" != "$expected" ]; then
          case "$dest" in
            /nix/store/*)
              ln -sfn "$expected" "$target"
              changed=1
              ;;
            ${lib.optionalString (cfg.adoptConfig == "always") ''
              *)
                ln -sfn "$expected" "$target"
                changed=1
                echo "herdr-drip: adopted config.toml (it pointed at $dest, which is left untouched)" >&2
                ;;
            ''}
            *)
              echo "herdr-drip: config.toml -> $dest (not store-managed); leaving it. The curated config is NOT in effect for $(id -un) on this host." >&2
              ;;
          esac
        fi
      elif [ -e "$target" ]; then
      ${
        if cfg.adoptConfig == "never" then
          ''echo "herdr-drip: config.toml is a plain file and adoptConfig = \"never\"; leaving it. The curated config is NOT in effect for $(id -un) on this host." >&2''
        else
          ''
            backup="$target.bak"
              if [ -e "$backup" ]; then
                backup="$target.bak.$(date +%Y%m%d%H%M%S)"
              fi
              mv "$target" "$backup"
              ln -s "$expected" "$target"
              changed=1
              echo "herdr-drip: adopted config.toml (herdr had written its own); previous contents kept at $backup" >&2''
      }
      else
        mkdir -p "$(dirname "$target")"
        ln -s "$expected" "$target"
        changed=1
      fi
    ''}

    # ---- plugins ------------------------------------------------------------
    # Linking is idempotent and cheap, so the only reason to read the registry
    # first is to recognise the two entries that must NOT be overwritten: a
    # developer's linked working tree, and (for the message) somebody else's
    # build of one of our ids.
    ensure_plugin() {
      local name=$1 id=$2 root=$3 entry kind current resolved owner repo managed
      entry="$(printf '%s' "$plugins_json" | jq -c --arg id "$id" \
        '[.result.plugins[] | select(.plugin_id == $id)] | first // empty')"
      # What herdr will actually record for this link, which is not the path
      # we hand it: it canonicalizes, so ${etcRoot}/<name> lands in the
      # registry as the store path behind it.
      resolved="$(readlink -f "$root" 2>/dev/null || printf '%s' "$root")"
      managed=
      if [ -n "$entry" ]; then
        current="$(printf '%s' "$entry" | jq -r '.plugin_root // ""')"
        if [ "$current" = "$resolved" ] || [ "$current" = "$root" ]; then
          return 0
        fi
        kind="$(printf '%s' "$entry" | jq -r '.source.kind // ""')"
        if [ "$kind" = local ]; then
          # A linked working tree wins silently — that is the dev loop, and
          # the link carries newer code than any pin. Our own links are local
          # too, so what tells them apart is that ours are always in the
          # store: a mutable path is somebody's checkout, a store path is a
          # previous generation of this module's work.
          case "$current" in
            /nix/store/*) : ;;
            *) return 0 ;;
          esac
        elif [ "$kind" = github ]; then
          owner="$(printf '%s' "$entry" | jq -r '.source.owner // ""')"
          repo="$(printf '%s' "$entry" | jq -r '.source.repo // ""')"
          if [ "$owner/$repo" = kurisu-agent/herdr-drip ]; then
            # Our own previous work: a fetched checkout from back when this
            # module installed over the network. Collect it below, once the
            # link that replaces it has actually succeeded.
            managed="$(printf '%s' "$entry" | jq -r '.source.managed_path // ""')"
          else
            echo "herdr-drip: $id was installed from $kind:$owner/$repo; replacing that registration with this drip's copy (its files are left on disk)" >&2
          fi
        fi
      fi
      if "$herdr" plugin link "$root" >/dev/null; then
        changed=1
        case "$managed" in
          "$HOME"/.config/herdr/plugins/github/*)
            # Scoped hard: non-empty, ours, and under the directory herdr
            # keeps fetched checkouts in. Anything else is left standing.
            rm -rf "$managed"
            ;;
        esac
      else
        echo "herdr-drip: linking $name from $root failed; will retry next activation" >&2
      fi
    }

    plugins_json="$("$herdr" plugin list --json 2>/dev/null)" || plugins_json=
    if [ -n "$plugins_json" ] && printf '%s' "$plugins_json" | jq -e '.result.plugins' >/dev/null 2>&1; then
    ${lib.concatStringsSep "\n  " (
      map (
        p:
        "ensure_plugin ${lib.escapeShellArg p.name} ${lib.escapeShellArg p.id} ${lib.escapeShellArg "${etcRoot}/${p.name}"}"
      ) pluginList
    )}
    else
      # `plugin list` has an offline path of its own, so this is not the
      # server being down — it is a registry that cannot be read at all.
      echo "herdr-drip: could not read the plugin registry; plugins left as-is" >&2
    fi

    if [ -n "$changed" ]; then
      "$herdr" server reload-config >/dev/null 2>&1 || true
    fi
  '';
in
{
  options.services.herdr-drip.plugins = {
    enable = lib.mkEnableOption "the drip's herdr plugins, curated config and runtime deps, provisioned declaratively";

    plugins = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [
        "agent-scope"
        "beads"
        "flip-split"
        "gumbo-usage"
        "pane-titles"
        "smart-focus"
        "worktree-graph"
        "worktree-tokens"
      ];
      description = ''
        Top-level plugin directories of this repo to keep linked
        (`hello`, the template, is deliberately not in the default).
        Removing a name does NOT unlink it — the module never touches
        plugins outside this list. Set to `[ ]` to provision none of them
        and leave the user to `herdr plugin install` whatever they want.
      '';
    };

    beadsPackage = lib.mkOption {
      type = lib.types.nullOr lib.types.package;
      default = beadsDefault;
      defaultText = lib.literalExpression "this flake's pinned beads (github:gastownhall/beads/v1.2.2)";
      description = ''
        The `bd` the beads plugin runs. Scoped to that plugin's own
        commands — it does not land on any interactive PATH — so the rail
        works on a host where `bd` is otherwise only inside a repo's
        devshell, which is where the plugin previously found nothing and
        drew an empty rail without saying why.

        Point this at a bd the host ALREADY has rather than taking the
        default, when it has one: two beads of different versions on one
        box is the hazard the pin comment in this flake describes, because
        the first write by the newer one migrates the shared on-disk schema
        and the older one then refuses to read it. On a drift-rust circuit
        that means `inputs.drift-rust.packages.''${pkgs.system}.bd`.

        `null` disables the injection: bd then has to be on the herdr
        server's PATH by some other means, and the rail stays empty until
        it is.
      '';
    };

    beadsSettings = lib.mkOption {
      type = lib.types.attrsOf lib.types.str;
      default = { };
      example = lib.literalExpression ''
        {
          HERDR_DRIP_BEADS_ROWS = "3";
          HERDR_DRIP_BEADS_STATUSES = "blocked,in_progress";
          HERDR_DRIP_BEADS_INTERVAL = "30";
        }
      '';
      description = ''
        Settings for the beads plugin, as environment variables set on its own
        commands and its board binary — and on nothing else, the same scoping
        `beadsPackage` gets.

        This is the declarative half of the config file the rail and the board
        share (`$HERDR_PLUGIN_CONFIG_DIR/config.json`, documented in
        `beads/board/src/config.rs`): both read the environment first, so a
        host can state what its rail and board look like without writing JSON
        into a directory herdr manages, and without touching the herdr
        server's environment.

        The knobs, all optional:

        - `HERDR_DRIP_BEADS_ROWS` — rows the rail draws (default 5);
        - `HERDR_DRIP_BEADS_STATUSES` — comma-separated status vocabulary: an
          order for the board, a filter for the rail;
        - `HERDR_DRIP_BEADS_SHOW_CLOSED` — `1` to include closed beads;
        - `HERDR_DRIP_BEADS_INTERVAL` — seconds between rail refreshes (15);
        - `HERDR_DRIP_BEADS_LIMIT` — outer ceiling on beads written (40);
        - `HERDR_DRIP_BEADS_CWD` — pin both surfaces to one repo instead of
          following focus;
        - `HERDR_DRIP_BD_BIN` — the `bd` to run, if not `beadsPackage`'s.

        Set as DEFAULTS: the same variable in the herdr server's environment
        still wins, because that is where somebody changes a setting for an
        afternoon.
      '';
    };

    extraPlugins = lib.mkOption {
      type = lib.types.attrsOf (
        lib.types.submodule {
          options = {
            id = lib.mkOption {
              type = lib.types.str;
              example = "acme.thing";
              description = ''
                The plugin's manifest id. Stated rather than read out of the
                manifest, so that evaluating this module never has to build
                `path` first (import-from-derivation).
              '';
            };
            path = lib.mkOption {
              type = lib.types.either lib.types.path lib.types.package;
              description = ''
                Directory holding the plugin's `herdr-plugin.toml`. Published
                at /etc/${etcSubdir}/<name> and linked from there, so it is
                subject to the same rules as the drip's own: read-only, no
                [[build]] step is ever run, and anything the manifest needs
                built must already be in it.
              '';
            };
          };
        }
      );
      default = { };
      example = lib.literalExpression ''
        {
          my-plugin = {
            id = "acme.my-plugin";
            path = inputs.acme-plugins + "/my-plugin";
          };
        }
      '';
      description = ''
        Plugins from outside this repo, provisioned the same way the drip's
        own are — declaratively, from the store, with no fetch at activation.
        The attribute name is the directory name under /etc/${etcSubdir}.
      '';
    };

    herdrPackage = lib.mkOption {
      type = lib.types.nullOr lib.types.package;
      default = null;
      description = ''
        herdr package whose CLI links the plugins. null resolves
        `herdr` from the running user's PATH at activation time (system
        profile, ~/.nix-profile, per-user profile) and warns instead of
        failing when absent — right for fleets where herdr arrives
        differently per host.
      '';
    };

    users = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = config.services.claude-code.users or [ ];
      defaultText = lib.literalExpression "config.services.claude-code.users or [ ]";
      description = ''
        Users to provision via per-user system oneshots — the backstop for
        hosts with no systemd user manager (no logind / PAM session).
        Rides nix-claude-drip's user list when that module is present;
        empty means user activation only, which is enough where every
        user gets a session.
      '';
    };

    settings = lib.mkOption {
      type = settingsFormat.type;
      default = { };
      example = lib.literalExpression ''
        {
          ui.tab_bar_position = "top";
          theme.name = "gruvbox";
        }
      '';
      description = ''
        herdr config (config.toml) as Nix values. With `curatedDefaults`
        on, the drip's curated config/herdr.toml sits underneath as
        leaf-level defaults, so a key set here overrides just that key and
        every other curated setting stays. Lists are leaves — overriding
        e.g. `keys.command` replaces the whole list, not one entry.
        `lib.mkForce` a subtree to drop the curated contents of it
        entirely. Only consulted while `manageConfig` is true.
      '';
    };

    theme = {
      enable = lib.mkOption {
        type = lib.types.bool;
        default = true;
        description = ''
          Generate herdr's `[theme.custom]` tokens and `ui.accent` from
          `theme.palette`. Off, the colour scheme is whatever the curated
          config and `settings` say — use it on a host that themes herdr by
          hand, or one that wants a built-in theme unmodified
          (`settings.theme.name`) with no per-token overrides on top.
        '';
      };

      palette = lib.mkOption {
        type = lib.types.attrsOf lib.types.str;
        default = dripTheme.defaultPalette;
        defaultText = lib.literalExpression "the drip's Catppuccin Mocha palette (nix/theme.nix)";
        example = lib.literalExpression "inputs.nix-env.lib.\${pkgs.system}.palette";
        description = ''
          Colour name -> hex, in the shape `nix-env/lib/palette.nix`
          produces: the Catppuccin names (`mauve`, `green`, `surface0`, …)
          plus the role aliases layered over them (`accent`, `bg_alt`,
          `bg_surface`, `primary`, `secondary`). Roles win where a herdr
          token has one, so re-pointing `accent` re-tints herdr without
          touching a colour name.

          Our palette is the DEFAULT — a host that sets nothing comes up in
          our colour scheme. Setting this replaces it wholesale (it is one
          value, not one per colour), which is what a host with different
          colours wants; to move a single colour, override the palette at
          its own source and pass that, or set the one herdr token through
          `settings.theme.custom`.

          Only the keys `nix/theme.nix` reads matter; extra keys are
          ignored, so a fuller palette passes through unharmed.
        '';
      };

      transparentChrome = lib.mkOption {
        type = lib.types.bool;
        default = true;
        description = ''
          Leave herdr's two chrome planes — `panel_bg` (tab bar, floating
          panels, overlays, modals) and `sidebar_bg` — transparent, so they
          inherit the terminal's own background instead of being painted.

          This is the one thing `palette` cannot say: every value in a
          palette is a hue, and transparency is the absence of one. herdr
          spells it `reset`, and its own default for `sidebar_bg` is that.

          On, which is the default and the drip's appearance: herdr sits
          inside zellij inside a terminal that already has a background, and
          an opaque plane a shade off from it is a visible seam for nothing.
          Off paints both planes with the palette's `bg_alt` (mantle) — the
          shade the zellij theme uses for its own chrome — for a host that
          wants herdr opaque.

          Only consulted while `theme.enable` is true.
        '';
      };
    };

    curatedDefaults = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Layer the drip's curated config/herdr.toml under `settings` as
        defaults. Off, `settings` is the whole config and this module
        provisions nothing it was not told: use it on a host that wants the
        plugins and the deps but has its own opinions about herdr's config.
      '';
    };

    manageConfig = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Keep ~/.config/herdr/config.toml a symlink to the generated config
        (the curated defaults merged with `settings`, as a store copy).
        Never overwrites a symlink pointing outside the store, so
        apply-config.sh's working-tree link — the dev loop — wins.
      '';
    };

    adoptConfig = lib.mkOption {
      type = lib.types.enum [
        "always"
        "plain-file"
        "never"
      ];
      default = "always";
      example = "plain-file";
      description = ''
        How far to go in taking over a `~/.config/herdr/config.toml` this
        module did not put there.

        `always` (the default) also replaces a symlink pointing outside the
        store — `apply-config.sh`'s link into a checkout. Only the link is
        replaced: the checkout's file is never followed, written, or removed.
        This is what a deployment wants, because a checkout that stops being
        pulled pins herdr to whatever the drip looked like the day someone ran
        that script, and nothing says so.

        `plain-file` adopts only a regular file (herdr writes one itself
        during onboarding, and from the theme and sound pickers) and leaves a
        linked checkout alone. **This is the setting for a host the drip is
        developed on**, where that link is the dev loop.

        `never` leaves anything that already exists alone and reports the
        mismatch on stderr.

        A displaced regular file is always kept as `config.toml.bak`
        (timestamp-suffixed rather than overwriting an existing one).
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    # Three layers, weakest first, all at leaf level so a host that sets one
    # key keeps every other one:
    #
    #   1000 (mkDefault)  the curated config/herdr.toml, as before;
    #    900              the palette-generated theme;
    #    100 (a plain     whatever the host puts in `settings`.
    #         assignment)
    #
    # The middle rung is what makes the palette authoritative for colour on
    # the nix path. config/herdr.toml carries the same tokens — it has to,
    # because apply-config.sh links that file verbatim and the non-nix dev
    # loop would otherwise have no theme — but they are the RENDER of the
    # default palette, not a second opinion about it. A host that passes its
    # own palette must not have that render silently win, and it does not:
    # 900 outranks 1000. A host that sets `settings.theme.custom.<token>`
    # directly still outranks both.
    services.herdr-drip.plugins.settings = lib.mkMerge [
      (lib.mkIf cfg.curatedDefaults (lib.mapAttrsRecursive (_: lib.mkDefault) curatedSettings))
      (lib.mkIf cfg.theme.enable (
        lib.mapAttrsRecursive (_: lib.mkOverride 900) (
          dripTheme.mkTheme {
            inherit (cfg.theme) palette transparentChrome;
          }
        )
      ))
    ];

    # The published plugin directories. This is also what keeps them alive:
    # the registry holds paths, not store references, so without an entry in
    # the system closure the GC would be free to collect a plugin that a
    # running herdr still points at.
    environment.etc = lib.listToAttrs (
      map (p: lib.nameValuePair "${etcSubdir}/${p.name}" { source = p.package; }) pluginList
    );

    # yolo-shell is the config's PATH-resolved default_shell; bun is
    # worktree-graph's pane command. python3 is deliberately ABSENT: its only
    # consumer is the claude agent-state hook, and claude-agent-state.nix
    # injects a store python3 scoped to that one command — putting it here
    # would widen every interactive PATH for nothing.
    environment.systemPackages = [
      yoloShell
      pkgs.bun
    ];

    # No network access at activation any more: this reads a registry and
    # writes symlinks. It stays an activation script rather than something
    # ordered after the network because there is nothing left to wait for.
    system.userActivationScripts.herdrDripPlugins = "${ensurePlugins}";

    # Mirror of the settings-installer backstop: user activation never fires
    # on hosts with no systemd user manager, so cover the named users from a
    # per-user SYSTEM oneshot too.
    systemd.services = lib.listToAttrs (
      map (
        u:
        lib.nameValuePair "herdr-drip-plugins-${u}" {
          description = "herdr-drip: provision drip plugins + config for ${u}";
          wantedBy = [ "multi-user.target" ];
          serviceConfig = {
            Type = "oneshot";
            RemainAfterExit = true;
            User = u;
            Group = config.users.users.${u}.group;
            Environment = "HOME=${config.users.users.${u}.home}";
            ExecStart = "${ensurePlugins}";
          };
        }
      ) cfg.users
    );
  };
}
