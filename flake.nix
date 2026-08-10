{
  description = "herdr-drip — our herdr plugins and the yolo-shell pane shell";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      forAllSystems = nixpkgs.lib.genAttrs [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        rec {
          # See nix/yolo-shell.nix — shared with the plugins module, which
          # builds the same thing from the host's nixpkgs.
          yolo-shell = import ./nix/yolo-shell.nix pkgs;

          # Everything the drip needs at runtime, resolved against the herdr
          # SERVER's PATH, in one profile add: yolo-shell (default_shell),
          # bun (worktree-graph's [[build]] and pane command), python3 (the
          # claude agent-state hook execs an inline python heredoc — and
          # silently no-ops without it).
          herdr-drip-deps = pkgs.buildEnv {
            name = "herdr-drip-deps";
            paths = [
              yolo-shell
              pkgs.bun
              pkgs.python3
            ];
          };

          default = yolo-shell;
        }
      );

      # Hardcore plugins — the drip's source patches on herdr itself, for
      # what herdr has no plugin surface for. One function so every consumer
      # applies the identical set; see nix/herdr-patches.nix for the rules
      # (fail-loudly, one story per patch, never apply twice).
      lib.patchHerdr = import ./nix/herdr-patches.nix;

      nixosModules = rec {
        # For NixOS hosts running nix-claude-drip (services.claude-code):
        # keeps herdr's claude agent-state integration alive across the
        # settings.json overwrites that module performs on every rebuild and
        # boot. See nix/claude-agent-state.nix.
        claude-agent-state = import ./nix/claude-agent-state.nix;

        # Declarative installs of the drip's plugins + curated config +
        # runtime deps (see nix/plugins.nix). The wrapper pins installs to
        # this flake's own rev, so a consumer bumping the flake input moves
        # the installed plugins with it; a dirty checkout has no rev and the
        # module warns instead of installing something unpinned.
        plugins = {
          imports = [ ./nix/plugins.nix ];
          services.herdr-drip.plugins.ref = nixpkgs.lib.mkDefault (self.rev or null);
        };

        # The standard procedure: both halves.
        default = {
          imports = [
            claude-agent-state
            plugins
          ];
        };
      };
    };
}
