// ---------------------------------------------------------------------------
// drip hardcore plugin: name presets in the rename modal.
//
// Appended to src/app/state.rs by nix/herdr-patches.nix -- state.rs and not
// ui/dialogs.rs because BOTH sides need this: `ui::dialogs` draws the cells and
// `app::input::mouse` hit-tests them. `app::state` is a `pub mod` while
// `app::input` is private to `app` and `ui::dialogs` is private to `ui`, so
// state.rs is the only one of the three either side can name. Same reasoning
// as context-menu-items.rs and agent-scope-icon.rs.
//
// This half owns the VOCABULARY (the twelve names) and the GEOMETRY (where
// their cells are, and where the action row moved to). The drawing half is
// nix/rename-presets-render.rs. Geometry lives with the vocabulary rather than
// with the renderer so that the cells you can CLICK are computed by the same
// function that decides the cells that get DRAWN -- a click landing one cell
// off what the pointer is over is the failure mode this whole file is shaped to
// make impossible, and it is the one the sidebar patches learned the hard way.
// A grid only sharpens that: a one-column arithmetic difference between the two
// sides is now a click on the neighbour, not a near miss.
// ---------------------------------------------------------------------------

/// The presets, in the order they are drawn: left to right, then down.
///
/// The glyph is part of the NAME, not decoration next to it: clicking a cell
/// applies the string verbatim, so what lands on the pane, tab or space is
/// `<magnifier> recon` and the glyph is what you then read in the sidebar and
/// the tab bar. That is the point of a preset -- a dozen kinds of work, each
/// with a mark you recognise at a glance in a 26-column sidebar.
///
/// Six characters is the ceiling on the word, and that is geometry rather than
/// taste: four cells across a 56-column modal is thirteen columns each, and
/// glyph plus space plus six characters is eight of them. `orchestration` was
/// affordable when a preset owned a whole row and nothing else; beside three
/// others it is not, and `orch` is what the tab bar was truncating it to
/// anyway. The short name is also the better name once it is a label you read
/// in a sidebar forty times a day rather than a sentence you read once here.
///
/// All twelve are Codicons from the U+EA60-U+EBEB block, taken from upstream
/// nerd-fonts `glyphnames.json` (3.5.0) rather than from memory -- every one of
/// the names below exists in that file under exactly the key its comment gives,
/// so nothing here is a near miss for a codepoint that means something else.
/// One family, so the grid shares a weight; and deliberately NOT the Material
/// Design plane sidebar-version's drop lives in, whose codepoints moved
/// wholesale between Nerd Fonts v2 and v3 and draw tofu on an older patched
/// font. Every one is single width (`unicode_width` reports 1 across the BMP
/// private use area), so a name is exactly two columns wider than its word --
/// which is what makes eight the width of the widest cell contents.
///
/// A font without the block renders twelve boxes and twelve cells that still
/// read, because the word is right there. Nothing here depends on the glyph.
///
/// Why these twelve, and why baked in: they are the kinds of pane this drip's
/// hosts actually open. One coordinator driving workers and the workers doing
/// the feature; a read-only look at code nobody is changing yet and a throwaway
/// experiment that will be thrown away; something watching a log, and something
/// reading a diff before it lands. Then the four that a six-row list could not
/// afford and a grid can: chasing one failure, working the board down, writing
/// the prose rather than the code, putting it on a machine, and measuring what
/// happened when you did. And `misc`, because a fixed list that pretends to be
/// exhaustive makes you fight it. A configurable list is a config surface, a
/// parser and a reload path for a list that has not changed in the time it took
/// to want it; when it does change, it changes here, in one commit, for every
/// host at once. That is what the rest of the drip does with its opinions.
pub(crate) const DRIP_RENAME_PRESETS: [&str; 12] = [
    "\u{ebb9} orch",   // cod-type_hierarchy -- one node, and what hangs off it
    "\u{eac4} impl",   // cod-code
    "\u{ea6d} recon",  // cod-search
    "\u{ea79} spike",  // cod-beaker
    "\u{eb31} watch",  // cod-pulse
    "\u{eab3} review", // cod-checklist
    "\u{eaaf} debug",  // cod-bug
    "\u{eb09} triage", // cod-inbox -- the queue, not one item in it
    "\u{eaa4} docs",   // cod-book
    "\u{eb44} deploy", // cod-rocket
    "\u{eacd} bench",  // cod-dashboard -- the gauge, the nearest Codicon to a measurement
    "\u{ea7c} misc",   // cod-ellipsis -- the cell that says "one of the others"
];

