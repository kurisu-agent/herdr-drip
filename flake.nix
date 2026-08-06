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
          default = yolo-shell;
        }
      );
    };
}
