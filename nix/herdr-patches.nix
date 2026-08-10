# Hardcore plugins — the drip's source patches on herdr itself.
#
# A regular plugin lives in a top-level directory here and talks to herdr
# through its plugin surface. A HARDCORE plugin is for the things herdr has
# no surface for at all: it patches herdr's source and rides the build. Same
# curation idea as the rest of the drip — our opinions about how herdr
# should behave, in one repo — just applied one layer down.
#
# Rules of the set:
#   - One function over the herdr package, so every consumer (the
#     nix-claude-drip herdr knob, a host pinning its own herdr build)
#     applies the identical set: `herdr-drip.lib.patchHerdr herdrPkg`.
#   - Every patch FAILS LOUDLY when upstream moves: `substituteInPlace
#     --replace-fail` (or a context patch) errors the build rather than
#     silently no-opping, so a herdr bump can never shed a patch without
#     someone reading why.
#   - Each patch carries its story: what it changes, and why it cannot be a
#     real plugin. When herdr grows a surface for one, the patch graduates
#     into a plugin directory and leaves this file.
#   - Applying the set twice is a build error by construction (the
#     --replace-fail no longer matches) — a host overriding
#     `services.claude-code.herdr.package` supplies an UNPATCHED build and
#     lets the module patch it.
herdrPkg:
herdrPkg.overrideAttrs (old: {
  postPatch = (old.postPatch or "") + ''
    # sidebar-version: the sidebar's workspace-list header hardcodes the
    # label " spaces" — pure redundancy over a list that is visibly spaces.
    # Herdr has no plugin surface for sidebar chrome, so render the running
    # herdr version there instead, which the UI otherwise shows nowhere.
    substituteInPlace src/ui/sidebar.rs \
      --replace-fail '" spaces",' 'concat!(" herdr ", env!("CARGO_PKG_VERSION")),'
  '';
})