/// How many there are, spelled once. Every array below is this long, and the
/// renderer and the hit test both walk it — a thirteenth preset is its line in
/// the table above and that table's length, and nothing else: it takes the free
/// cell on the last row, and a fourteenth starts a row and grows the modal by
/// one, without a number below here being touched.
const DRIP_PRESET_COUNT: usize = DRIP_RENAME_PRESETS.len();

/// How many cells across.
///
/// Four because the modal is 56 columns -- 54 inside the border, less two of
/// lead, is 52, which is four thirteens exactly. Three would waste a third of
/// every row on a word that is at most eight columns; five would put the widest
/// name a column from its neighbour and make the grid read as a sentence.
const DRIP_PRESET_COLS: u16 = 4;

/// How many rows that makes, rounded up. Derived rather than written down,
/// because the modal's height is derived from it in turn.
///
/// Spelled as the old round-up idiom rather than `div_ceil`, which reads better
/// but is only usable in a `const` on a new enough rustc; this is a patch
/// applied to someone else's crate and it should not be the thing that decides
/// their toolchain floor.
const DRIP_PRESET_ROWS: u16 = (DRIP_PRESET_COUNT as u16 + DRIP_PRESET_COLS - 1) / DRIP_PRESET_COLS;

/// The first preset row, as an offset from the top of the modal's inner rect.
/// Stock's own vertical layout is header / blank / input, so 3 is the rule and
/// 4 is the first row of cells.
const DRIP_PRESET_TOP: u16 = 4;

/// The rename modal's popup height, presets included. Stock is 7.
///
/// A CONSTANT and not two literals, because this number is read in two places
/// that must agree to the row: `render_rename_overlay` sizes the popup it
/// draws, and `rename_modal_inner` sizes the popup it hit-tests. Stock had
/// them as a `56, 7` pair in each file; the patch points both at this, so the
/// two cannot drift apart in a later edit.
///
/// DERIVED from the row count and not from a literal, which is the whole point
/// of the grid: twelve presets four across is three rows where six presets one
/// across was six, so this went from 14 to 11 without anyone doing the
/// arithmetic. The terms are the border's two rows, the four rows above the
/// first cell (`DRIP_PRESET_TOP` -- header, blank, input, rule), the cells
/// themselves, and the blank row plus the action row under them.
pub(crate) const DRIP_RENAME_POPUP_HEIGHT: u16 = 2 + DRIP_PRESET_TOP + DRIP_PRESET_ROWS + 2;

/// The inner height that popup gives us, once the border takes its two rows.
/// Below it there are no preset cells at all -- see `drip_rename_preset_rects`.
const DRIP_RENAME_INNER_MIN: u16 = DRIP_RENAME_POPUP_HEIGHT - 2;

/// The columns of lead before the first cell, so the leftmost glyph sits clear
/// of the modal's border the way the name field above it does. It is GEOMETRY
/// and not a couple of spaces in the renderer's format string: the hit test has
/// to know that the first two columns of a preset row belong to no cell, and
/// the only way it can know that is for the rects to say so.
const DRIP_PRESET_LEAD: u16 = 2;

/// The narrowest a cell may be before the grid is not drawn at all: a glyph, a
/// space, and the six characters the table allows a name.
///
/// This is the too-NARROW guard, and it is new with the grid -- a full-width
/// row fit in whatever the modal had, and four cells do not.
/// `centered_popup_rect` clamps the popup to `area.width - 4`, so a terminal
/// under 40 columns hands us an inner rect that cannot hold four eight-column
/// cells, and the honest answer there is the same one the too-short case gives:
/// no cells drawn, no cells clickable, stock's dialog.
const DRIP_PRESET_CELL_MIN: u16 = 8;

/// The band at the bottom of the modal that the action row is placed inside.
///
/// Four rows because `centered_button_row` puts the buttons at `row_offset`
/// from the top of whatever rect it is handed, `rename_button_rects` passes 3,
/// and 3 is the last row of a 4-row band. So handing it the bottom four rows
/// of `inner` moves the buttons to the bottom of the modal without touching
/// stock's offset -- one anchor at each of the two call sites instead of a
/// multi-line rewrite of the function they share.
const DRIP_RENAME_ACTION_BAND: u16 = 4;

