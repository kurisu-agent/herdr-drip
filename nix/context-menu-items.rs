// context-menu-items: what herdr's right-click menus SAY, in what order, and
// which glyph each row carries. Appended to src/app/state.rs by
// nix/herdr-patches.nix; the drawing half is nix/context-menu-render.rs.
//
// It lives here because `app::state` is the module both halves can name.
// `mod app` and `mod ui` are private at the crate root, which makes them
// visible crate-WIDE (every module is a descendant of the root), so
// `crate::app::state::…` resolves from `ui::menus` and from the dispatcher in
// `app::input::modal` alike — while a const defined in either of those two
// would be reachable from neither, since `app::input` and `ui::menus` are
// private to their own parents.
//
// The pane menu's shape is `drip_pane_menu` below. Stock herdr builds its list
// and we map it to ours: that way the CONDITIONAL items stay herdr's decision
// (a swap is only offered mid-swap, the passthrough exit only while a pane is
// in passthrough) and we only decide order, wording and what to drop. It is
// also one anchor instead of unpicking four pushes.

/// A non-selectable rule between groups. Renders as a full-width line and is
/// skipped by both `drip_menu_move` (keyboard) and the patched
/// `context_menu_item_at` (mouse), so no gesture can land on one — which is
/// the whole difference between a separator and a dead menu entry.
pub(crate) const DRIP_MENU_SEPARATOR: &str = "─";

// The pane menu, in the order it is drawn. Three creations, then the things
// that rearrange what already exists, then the four ways to split, then the way
// out.
//
// `New Agent` sits under `New Space` because it IS a new space, with its cwd
// and its pane's kind pinned rather than inherited — see
// nix/pane-menu-new-agent.rs. Third rather than first: the two above it are
// where you go to work, and this is the one you take a thought to.
pub(crate) const DRIP_NEW_TAB: &str = "New Tab";
pub(crate) const DRIP_NEW_SPACE: &str = "New Space";
pub(crate) const DRIP_NEW_AGENT: &str = "New Agent";
pub(crate) const DRIP_AGENT_RIGHT: &str = "Agent Right";
pub(crate) const DRIP_AGENT_DOWN: &str = "Agent Down";
pub(crate) const DRIP_SHELL_RIGHT: &str = "Shell Right";
pub(crate) const DRIP_SHELL_DOWN: &str = "Shell Down";
pub(crate) const DRIP_CLOSE: &str = "Close";

// Stock strings we do not rename but do reorder, glyph, or test for. Spelled
// as consts so the ordering below and the glyph table read the same way for
// every row, and so a herdr bump that reworded one is a mismatch HERE rather
// than a row that quietly loses its icon.
pub(crate) const DRIP_ZOOM: &str = "Zoom";
pub(crate) const DRIP_SWAP: &str = "Swap with focused pane";
pub(crate) const DRIP_PASSTHROUGH_EXIT: &str = "Use Herdr right-click menu";

/// The pane menu, built from the list stock herdr just assembled.
///
/// Everything not named here is dropped, which is where `pane-menu-trim` went:
/// `Rename pane`, `Clear pane name` and `Send right-clicks to pane` are absent
/// because they are not in this function, not because a later pass removes
/// them. The two conditional rows are carried through only if stock offered
/// them.
pub(crate) fn drip_pane_menu(stock: Vec<&'static str>) -> Vec<&'static str> {
    let offered = |item: &str| stock.iter().any(|candidate| *candidate == item);

    let mut items = vec![
        DRIP_NEW_TAB,
        DRIP_NEW_SPACE,
        DRIP_NEW_AGENT,
        DRIP_MENU_SEPARATOR,
        DRIP_ZOOM,
    ];
    if offered(DRIP_SWAP) {
        items.push(DRIP_SWAP);
    }
    items.push(DRIP_MENU_SEPARATOR);
    items.extend([
        DRIP_AGENT_RIGHT,
        DRIP_AGENT_DOWN,
        DRIP_SHELL_RIGHT,
        DRIP_SHELL_DOWN,
    ]);
    items.push(DRIP_MENU_SEPARATOR);
    if offered(DRIP_PASSTHROUGH_EXIT) {
        items.push(DRIP_PASSTHROUGH_EXIT);
    }
    items.push(DRIP_CLOSE);
    items
}

/// The glyph for one menu row, or None for a row that gets none.
///
/// All Codicons, from the U+EA60–U+EBEB block every Nerd Font since v2.3
/// carries. One family, so the column shares a weight — and off the Material
/// Design plane (U+F0000+), whose codepoints moved wholesale between Nerd Fonts
/// v2 and v3 and would draw boxes on a v2 font. Every glyph below was taken
/// from upstream's glyphnames.json, not from memory.
///
/// The four splits take their glyph from the DIRECTION, because the label
/// already says the kind: each row is then named twice over, once by the word
/// `Agent`/`Shell` at the left edge and once by the split icon at the right,
/// and neither repeats the other. A font missing the block leaves four boxes
/// and four rows that still read.
///
/// Rows from the OTHER context menus (workspace, git workspace, tab) are here
/// too — the renderer is shared, and an icon column that appears on one menu
/// and not the next reads as a bug rather than as a choice.
pub(crate) fn drip_menu_glyph(item: &str) -> Option<&'static str> {
    Some(match item {
        DRIP_NEW_TAB | "New tab" => "\u{eb23}",       // cod-multiple_windows
        DRIP_NEW_SPACE => "\u{eb7f}",                 // cod-window
        DRIP_NEW_AGENT => "\u{eb08}",                 // cod-hubot
        DRIP_ZOOM => "\u{eb4c}",                      // cod-screen_full
        DRIP_SWAP => "\u{ebcb}",                      // cod-arrow_swap
        DRIP_AGENT_RIGHT | DRIP_SHELL_RIGHT => "\u{eb56}", // cod-split_horizontal
        DRIP_AGENT_DOWN | DRIP_SHELL_DOWN => "\u{eb57}",   // cod-split_vertical
        DRIP_PASSTHROUGH_EXIT => "\u{eb94}",          // cod-menu
        // DRIP_CLOSE is "Close", which is also the workspace and tab menus'
        // own close row — one arm covers all three.
        DRIP_CLOSE | "Close group" => "\u{ea76}", // cod-close
        "Rename" => "\u{ea73}",                       // cod-edit
        "New worktree" => "\u{ea80}",                 // cod-new_folder
        "Open worktree..." => "\u{eaf7}",             // cod-folder_opened
        "Delete worktree checkout..." => "\u{ea81}",  // cod-trash
        "Expand" => "\u{eab4}",                       // cod-chevron_down
        "Collapse" => "\u{eab7}",                     // cod-chevron_up
        _ => return None,
    })
}

/// Move the highlight by one row, stepping OVER separators.
///
/// Replaces `menu.list.move_prev()` / `move_next(len)` at the two live key
/// handlers. `MenuListState` is shared with menus that have no separators and
/// knows nothing about items, so the skip has to be here, where the list is.
/// Running off either end leaves the highlight where it was, which is what the
/// stock saturating/clamping does — a run of separators at the end of a list
/// can never strand it.
pub(crate) fn drip_menu_move(menu: &mut ContextMenuState, delta: isize) {
    let items = menu.items();
    let mut idx = menu.list.highlighted as isize;
    loop {
        let next = idx + delta;
        if next < 0 || next as usize >= items.len() {
            return;
        }
        idx = next;
        if items[idx as usize] != DRIP_MENU_SEPARATOR {
            menu.list.highlighted = idx as usize;
            return;
        }
    }
}
