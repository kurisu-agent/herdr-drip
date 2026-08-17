// ---------------------------------------------------------------------------
// drip hardcore plugin: a tab says how many panes it is holding.
//
// Appended to src/ui/tabs.rs by nix/herdr-patches.nix. The tab bar tells you
// what a space's tabs are CALLED and nothing about what is inside them, so a
// tab holding one agent and a tab holding four look identical until you switch
// to it -- and with the drip's shell-panes patch, "how many panes" is exactly
// the question of how much work is parked in there.
//
// `tab_chrome_label` is the single place a tab bar label is composed, and both
// consumers go through it: the renderer that centres the text in the cell, and
// `tab_width`, which sizes the cell from `display_width_u16` of the same
// string. So a marker added here is a marker the strip has already made room
// for -- no layout code is touched, and herdr's own
// `zoom_marker_counts_toward_tab_width` test is the proof that this is how the
// existing marker rides too.
//
// No plugin surface reaches it. A tab label is `custom_name` or the tab
// number, composed in the renderer; the plugin API's text surfaces are the
// sidebar's per-row tokens and the tab bar's RIGHT-hand status segments
// (`tab_bar_right`), which are global to the bar, not per tab. Nothing in it
// can add a character to a tab's own cell.
// ---------------------------------------------------------------------------

/// True for a codepoint in any of Unicode's three Private Use Areas: the BMP's
/// U+E000..U+F8FF and the two supplementary planes.
///
/// That is the test for "this name starts with an icon", and it is the honest
/// one: every glyph this drip puts at the head of a name is a Nerd Font glyph,
/// and Nerd Fonts live in exactly these ranges by construction -- the rename
/// presets' Codicons in the BMP block (U+EA60..U+EBEB, see rename-presets.rs)
/// and Material Design Icons, which Nerd Fonts v3 moved wholesale into plane 15
/// (U+F0001.., the plane sidebar-version's own drop glyph sits in). Testing the
/// AREAS rather than listing our blocks means a plugin whose manifest title
/// leads with some other Nerd Font glyph is treated the same as ours, which is
/// the point: this splices names it did not write.
///
/// It cannot false-positive on a name someone typed, because there is no
/// keyboard for the private use area -- a codepoint gets there by being pasted
/// out of a font's glyph table.
fn drip_is_private_use(c: char) -> bool {
    matches!(c, '\u{e000}'..='\u{f8ff}' | '\u{f0000}'..='\u{ffffd}' | '\u{100000}'..='\u{10fffd}')
}

/// Split a tab display name into its leading icon and the rest, when it has
/// one: `<icon> implementation` -> `("<icon>", "implementation")`.
///
/// The shape being matched is the one both of our sources produce -- a single
/// private-use glyph, one space, then a word: the rename presets
/// (rename-presets.rs) and the plugin manifests' pane titles
/// (`beads/herdr-plugin.toml`'s `<icon> beads`). Anything else answers None and
/// is treated as an unadorned name, including a name that is nothing BUT a
/// glyph, which neither source can produce and which has no "after the icon"
/// for a count to sit in.
fn drip_tab_name_icon_split(name: &str) -> Option<(&str, &str)> {
    let mut chars = name.char_indices();
    let (_, icon) = chars.next()?;
    if !drip_is_private_use(icon) {
        return None;
    }
    let (space_idx, second) = chars.next()?;
    if second != ' ' {
        return None;
    }
    let rest = name.get(space_idx + 1..).filter(|rest| !rest.is_empty())?;
    Some((&name[..space_idx], rest))
}

