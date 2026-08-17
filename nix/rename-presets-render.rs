// rename-presets-render: the twelve preset cells, drawn under the name field
// four to a row. Appended to src/ui/dialogs.rs by nix/herdr-patches.nix; the
// vocabulary and the geometry are nix/rename-presets.rs.
//
// It is a separate half for the same reason context-menu-render.rs is: the
// names have to be nameable from `app::input` (the click) and this has to be
// callable from `ui::dialogs` (the draw), and no single private module is
// visible to both. What lives here is only what needs a `Frame`.
//
// Every rect it draws into comes from the geometry half, never from arithmetic
// of its own -- the hit test reads those same functions, so there is exactly
// one description of where a cell is. That was worth having when the presets
// were full-width rows and a mistake could only be off by a row; with four
// cells to a row a private column calculation here would put the click on the
// neighbouring word, which looks like a bug in the preset rather than in the
// geometry.

/// Draws the rule, its caption and the grid of preset cells into `inner`.
///
/// Silently draws nothing when the modal is too small to hold them, which is
/// the same condition under which the hit test accepts no clicks: on a terminal
/// under 13 rows or under 40 columns the dialog is stock's title/input/buttons
/// and nothing is offered that cannot be hit.
pub(super) fn drip_render_rename_presets(
    frame: &mut Frame,
    inner: Rect,
    p: &crate::app::state::Palette,
) {
    let Some(rects) = crate::app::state::drip_rename_preset_rects(inner) else {
        return;
    };

    // A rule with the word in it, rather than a bare rule: rows of unadorned
    // words under a text field read as information about the thing being
    // renamed, and these are buttons. `presets` is the shortest true caption,
    // and the modal is 56 columns -- it can afford one where the 26-column
    // sidebar could not, which is why sidebar-quiet-chrome took captions out
    // there and this puts one in here.
    //
    // Drawn in `overlay0` rather than the `surface1` the open-worktree dialog
    // rules itself in: surface1 is a divider colour, and a word in it on the
    // panel background is closer to invisible than to quiet.
    if let Some(rule) = crate::app::state::drip_rename_rule_row(inner) {
        let width = rule.width as usize;
        let caption = "\u{2500} presets ";
        let text = if width >= caption.chars().count() {
            format!(
                "{caption}{}",
                "\u{2500}".repeat(width - caption.chars().count())
            )
        } else {
            "\u{2500}".repeat(width)
        };
        frame.render_widget(
            Paragraph::new(text).style(Style::default().fg(p.overlay0)),
            rule,
        );
    }

    // The cells themselves, in `text` and not the `subtext0` a list's
    // unselected rows wear: nothing here is selected or unselected -- there is
    // no cursor in this grid and no keyboard path to one -- so dimming them
    // would say "inactive" about twelve cells that are each one click from
    // doing something.
    //
    // The name is drawn at the cell's own left edge with no lead of its own:
    // the two columns that keep the leftmost glyph clear of the modal's border
    // are in the rects now, because the hit test has to agree about who owns
    // them, and adding them again here would indent every column by two more
    // than the one to its left. Truncated to the cell rather than wrapped, for
    // the same reason herdr truncates every other row it draws: a name that ran
    // on would push its neighbour out from under its own hit rect. Nothing in
    // the table is long enough to reach that -- eight columns of contents in a
    // cell of at least eight -- so it is the guard for a preset added later
    // without counting, and for a font that renders a glyph double width.
    for (rect, preset) in rects
        .iter()
        .zip(crate::app::state::DRIP_RENAME_PRESETS)
    {
        frame.render_widget(
            Paragraph::new(truncate_end(preset, rect.width as usize))
                .style(Style::default().fg(p.text)),
            *rect,
        );
    }
}
