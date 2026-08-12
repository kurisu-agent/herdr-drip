# worktree-graph's node_modules, built by nix instead of by activation.
#
# The manifest's `[[build]] command = ["bun", "install"]` is the one piece of
# the drip that is not just tracked files: node_modules/ is gitignored, so a
# store copy of the plugin directory is missing its dependencies. `herdr
# plugin install` used to cover that by running the build step after its
# fetch — which is exactly the activation-time web request nix/plugins.nix no
# longer makes.
#
# So the fetch moves to where nix already puts fetches: a fixed-output
# derivation, the only kind allowed to reach the network. The hash below
# pins the resolved dependency tree the same way bun.lock does, one layer
# down; `herdr plugin link` never runs [[build]], and with this spliced in as
# node_modules it has nothing left to run.
#
# Bumping it: change bun.lock, build, and nix prints the hash it got.
#
#   nix build .#worktree-graph-node-modules
#   error: hash mismatch in fixed-output derivation
#            specified: sha256-AAAA...
#               got:    sha256-<the one to paste below>
#
# That is also the failure mode when a nixpkgs bump moves bun far enough to
# change its output. It fails loudly and stops the build, which is the same
# bargain nix/herdr-patches.nix takes with --replace-fail: a dependency set
# that silently changed shape is worse than a build that says so.
pkgs:
let
  # Only the two files that decide the tree. Feeding the whole plugin
  # directory here would rebuild every dependency whenever a .tsx changed,
  # and would drag the working tree's own untracked node_modules into the
  # hash on a local (non-flake) build.
  src = pkgs.runCommandLocal "drip-worktree-graph-deps-src" { } ''
    mkdir -p $out
    cp ${../worktree-graph/package.json} $out/package.json
    cp ${../worktree-graph/bun.lock} $out/bun.lock
  '';
in
pkgs.stdenvNoCC.mkDerivation {
  name = "drip-worktree-graph-node-modules";
  inherit src;

  nativeBuildInputs = [ pkgs.bun ];

  dontConfigure = true;

  buildPhase = ''
    runHook preBuild

    # bun wants a writable HOME and cache; the sandbox gives neither by
    # default. Both are scoped to the build and neither reaches the output.
    export HOME=$TMPDIR
    export BUN_INSTALL_CACHE_DIR=$TMPDIR/bun-cache

    # --frozen-lockfile is the reproducibility contract: resolve exactly what
    # bun.lock says or fail, so this output can be hashed at all.
    # --ignore-scripts keeps dependency postinstall hooks (which are free to
    # bake absolute paths or timestamps into the tree) out of the hash.
    bun install \
      --frozen-lockfile \
      --no-progress \
      --ignore-scripts

    runHook postBuild
  '';

  # The tree lands at $out/node_modules, NOT at $out. bun resolves a package
  # directory to its realpath and then walks up looking for an ancestor
  # literally named `node_modules`; a symlink to a store path called
  # `...-drip-worktree-graph-node-modules` does not qualify, and bun silently
  # falls back to whatever is in the user's global install cache — which on a
  # fresh host is nothing, and on a dev box is a different resolution than the
  # lockfile's. Wrapping it in a correctly-named directory lets nix/plugins.nix
  # link this in and be shared by every host, instead of copying the tree into
  # each plugin path to satisfy a name.
  installPhase = ''
    runHook preInstall
    mkdir -p $out
    cp -R node_modules $out/node_modules
    runHook postInstall
  '';

  outputHashMode = "recursive";
  outputHashAlgo = "sha256";
  outputHash = "sha256-e+/RTRtaQAmsI0u70izwJzYKk/AYq4RJKCZUrYmgKoM=";
}
