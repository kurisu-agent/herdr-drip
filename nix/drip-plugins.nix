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
  };

  # Generated content is excluded rather than trusted: on a flake build the
  # source is a clean git tree and these never exist, but the NixOS module can
  # be imported from a working PATH (a dev checkout), and there node_modules/
  # is 22M of whatever the last `bun install` left. It must not reach the
  # store path — the whole point is that buildOutputs decides what is there.
  ignored = [
    "node_modules"
    "dist"
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
  # command in the plugin's own bin/ — the idiom claude-agent-state.nix uses
  # for python3, and for the same reason nix/plugins.nix gives for keeping
  # python3 out of systemPackages: a tool with ONE consumer is scoped to that
  # consumer instead of widening every interactive PATH.
  #
  # The wrapper leaves the manifest alone: `[[startup]] command = ["bin/watch"]`
  # still names a file at that path, and the real script keeps resolving its
  # own root through `dirname $BASH_SOURCE/..` because wrapProgram leaves the
  # hidden original in the same bin/.
  mkPluginWith =
    { runtimeInputs ? [ ] }:
    name:
    pkgs.runCommandLocal "drip-plugin-${name}"
      {
        nativeBuildInputs = lib.optional (runtimeInputs != [ ]) pkgs.makeWrapper;
      }
      ''
        cp -R ${pluginSrc name} $out
        chmod -R u+w $out
        ${lib.concatStrings (
          lib.mapAttrsToList (destination: source: "ln -s ${source} $out/${destination}\n") (
            buildOutputs.${name} or { }
          )
        )}
        ${lib.optionalString (runtimeInputs != [ ]) ''
          for command in "$out"/bin/*; do
            [ -f "$command" ] && [ -x "$command" ] || continue
            wrapProgram "$command" --prefix PATH : ${lib.makeBinPath runtimeInputs}
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