/// Where the twelve preset cells are, or `None` when the modal is too small to
/// have them.
///
/// The grid is row-major: cell `idx` is at column `idx % DRIP_PRESET_COLS` of
/// row `idx / DRIP_PRESET_COLS`, which is the order `DRIP_RENAME_PRESETS` is
/// written in and the order it is read in. Returning ONE array indexed the same
/// way that table is indexed is what lets the renderer `.zip()` it and the hit
/// test `.find()` in it without either of them knowing the grid's shape.
///
/// Cells are as wide as the modal allows rather than a fixed thirteen: the
/// width left after the lead, divided four ways. At the modal's full 56 that is
/// 13 and the arithmetic is the same as a constant would give; clamped
/// narrower it shrinks the cells evenly instead of overlapping them or running
/// the last one off the edge. Any remainder from that division -- at most three
/// columns, at the right edge -- is left dead rather than given to the last
/// cell, so that every cell in a row is the same size and the column a word
/// starts in is `lead + col * width` on both sides of this file.
///
/// Too small is a real case and not a paranoia: `centered_popup_rect` clamps
/// the popup to the screen, so on a terminal under 13 rows or under 40 columns
/// the modal arrives smaller than it asked for. The renderer and the hit test
/// BOTH go through here, so a squeezed modal draws no presets and accepts no
/// clicks on the cells it did not draw -- it degrades to stock's shape (title,
/// input, buttons) rather than to a dialog with invisible live regions in it.
pub(crate) fn drip_rename_preset_rects(inner: Rect) -> Option<[Rect; DRIP_PRESET_COUNT]> {
    if inner.height < DRIP_RENAME_INNER_MIN {
        return None;
    }
    let cell = inner.width.saturating_sub(DRIP_PRESET_LEAD) / DRIP_PRESET_COLS;
    if cell < DRIP_PRESET_CELL_MIN {
        return None;
    }
    Some(std::array::from_fn(|idx| {
        let col = idx as u16 % DRIP_PRESET_COLS;
        let row = idx as u16 / DRIP_PRESET_COLS;
        Rect::new(
            inner
                .x
                .saturating_add(DRIP_PRESET_LEAD)
                .saturating_add(col.saturating_mul(cell)),
            inner.y.saturating_add(DRIP_PRESET_TOP).saturating_add(row),
            cell,
            1,
        )
    }))
}

/// The rule row above the presets -- the same row the renderer draws its
/// `presets` caption on. Derived from the preset rects rather than from
/// `DRIP_PRESET_TOP` again, so it cannot end up one row away from them. It
/// spans the full inner width and not the grid's, because it is a rule under
/// the input field and the input field is the width of the modal.
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
/// The whole CELL is the target and not just the columns the word occupies,
/// which is the same rule the full-width rows had and matters more now that a
/// cell is thirteen columns around an eight-column word. The five columns after
/// `orch` belong to `orch`: they are nearer it than anything else, there is
/// nothing else in them, and a click that misses a word by two columns meant
/// the word it was aiming at. Making them dead instead would mean a third of
/// the grid did nothing, which is exactly the frustration the whole-row rule
/// was written to avoid.
///
/// The two columns of lead at the left edge are NOT in any cell, and that is
/// deliberate: they are the modal's gutter, the same gutter every other row of
/// this dialog keeps clear, and a click in it is a click on the dialog rather
/// than on its first column. Missing the cells still means what it meant in
/// stock herdr -- `rename_button_rects`'s caller falls through to
/// `ModalAction::Cancel`.
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
/// clickable buttons the same cells. It works from the modal's own floor rather
/// than from the preset rects on purpose: the buttons exist at every size, so
/// they must not vanish with the grid on a modal too small to hold one. On a
/// modal too short to hold the band the `min` collapses it to what there is, so
/// the band starts at the modal's top and the buttons land on its last row --
/// the bottom of whatever the popup was clamped to, which is where they belong
/// on a short modal too. Both callers read this one function, so drawn and
/// clickable stay identical at every height, the same graceful nothing
/// `drip_rename_preset_rects` does one function up.
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
