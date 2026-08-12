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

          # worktree-graph's `bun install`, as a fixed-output derivation. Built
          # here so the hash can be bumped without a NixOS host in the loop:
          # `nix build .#worktree-graph-node-modules` prints what to paste into
          # nix/worktree-graph-deps.nix when bun.lock moves.
          worktree-graph-node-modules = import ./nix/worktree-graph-deps.nix pkgs;

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
        # Each plugin directory as the store path the NixOS module publishes
        # and links (`nix build .#plugin-worktree-graph` to inspect one).
        // (nixpkgs.lib.mapAttrs' (
          name: nixpkgs.lib.nameValuePair "plugin-${name}"
        ) (import ./nix/drip-plugins.nix pkgs).all)
      );

      # Hardcore plugins — the drip's source patches on herdr itself, for
      # what herdr has no plugin surface for. One function so every consumer
      # applies the identical set; see nix/herdr-patches.nix for the rules
      # (fail-loudly, one story per patch, never apply twice).
      #
      # `rev` is bound HERE rather than by the caller, so `patchHerdr` stays
      # the one-argument function every consumer already applies
      # (nix-claude-drip's herdr knob, drift-rust's guest tools) and the
      # sidebar's version line still names the drip that patched it. Same
      # source as the plugin pin below, and the same dirty-checkout story: no
      # rev, no hash in the header.
      lib.patchHerdr = import ./nix/herdr-patches.nix { rev = self.rev or null; };

      nixosModules = rec {
        # For NixOS hosts running nix-claude-drip (services.claude-code):
        # keeps herdr's claude agent-state integration alive across the
        # settings.json overwrites that module performs on every rebuild and
        # boot. See nix/claude-agent-state.nix.
        claude-agent-state = import ./nix/claude-agent-state.nix;

        # Declarative provisioning of the drip's plugins + curated config +
        # runtime deps (see nix/plugins.nix). The plugins are store paths
        # built from THIS flake's source and registered with `herdr plugin
        # link`, so the consumer's flake input decides exactly which plugin
        # code runs and `nix flake update herdr-drip` moves it — with no rev
        # to pin, no fetch at activation, and no dirty-checkout special case.
        plugins = import ./nix/plugins.nix;

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
