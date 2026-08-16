// ---------------------------------------------------------------------------
// drip hardcore plugin: name presets in the rename modal.
//
// Appended to src/app/state.rs by nix/herdr-patches.nix -- state.rs and not
// ui/dialogs.rs because BOTH sides need this: `ui::dialogs` draws the rows and
// `app::input::mouse` hit-tests them. `app::state` is a `pub mod` while
// `app::input` is private to `app` and `ui::dialogs` is private to `ui`, so
// state.rs is the only one of the three either side can name. Same reasoning
// as context-menu-items.rs and agent-scope-icon.rs.
//
// This half owns the VOCABULARY (the six names) and the GEOMETRY (where their
// rows are, and where the action row moved to). The drawing half is
// nix/rename-presets-render.rs. Geometry lives with the vocabulary rather than
// with the renderer so that the rows you can CLICK are computed by the same
// function that decides the rows that get DRAWN -- a click landing one row off
// what the pointer is over is the failure mode this whole file is shaped to
// make impossible, and it is the one the sidebar patches learned the hard way.
// ---------------------------------------------------------------------------

/// The presets, in the order they are drawn, glyph included.
///
/// The glyph is part of the NAME, not decoration next to it: clicking a row
/// applies the string verbatim, so what lands on the pane, tab or space is
/// `<magnifier> research` and the glyph is what you then read in the sidebar
/// and the tab bar. That is the point of a preset -- six kinds of work, each
/// with a mark you recognise at a glance in a 26-column sidebar.
///
/// All six are Codicons from the U+EA60-U+EBEB block, taken from upstream
/// nerd-fonts `glyphnames.json` (3.5.0) rather than from memory. One family,
/// so the column shares a weight; and deliberately NOT the Material Design
/// plane sidebar-version's drop lives in, whose codepoints moved wholesale
/// between Nerd Fonts v2 and v3 and draw tofu on an older patched font. Every
/// one is single width (`unicode_width` reports 1 across the BMP private use
/// area), so a name is exactly two columns wider than its word -- which is
/// what keeps these fitting where a bare label already fit.
///
/// A font without the block renders six boxes and six rows that still read,
/// because the word is right there. Nothing here depends on the glyph.
///
/// Why these six, and why baked in: they are the kinds of pane this drip's
/// hosts actually open -- one coordinator driving workers, feature work in a
/// worktree, a read-only look, a throwaway experiment, something watching a
/// log, and everything else. A configurable list is a config surface, a parser
/// and a reload path for a list that has not changed in the time it took to
/// want it; when it does change, it changes here, in one commit, for every
/// host at once. That is what the rest of the drip does with its opinions.
pub(crate) const DRIP_RENAME_PRESETS: [&str; 6] = [
    "\u{ebb9} orchestration",  // cod-type_hierarchy -- one node, and what hangs off it
    "\u{eac4} implementation", // cod-code
    "\u{ea6d} research",       // cod-search
    "\u{ea79} spike",          // cod-beaker
    "\u{eb31} monitor",        // cod-pulse
    "\u{ea7c} misc",           // cod-ellipsis -- the row that says "one of the others"
];

/// How many there are, spelled once. Every array below is this long, and the
/// renderer and the hit test both walk it — a seventh preset is one line in
/// the table above and one row taller a modal, and nothing else.
const DRIP_PRESET_COUNT: usize = DRIP_RENAME_PRESETS.len();

/// The rename modal's popup height, presets included. Stock is 7.
///
/// A CONSTANT and not two literals, because this number is read in two places
/// that must agree to the row: `render_rename_overlay` sizes the popup it
/// draws, and `rename_modal_inner` sizes the popup it hit-tests. Stock had
/// them as a `56, 7` pair in each file; the patch points both at this, so the
/// two cannot drift apart in a later edit.
///
/// 14 = the stock 7, plus the rule under the input, plus six preset rows, plus
/// the blank row above the action row.
pub(crate) const DRIP_RENAME_POPUP_HEIGHT: u16 = 14;

