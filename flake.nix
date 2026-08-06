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
          # config/herdr.toml resolves default_shell from PATH, so putting
          # this package in your environment is all the wiring nix needs.
          # claude and zsh stay PATH-resolved on purpose: the wrapper should
          # ride whatever versions the host environment ships.
          yolo-shell = pkgs.runCommandLocal "yolo-shell" { } ''
            install -Dm755 ${./scripts/yolo-shell} $out/bin/yolo-shell
          '';

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

      # For NixOS hosts running nix-claude-drip (services.claude-code):
      # keeps herdr's claude agent-state integration alive across the
      # settings.json overwrites that module performs on every rebuild and
      # boot. See nix/claude-agent-state.nix.
      nixosModules = rec {
        claude-agent-state = import ./nix/claude-agent-state.nix;
        default = claude-agent-state;
      };
    };
}
