# Each plugin directory as a store path — the drip, ready to link.
#
# `herdr plugin install owner/repo/subdir` fetches a directory and (for
# manifests that declare one) runs its [[build]]. Both halves of that are
# things nix already does better: the directory is right here in the flake
# source, and the build is nix/worktree-graph-deps.nix. What is left for
# activation is `herdr plugin link`, which needs no network, no git, and no
# running server — see nix/plugins.nix.
#
# One function over pkgs, the same shape as nix/yolo-shell.nix and for the
# same reason: flake.nix builds these from its own nixpkgs for `nix build`,
# while the NixOS module builds them from the HOST's, and neither definition
# can drift from the other.
pkgs:
let
  inherit (pkgs) lib;

  # Spliced into the plugin directory at build time, standing in for what the
  # manifest's [[build]] would have produced. Keyed by plugin directory so a
  # second plugin growing a build step adds a line here rather than a special
  # case in the builder below.
  buildOutputs = {
    # Linked from inside the deps derivation rather than at its root, because
    # bun only honours a directory named exactly `node_modules` — see the
    # installPhase comment in nix/worktree-graph-deps.nix.
    worktree-graph.node_modules = "${import ./worktree-graph-deps.nix pkgs}/node_modules";

    # The board binary, at the path beads/herdr-plugin.toml's [[panes]] command
    # names — which is cargo's own output path for the crate in beads/board,
    # so that a store build and a `cargo build --release` in a linked working
    # tree put the binary in the same place and the manifest needs no idea
    # which one it got. The leading `./` in the manifest is load-bearing (herdr
    # hands the command to portable_pty's CommandBuilder, which only treats it
    # as a path when it contains a `/`), and this destination being three
    # directories deep is why the splice below has to mkdir first.
    beads."board/target/release/herdr-beads" = "${import ./beads-board.nix pkgs}/bin/herdr-beads";
  };

  # Generated content is excluded rather than trusted: on a flake build the
  # source is a clean git tree and these never exist, but the NixOS module can
  # be imported from a working PATH (a dev checkout), and there node_modules/
  # is 22M of whatever the last `bun install` left. It must not reach the
  # store path — the whole point is that buildOutputs decides what is there.
  ignored = [
    "node_modules"
    "dist"
    "target"
    ".git"
  ];

  pluginSrc =
    name:
    lib.cleanSourceWith {
      name = "drip-plugin-${name}-src";
      src = ../. + "/${name}";
      filter = path: _type: !(builtins.elem (baseNameOf path) ignored);
    };
in
rec {
  # The plugin directory for `name`, as a read-only store path. Verified to
  # work as a herdr plugin_root: commands run with it as cwd, and every
  # plugin writes to $HERDR_PLUGIN_STATE_DIR / $HERDR_PLUGIN_CONFIG_DIR
  # under $HOME instead of into its own root.
  mkPlugin = mkPluginWith { };

  # As `mkPlugin`, but with `runtimeInputs` spliced onto the PATH of every
  # executable the plugin runs — the idiom claude-agent-state.nix uses for
  # python3, and for the same reason nix/plugins.nix gives for keeping python3
  # out of systemPackages: a tool with ONE consumer is scoped to that consumer
  # instead of widening every interactive PATH.
  #
  # "Every executable" is the plugin's own bin/ plus whatever buildOutputs
  # spliced in, because the beads board needs the same `bd` bin/sync does and
  # is a binary at target/release/ rather than a script in bin/. A spliced
  # DIRECTORY (worktree-graph's node_modules) fails the -f test and is skipped.
  #
  # The wrapper leaves the manifest alone: `[[startup]] command = ["bin/watch"]`
  # and `[[panes]] command = ["./board/target/release/herdr-beads"]` still name
  # files at those paths, and the real script keeps resolving its own root
  # through `dirname $BASH_SOURCE/..` because wrapProgram leaves the hidden
  # original beside it.
  #
  # `runtimeEnv` rides the same wrapper, and is how a NixOS host sets a
  # plugin's settings without a config file. --set-DEFAULT deliberately: every
  # drip plugin reads its knobs from the environment first, and the herdr
  # SERVER's environment is where a person sets one for an afternoon. That
  # person outranks the host's config, which is the layering `settings` already
  # has with config/herdr.toml.
  mkPluginWith =
    { runtimeInputs ? [ ], runtimeEnv ? { } }:
    name:
    let
      splices = buildOutputs.${name} or { };
      wrapping = runtimeInputs != [ ] || runtimeEnv != { };
      wrapArgs =
        lib.optionalString (runtimeInputs != [ ]) "--prefix PATH : ${lib.makeBinPath runtimeInputs}"
        + lib.concatStrings (
          lib.mapAttrsToList (
            key: value: " --set-default ${lib.escapeShellArg key} ${lib.escapeShellArg value}"
          ) runtimeEnv
        );
    in
    pkgs.runCommandLocal "drip-plugin-${name}"
      {
        nativeBuildInputs = lib.optional wrapping pkgs.makeWrapper;
      }
      ''
        cp -R ${pluginSrc name} $out
        chmod -R u+w $out
        ${lib.concatStrings (
          lib.mapAttrsToList (destination: source: ''
            mkdir -p "$(dirname "$out/${destination}")"
            ln -s ${source} $out/${destination}
          '') splices
        )}
        ${lib.optionalString wrapping ''
          for command in "$out"/bin/* ${
            lib.concatMapStringsSep " " (destination: "\"$out\"/${destination}") (lib.attrNames splices)
          }; do
            [ -f "$command" ] && [ -x "$command" ] || continue
            wrapProgram "$command" ${wrapArgs}
          done
        ''}
      '';

  # Every plugin directory in the repo, by name. `hello` is the template and
  # is deliberately not in nix/plugins.nix's default list, but it builds like
  # any other — it is what to link when testing this machinery.
  all = lib.genAttrs [
    "agent-scope"
    "beads"
    "flip-split"
    "gumbo-usage"
    "hello"
    "pane-titles"
    "smart-focus"
    "worktree-graph"
    "worktree-tokens"
  ] mkPlugin;
}