/// `name` with the tab's pane count spliced in, or `name` untouched when the
/// tab holds a single pane.
///
/// WHICH COUNT. `tab.layout.pane_count()`, not `tab.panes.len()`. The two
/// agree by invariant: `Workspace::assert_invariants_for_test` asserts, per
/// tab, that the layout's pane set and the pane-state map's key set are EQUAL
/// ("layout panes must exactly match pane states"). That is a test-time
/// assertion rather than a runtime one, but it is upstream stating the two are
/// one set -- so this is not a correctness call between them but a "which
/// structure is this label describing" one. The layout tree is what the tab bar is a label for: it is
/// what the renderer splits into cells, what herdr's own chrome decisions read
/// (`ui/panes.rs` sizes `multi_pane` off it) and what its close paths ask
/// before deciding a tab is empty. `panes` is a `HashMap<PaneId, PaneState>` --
/// the right answer to "which panes", and only incidentally to "how many".
///
/// Zooming does not change either: `Tab::zoomed` is a flag beside the layout,
/// not a layout with one pane in it, so a zoomed tab still counts every pane it
/// is hiding. That is the point (see the zoom note below).
///
/// Panes that are not in a tab's layout are not in the count, and should not
/// be: overlay and plugin panes are `AppState`-level, drawn over whichever tab
/// is on screen and owned by none of them.
///
/// WHERE IT GOES. After the icon and before the word -- `<icon> ·3 impl` -- not
/// at the end of the label. The icon is the first thing the eye lands on in a
/// strip of tabs, so the count is the second, and the name keeps the tail of
/// the cell where a truncating label loses characters anyway. It is spliced
/// INTO the name rather than composed beside it because the icon is not a
/// field: `tab_display_name` hands back one string that happens to start with
/// a glyph, whether it came from a rename preset or a plugin's manifest title.
///
/// With no leading icon -- a name someone typed, or the `tab_idx + 1` fallback
/// for a tab nobody has named -- the count LEADS, which is the same rule:
/// count, then name. Nothing is inserted in the middle of a word to do it.
///
/// WHAT IT LOOKS LIKE. `·` (U+00B7 MIDDLE DOT) then the decimal count. Three
/// constraints picked it:
///
///   - It cannot be read as part of the name. The fallback name for an unnamed
///     tab IS a number, so a bare count in front of it would render tab 3 with
///     two panes as `2 3` -- a label that is entirely digits and says nothing
///     twice. The dot is what makes `·2 3` legible, and it costs one column.
///   - It cannot be read as a CONTROL. `×2` says "two of them" more naturally
///     and was rejected for it: `×` in a tab bar is the close button in every
///     GUI anyone using this has ever used, and herdr's tab cells ARE clickable
///     (`tab_hit_areas`). A marker that invites a click that closes nothing is
///     worse than a quiet one.
///   - It has to be one column in any font. U+00B7 is Latin-1: present
///     everywhere, including terminal fonts with no Nerd Font patch, where the
///     name's own icon is already a box. Superscript digits (`³`) were the
///     other quiet option and lose on exactly this -- U+2070..U+2079 is patchy
///     in fixed-width fonts and gets worse the moment the count reaches two
///     digits.
///
/// ONE PANE SAYS NOTHING, deliberately, and the reason is horizontal space --
/// the scarce thing in a strip that scrolls once the tabs overflow it. It is
/// the same call `hide_tab_bar_when_single_tab` makes in config/herdr.toml:
/// one of a thing is not a choice, so the columns spent announcing it are
/// spent on nothing. A `·1` on every tab in the common case would cost three
/// columns per tab to say "normal".
///
/// AND THE ZOOM MARKER. Both markers land on this one label and a tab can be
/// zoomed AND hold several panes, so they are given different ENDS of it rather
/// than a shared suffix whose order someone has to remember: the count rides
/// with the icon at the head, `Z` stays the last thing in the label, exactly
/// where a reader who has been scanning tab ends for it already looks. Stock's
/// `format!("{name} Z")` is left composing that end untouched, so the two
/// cannot collide -- `<icon> ·3 impl Z` is the count, the name and the zoom in
/// the order they became true. The pairing is also the case where the count
/// matters most: zoom is the state in which the other panes are invisible.
fn drip_tab_pane_count_label(
    ws: &crate::workspace::Workspace,
    tab_idx: usize,
    name: String,
) -> String {
    let Some(count) = ws
        .tabs
        .get(tab_idx)
        .map(|tab| tab.layout.pane_count())
        .filter(|count| *count > 1)
    else {
        return name;
    };
    match drip_tab_name_icon_split(&name) {
        Some((icon, rest)) => format!("{icon} ·{count} {rest}"),
        None => format!("·{count} {name}"),
    }
}
