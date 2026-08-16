# The beads plugin's board binary, built by nix instead of by activation.
#
# beads/board is a fork of miiraheart/herdr-beads (see beads/board/UPSTREAM.md):
# a ratatui TUI over `bd`, and the second surface of the `beads` plugin — the
# board you open when the sidebar rail's one line is not enough.
#
# Its manifest half declares `[[build]] command = ["cargo", "build",
# "--release"]`, which is the same activation-time build nix/plugins.nix took
# out of the loop for worktree-graph: `herdr plugin link` never runs [[build]],
# and a store copy of a plugin directory has no toolchain, no network and no
# writable root to run one in. So the build happens here and
# nix/drip-plugins.nix splices the result in at the path the manifest names.
#
# Unlike worktree-graph's node_modules this is NOT a fixed-output derivation
# and needs no hash bumping: cargoLock.lockFile reads beads/board/Cargo.lock —
# which upstream gitignores and we commit for exactly this reason — and nix
# fetches each crate as its own FOD keyed by the lock. Changing a dependency is
# `cargo update` and nothing else.
#
# On a memory-constrained box, build it at `--cores 4 --max-jobs 2` (CLAUDE.md).
pkgs:
let
  inherit (pkgs) lib;

  cargoToml = builtins.fromTOML (builtins.readFile ../beads/board/Cargo.toml);
in
pkgs.rustPlatform.buildRustPackage {
  pname = "herdr-beads";
  inherit (cargoToml.package) version;

  # `target/` is gitignored, so a flake build never sees one — but the NixOS
  # module can be imported from a working PATH, and there it is whatever the
  # last `cargo build --release` left (the dev loop the plugin manifest's
  # [[build]] still describes). Filtering it here keeps that out of the source
  # hash, so a dev box and a clean checkout build the same derivation.
  src = lib.cleanSourceWith {
    name = "drip-beads-board-src";
    src = ../beads/board;
    filter = path: _type: baseNameOf path != "target";
  };

  cargoLock.lockFile = ../beads/board/Cargo.lock;

  # `cargo test` is cheap here and worth keeping: the bd JSON parsers are the
  # part of this fork most likely to break under a bd bump, and their tests
  # run against include_str!'d fixtures, so the check needs no bd and no
  # network. That is the whole test suite upstream ships.
  doCheck = true;

  meta = {
    inherit (cargoToml.package) description;
    homepage = cargoToml.package.repository;
    license = lib.licenses.mit;
    mainProgram = "herdr-beads";
  };
}
