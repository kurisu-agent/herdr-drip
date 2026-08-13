// pane-menu-labels: the four split entries of the pane context menu, as the
// exact `&'static str`s herdr compiles into its item list.
//
// Appended to src/app/state.rs by nix/herdr-patches.nix, so they live in the
// module that BUILDS the menu — and that direction is forced: `app::state` is
// `pub mod`, while `app::input` and its `modal` are private, so the dispatcher
// can name a const defined here and not the other way round. One definition,
// three uses each: the item list, the match arm that accepts the label, and
// the `== Some(..)` that decides shell-or-agent.
//
// ORDER is direction-first — both answers for `right`, then both for `down`
// (state.rs's list is what holds it; these consts only have to exist). The
// common pair is two panes split the same way, one of each kind, so the two
// rows you choose between sit adjacent instead of two apart.
//
// GLYPHS are all Codicons, from the U+EA60–U+EBEB block every Nerd Font since
// v2.3 carries. One family, so the four rows share a weight — and off the
// Material Design plane (U+F0000+), whose codepoints moved wholesale between
// Nerd Fonts v2 and v3 and would render as boxes on a v2 font.
//
//   U+EB56  cod-split_horizontal   panes side by side  -> right
//   U+EB57  cod-split_vertical     panes stacked       -> down
//   U+EA85  cod-terminal           a shell
//   U+EB08  cod-hubot              an agent
//
// The words stay rather than letting the glyph carry the whole meaning: a
// terminal whose font lacks the block draws four boxes, and every row still
// reads. They say `right`/`down`, not horizontal/vertical, because those two
// inverted between tmux (`split -h` is side by side) and vim (`:split` is
// stacked) — the glyph is the half that cannot be read backwards.
//
// Kept short on purpose: herdr sizes the popup from `item.len()`, which is
// BYTES, so each 3-byte glyph buys ~2 columns of padding it does not need.
// That errs wide, never narrow, so it is cosmetic.
pub(crate) const DRIP_SPLIT_RIGHT_SHELL: &str = "\u{eb56} \u{ea85} Split right (shell)";
pub(crate) const DRIP_SPLIT_RIGHT_AGENT: &str = "\u{eb56} \u{eb08} Split right (agent)";
pub(crate) const DRIP_SPLIT_DOWN_SHELL: &str = "\u{eb57} \u{ea85} Split down (shell)";
pub(crate) const DRIP_SPLIT_DOWN_AGENT: &str = "\u{eb57} \u{eb08} Split down (agent)";
