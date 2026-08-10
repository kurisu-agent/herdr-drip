# yolo-shell as a package. config/herdr.toml resolves default_shell from
# PATH, so putting this in your environment is all the wiring nix needs.
# claude and zsh stay PATH-resolved on purpose: the wrapper should ride
# whatever versions the host environment ships.
#
# A function over pkgs rather than a locked derivation, so flake.nix builds
# the standalone package from its own nixpkgs while the NixOS module
# (nix/plugins.nix) builds the same thing from the HOST's — one definition,
# no drift between the two.
pkgs:
pkgs.runCommandLocal "yolo-shell" { } ''
  install -Dm755 ${../scripts/yolo-shell} $out/bin/yolo-shell
''
