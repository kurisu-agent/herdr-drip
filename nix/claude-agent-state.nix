# claude-agent-state — keep herdr's Claude integration alive under
# nix-claude-drip's settings.json overwrite.
#
# `herdr integration install claude` writes two things:
#   1. the hook script      ~/.claude/hooks/herdr-agent-state.sh
#   2. a hooks.SessionStart entry in ~/.claude/settings.json
# On a host running nix-claude-drip, (2) does not survive: that module
# installs settings.json by wholesale overwrite, from user activation on
# every nixos-rebuild switch AND from a per-user boot oneshot. The failure
# is silent — the script stays on disk, `herdr integration status` only
# stats the script and keeps reporting "current", and herdr quietly falls
# back to process-name detection.
#
# This module:
#   - re-asserts the SessionStart entry declaratively through
#     services.claude-code.settings, so both of nix-claude-drip's delivery
#     paths carry it (its mkSettings recursiveUpdate merges nested attrs,
#     and `hooks` is not module-owned there, so nothing fights it);
#   - guarantees python3 in the system environment — the hook script
#     (integration v7) guards `command -v python3 || exit 0` and is
#     completely inert without it, while still reporting healthy;
#   - re-runs `herdr integration install claude` (per user) whenever herdr
#     reports the hook script missing or outdated, so the script side
#     tracks herdr's integration version instead of pinning a vendored
#     copy here that would go stale on the next version bump.
#
# Requires the nix-claude-drip module (services.claude-code) imported and
# enabled. On a host without it, plain `herdr integration install claude`
# works and stays put — this module exists to survive the overwrite.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.services.herdr-drip.claudeAgentState;

  # The declared settings.json entry. herdr's own installer writes an
  # absolute per-user path ("bash '/home/<u>/.claude/...' session"), which
  # cannot live in nix-claude-drip's single settings.json shared by every
  # user — hook commands run through a shell, so $HOME does the per-user
  # part. The [ -x ] guard keeps the entry inert for a user who has never
  # had the integration installed; a dangling command would error on every
  # SessionStart.
  hookScript = "$HOME/.claude/hooks/herdr-agent-state.sh";
  hookEntry = {
    matcher = "*";
    hooks = [
      {
        type = "command";
        command = "[ -x \"${hookScript}\" ] || exit 0; exec bash \"${hookScript}\" session";
        timeout = 10;
      }
    ];
  };

  resolveHerdr =
    if cfg.herdrPackage != null then
      ''
        herdr=${cfg.herdrPackage}/bin/herdr
      ''
    else
      ''
        herdr="$(command -v herdr || true)"
        if [ -z "$herdr" ]; then
          echo "herdr-drip: herdr not on PATH; claude agent-state integration left as-is" >&2
          exit 0
        fi
      '';

  ensureIntegration = pkgs.writeShellScript "herdr-claude-agent-state" ''
    set -eu
    export PATH=${
      lib.makeBinPath [
        pkgs.coreutils
        pkgs.gnugrep
        pkgs.jq
      ]
    }:/run/wrappers/bin:/run/current-system/sw/bin:$HOME/.nix-profile/bin:/etc/profiles/per-user/$(id -un)/bin''${PATH:+:$PATH}

    ${resolveHerdr}

    case "$("$herdr" integration status 2>/dev/null | grep '^claude:' || true)" in
      "claude: current"*) exit 0 ;;
    esac

    "$herdr" integration install claude

    # herdr's installer also appended ITS settings.json entry — it matches
    # by exact command string, so it never recognises the $HOME form this
    # module declares. settings.json is nix-managed here: drop herdr's
    # absolute-path copy (and only that — the `^bash ` anchor cannot match
    # the declared entry, which starts with `[ -x`) so the file stays at
    # the declared content instead of double-firing until the next rebuild.
    settings="$HOME/.claude/settings.json"
    if [ -f "$settings" ]; then
      jq 'if (.hooks.SessionStart? | type) == "array" then
            .hooks.SessionStart |= map(select(
              ([.hooks[]?.command // ""] | any(test("^bash .*herdr-agent-state"))) | not
            ))
          else . end' "$settings" >"$settings.herdr-drip.tmp"
      mv "$settings.herdr-drip.tmp" "$settings"
    fi
  '';
in
{
  options.services.herdr-drip.claudeAgentState = {
    enable = lib.mkEnableOption "herdr's Claude agent-state integration, kept alive across nix-claude-drip's settings.json overwrites";

    herdrPackage = lib.mkOption {
      type = lib.types.nullOr lib.types.package;
      default = null;
      description = ''
        herdr package whose CLI installs the integration. null resolves
        `herdr` from the running user's PATH at activation time (system
        profile, ~/.nix-profile, per-user profile) and warns instead of
        failing when absent. Setting it also gives the per-user oneshot a
        restart trigger on herdr upgrades.
      '';
    };

    users = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = config.services.claude-code.users;
      defaultText = lib.literalExpression "config.services.claude-code.users";
      description = ''
        Users to provision the integration for via per-user system
        oneshots — the same backstop nix-claude-drip uses for its settings
        installer on hosts with no systemd user manager (no logind / PAM
        session). Empty means user activation only, which is enough where
        every user gets a session.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = config.services.claude-code.enable;
        message = ''
          services.herdr-drip.claudeAgentState compensates for
          nix-claude-drip's settings.json overwrite; without
          services.claude-code enabled there is nothing to survive — run
          `herdr integration install claude` directly instead.
        '';
      }
    ];

    services.claude-code.settings.hooks.SessionStart = [ hookEntry ];

    # The hook script no-ops (exit 0, no log) without python3 on PATH.
    environment.systemPackages = [ pkgs.python3 ];

    # After claudeDripSettings so the post-install cleanup in
    # ensureIntegration sees the declared settings.json, not the
    # pre-overwrite one.
    system.userActivationScripts.herdrClaudeAgentState = {
      text = "${ensureIntegration}";
      deps = [ "claudeDripSettings" ];
    };

    # Mirror of nix-claude-drip's delivery path #2: user activation never
    # fires on hosts with no systemd user manager, so cover the named
    # users from a per-user SYSTEM oneshot too. Ordering after the
    # settings oneshot is best-effort (after= on an absent unit is
    # harmless).
    systemd.services = lib.listToAttrs (
      map (
        u:
        lib.nameValuePair "herdr-drip-claude-agent-state-${u}" {
          description = "herdr-drip: ensure herdr's claude integration for ${u}";
          wantedBy = [ "multi-user.target" ];
          after = [ "claude-drip-settings-${u}.service" ];
          serviceConfig = {
            Type = "oneshot";
            RemainAfterExit = true;
            User = u;
            Group = config.users.users.${u}.group;
            Environment = "HOME=${config.users.users.${u}.home}";
            ExecStart = "${ensureIntegration}";
          };
        }
      ) cfg.users
    );
  };
}