/// The inner height that popup gives us, once the border takes its two rows.
/// Below it there are no preset rows at all -- see `drip_rename_preset_rects`.
const DRIP_RENAME_INNER_MIN: u16 = DRIP_RENAME_POPUP_HEIGHT - 2;

/// The first preset row, as an offset from the top of the modal's inner rect.
/// Stock's own vertical layout is header / blank / input, so 3 is the rule and
/// 4 is the first name.
const DRIP_PRESET_TOP: u16 = 4;

/// The band at the bottom of the modal that the action row is placed inside.
///
/// Four rows because `centered_button_row` puts the buttons at `row_offset`
/// from the top of whatever rect it is handed, `rename_button_rects` passes 3,
/// and 3 is the last row of a 4-row band. So handing it the bottom four rows
/// of `inner` moves the buttons to the bottom of the modal without touching
/// stock's offset -- one anchor at each of the two call sites instead of a
/// multi-line rewrite of the function they share.
const DRIP_RENAME_ACTION_BAND: u16 = 4;

/// Where the six preset rows are, or `None` when the modal is too short to
/// have them.
///
/// Too short is a real case and not a paranoia: `centered_popup_rect` clamps
/// the popup to the screen, so on a terminal under 16 rows the modal arrives
/// smaller than it asked for. The renderer and the hit test BOTH go through
/// here, so a squeezed modal draws no presets and accepts no clicks on the
/// rows it did not draw -- it degrades to stock's shape (title, input,
/// buttons) rather than to a dialog with invisible live regions in it.
pub(crate) fn drip_rename_preset_rects(inner: Rect) -> Option<[Rect; DRIP_PRESET_COUNT]> {
    if inner.height < DRIP_RENAME_INNER_MIN || inner.width == 0 {
        return None;
    }
    Some(std::array::from_fn(|idx| {
        Rect::new(
            inner.x,
            inner.y.saturating_add(DRIP_PRESET_TOP).saturating_add(idx as u16),
            inner.width,
            1,
        )
    }))
}

/// The rule row above the presets -- the same row the renderer draws its
/// `presets` caption on. Derived from the preset rects rather than from
/// `DRIP_PRESET_TOP` again, so it cannot end up one row away from them.
pub(crate) fn drip_rename_rule_row(inner: Rect) -> Option<Rect> {
    let rects = drip_rename_preset_rects(inner)?;
    Some(Rect::new(
        inner.x,
        rects[0].y.saturating_sub(1),
        inner.width,
        1,
    ))
}

/// The preset under the pointer, if the pointer is on one.
///
/// A whole row is the target, edge to edge, not just the columns the text
/// happens to occupy: the rows are the only thing in that band, so a click
/// that misses the word by two columns meant the row it landed in and nothing
/// else. Missing them entirely still means what it meant in stock herdr --
/// `rename_button_rects`'s caller falls through to `ModalAction::Cancel`.
pub(crate) fn drip_rename_preset_at(inner: Rect, column: u16, row: u16) -> Option<&'static str> {
    let rects = drip_rename_preset_rects(inner)?;
    rects
        .iter()
        .zip(DRIP_RENAME_PRESETS)
        .find(|(rect, _)| row == rect.y && column >= rect.x && column < rect.right())
        .map(|(_, preset)| preset)
}

/// The rect to hand `rename_button_rects` so the action row sits at the bottom
/// of the modal instead of three rows under the title.
///
/// Both callers of that function -- the renderer and the mouse hit test -- are
/// patched to go through here, which is what keeps the drawn buttons and the
/// clickable buttons the same cells. On a modal too short to hold the band the
/// `min` collapses it to what there is, so the band starts at the modal's top
/// and the buttons land on its last row -- the bottom of whatever the popup
/// was clamped to, which is where they belong on a short modal too. Both
/// callers read this one function, so drawn and clickable stay identical at
/// every height, the same graceful nothing `drip_rename_preset_rects` does one
/// function up.
pub(crate) fn drip_rename_button_area(inner: Rect) -> Rect {
    let height = inner.height.min(DRIP_RENAME_ACTION_BAND);
    Rect::new(
        inner.x,
        inner
            .y
            .saturating_add(inner.height.saturating_sub(height)),
        inner.width,
        height,
    )
}
