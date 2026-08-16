// rename-presets-render: the six preset rows, drawn under the name field.
// Appended to src/ui/dialogs.rs by nix/herdr-patches.nix; the vocabulary and
// the geometry are nix/rename-presets.rs.
//
// It is a separate half for the same reason context-menu-render.rs is: the
// names have to be nameable from `app::input` (the click) and this has to be
// callable from `ui::dialogs` (the draw), and no single private module is
// visible to both. What lives here is only what needs a `Frame`.
//
// Every rect it draws into comes from the geometry half, never from arithmetic
// of its own -- the hit test reads those same functions, so there is exactly
// one description of where a row is.

/// Draws the rule, its caption and the six preset rows into `inner`.
///
/// Silently draws nothing when the modal is too short to hold them, which is
/// the same condition under which the hit test accepts no clicks: on a
/// terminal under 16 rows the dialog is stock's title/input/buttons and
/// nothing is offered that cannot be hit.
pub(super) fn drip_render_rename_presets(
    frame: &mut Frame,
    inner: Rect,
    p: &crate::app::state::Palette,
) {
    let Some(rects) = crate::app::state::drip_rename_preset_rects(inner) else {
        return;
    };

    // A rule with the word in it, rather than a bare rule: six unadorned rows
    // under a text field read as information about the thing being renamed,
    // and these are buttons. `presets` is the shortest true caption, and the
    // modal is 56 columns -- it can afford one where the 26-column sidebar
    // could not, which is why sidebar-quiet-chrome took captions out there and
    // this puts one in here.
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

    // The rows themselves, in `text` and not the `subtext0` a list's unselected
    // rows wear: nothing here is selected or unselected -- there is no cursor
    // in this list and no keyboard path to one -- so dimming them would say
    // "inactive" about six rows that are each one click from doing something.
    //
    // Two columns of lead so the glyph sits clear of the modal's border,
    // matching the one column of lead the name field above it draws with.
    // Truncated to the row rather than wrapped, for the same reason herdr
    // truncates every other row it draws: a name that ran onto a second line
    // would push the row under it out from under its own hit rect.
    for (rect, preset) in rects
        .iter()
        .zip(crate::app::state::DRIP_RENAME_PRESETS)
    {
        frame.render_widget(
            Paragraph::new(truncate_end(&format!("  {preset}"), rect.width as usize))
                .style(Style::default().fg(p.text)),
            *rect,
        );
    }
}
