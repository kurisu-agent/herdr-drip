# plugins — the whole drip, provisioned declaratively.
#
# By hand, a host is set up per user with `herdr plugin install
# kurisu-agent/herdr-drip/<dir>` for each plugin, `scripts/apply-config.sh`
# for the config, and a `nix profile add .#herdr-drip-deps` for the
# runtime deps. This module is that procedure as NixOS config:
#
#   - keeps each of `plugins` installed, PINNED to `ref` — the flake's
#     `nixosModules.plugins` defaults `ref` to the flake's own rev, so the
#     consumer's flake input decides exactly which plugin code runs and a
#     `nix flake update herdr-drip` moves it;
#   - keeps ~/.config/herdr/config.toml a symlink to a GENERATED config,
#     unless the user has taken it over: the curated config/herdr.toml
#     layered under the host's `settings` overrides, key by key — the
#     curated values are defaults, not mandates;
#   - puts the runtime deps on the system PATH: yolo-shell (the config's
#     PATH-resolved default_shell) and bun (worktree-graph's [[build]] and
#     pane command). Commands resolve against the herdr SERVER's PATH, and
#     /run/current-system/sw/bin is on it even for a server already running
#     at switch time, so this lands without a restart.
#
# The dev loop always wins: a `herdr plugin link`ed working tree
# (source.kind = "local") is left alone silently, and a config.toml
# symlinked anywhere outside /nix/store (apply-config.sh's link into a
# checkout) is left alone with a note. Third-party plugins and plugins not
# named in `plugins` are never touched.
#
# Known limitations, by design:
#   - removing a name from `plugins` does not uninstall it;
#   - a rev bump reinstalls in place, which re-ENABLES a drip plugin the
#     user had `herdr plugin disable`d (verified: install-over-existing
#     replaces and resets enabled);
#   - `ref = null` (a dirty flake has no rev) skips installs with a
#     warning; config and deps are still managed.
#
# Unlike claude-agent-state.nix this module needs nothing from
# nix-claude-drip — it only defaults `users` to claude-code's list when
# that module happens to be present.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.services.herdr-drip.plugins;

  # Install addresses a plugin by repo subdir (`flip-split`); list and
  # uninstall address it by manifest id (`drip.flip-split`). Resolve the id
  # from each manifest at EVAL time, so a typo in `plugins` fails the build
  # here rather than failing at activation on every host, and the id stays
  # authoritative if the drip.<dir> convention ever changes.
  pluginList = map (name: {
    inherit name;
    id = (builtins.fromTOML (builtins.readFile (../. + "/${name}/herdr-plugin.toml"))).id;
  }) cfg.plugins;

  settingsFormat = pkgs.formats.toml { };

  # The curated config as data. It enters the module system below as
  # leaf-level mkDefaults, so a host overrides one key and keeps the rest;
  # the tracked TOML stays the single source of truth for the non-nix path
  # (apply-config.sh links it verbatim).
  curatedSettings = builtins.fromTOML (builtins.readFile ../config/herdr.toml);

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
    # bun and git are for `herdr plugin install`: the fetch runs in the CLI
    # process, and so may a manifest's [[build]] (worktree-graph's
    # `bun install`). Both are scoped to this script; the SERVER gets its
    # bun from systemPackages below.
    export PATH=${
      lib.makeBinPath [
        pkgs.coreutils
        pkgs.jq
        pkgs.bun
        pkgs.git
      ]
    }:/run/wrappers/bin:/run/current-system/sw/bin:$HOME/.nix-profile/bin:/etc/profiles/per-user/$(id -un)/bin''${PATH:+:$PATH}

    ${resolveHerdr}

    changed=

    ${lib.optionalString cfg.manageConfig ''
      # ---- config.toml ------------------------------------------------------
      # Managed only while it is ours to manage: a fresh host gets the link, a
      # stale store link is retargeted, and anything the user did on purpose —
      # apply-config.sh's link into a checkout, a hand-written file — is left
      # standing with a note.
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
            *)
              echo "herdr-drip: config.toml -> $dest (not store-managed); leaving it" >&2
              ;;
          esac
        fi
      elif [ -e "$target" ]; then
        echo "herdr-drip: config.toml is a plain file; leaving it (scripts/apply-config.sh adopts it into a checkout, or remove it for the managed copy)" >&2
      else
        mkdir -p "$(dirname "$target")"
        ln -s "$expected" "$target"
        changed=1
      fi
    ''}

    # ---- plugins ------------------------------------------------------------
    ref=${lib.escapeShellArg (toString cfg.ref)}

    ensure_plugin() {
      local subdir=$1 id=$2 entry kind owner repo rev
      entry="$(printf '%s' "$plugins_json" | jq -c --arg id "$id" \
        '[.result.plugins[] | select(.plugin_id == $id)] | first // empty')"
      if [ -n "$entry" ]; then
        kind="$(printf '%s' "$entry" | jq -r '.source.kind // ""')"
        # A linked working tree wins silently — that is the dev loop, and
        # the link carries newer code than any pin.
        if [ "$kind" = local ]; then
          return 0
        fi
        owner="$(printf '%s' "$entry" | jq -r '.source.owner // ""')"
        repo="$(printf '%s' "$entry" | jq -r '.source.repo // ""')"
        if [ "$kind" != github ] || [ "$owner/$repo" != "kurisu-agent/herdr-drip" ]; then
          echo "herdr-drip: $id is installed from $kind:$owner/$repo, not this drip; leaving it" >&2
          return 0
        fi
        rev="$(printf '%s' "$entry" | jq -r '.source.resolved_commit // ""')"
        if [ "$rev" = "$ref" ]; then
          return 0
        fi
      fi
      # Absent and out-of-date are one path: install replaces an existing
      # same-id install in place. Guarded per plugin — one dead fetch must
      # not stop the rest; the next activation retries.
      if "$herdr" plugin install "kurisu-agent/herdr-drip/$subdir" --ref "$ref" -y; then
        changed=1
      else
        echo "herdr-drip: install of $subdir at $ref failed; will retry next activation" >&2
      fi
    }

    if [ -n "$ref" ]; then
      plugins_json="$("$herdr" plugin list --json 2>/dev/null)" || plugins_json=
      if [ -n "$plugins_json" ] && printf '%s' "$plugins_json" | jq -e '.result.plugins' >/dev/null 2>&1; then
        ${lib.concatMapStrings (
          p: "    ensure_plugin ${lib.escapeShellArg p.name} ${lib.escapeShellArg p.id}\n"
        ) pluginList}
      else
        echo "herdr-drip: 'herdr plugin list --json' unavailable (server down?); plugins left as-is" >&2
      fi
    else
      echo "herdr-drip: no rev to pin plugin installs to (dirty flake?); skipping installs" >&2
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
        "flip-split"
        "pane-titles"
        "smart-focus"
        "worktree-graph"
        "worktree-tokens"
      ];
      description = ''
        Top-level plugin directories of this repo to keep installed
        (`hello`, the template, is deliberately not in the default).
        Removing a name does NOT uninstall it — the module never touches
        plugins outside this list.
      '';
    };

    ref = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = ''
        Git rev plugin installs are pinned to. The flake's
        `nixosModules.plugins` defaults this to the flake's own rev, so
        bumping the flake input bumps the installed plugins with it. null
        (a dirty checkout has no rev) skips installs with a warning;
        config and deps are still managed.
      '';
    };

    herdrPackage = lib.mkOption {
      type = lib.types.nullOr lib.types.package;
      default = null;
      description = ''
        herdr package whose CLI installs the plugins. null resolves
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
        herdr config (config.toml) as Nix values. The drip's curated
        config/herdr.toml sits underneath as leaf-level defaults, so a key
        set here overrides just that key and every other curated setting
        stays. Lists are leaves — overriding e.g. `keys.command` replaces
        the whole list, not one entry. `lib.mkForce` a subtree to drop the
        curated contents of it entirely. Only consulted while
        `manageConfig` is true.
      '';
    };

    manageConfig = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Keep ~/.config/herdr/config.toml a symlink to the generated config
        (the curated defaults merged with `settings`, as a store copy).
        Never overwrites a hand-written file or a symlink outside the
        store, so apply-config.sh's working-tree link — the dev loop —
        wins.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    # The curated config lands as a default on every leaf, so any single
    # key a host sets through `settings` outranks it and the rest of the
    # file rides along untouched.
    services.herdr-drip.plugins.settings = lib.mapAttrsRecursive (_: lib.mkDefault) curatedSettings;

    # yolo-shell is the config's PATH-resolved default_shell; bun is
    # worktree-graph's [[build]] and pane command. python3 is deliberately
    # ABSENT: its only consumer is the claude agent-state hook, and
    # claude-agent-state.nix injects a store python3 scoped to that one
    # command — putting it here would widen every interactive PATH for
    # nothing.
    environment.systemPackages = [
      yoloShell
      pkgs.bun
    ];

    # Network access happens at activation only when a plugin is missing or
    # out of date — the same class of side effect as claude-agent-state
    # re-running `herdr integration install`.
    system.userActivationScripts.herdrDripPlugins = "${ensurePlugins}";

    # Mirror of the settings-installer backstop: user activation never fires
    # on hosts with no systemd user manager, so cover the named users from a
    # per-user SYSTEM oneshot too. Ordering after network-online is
    # best-effort — a failed fetch warns and the next activation retries.
    systemd.services = lib.listToAttrs (
      map (
        u:
        lib.nameValuePair "herdr-drip-plugins-${u}" {
          description = "herdr-drip: provision drip plugins + config for ${u}";
          wantedBy = [ "multi-user.target" ];
          wants = [ "network-online.target" ];
          after = [ "network-online.target" ];
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
