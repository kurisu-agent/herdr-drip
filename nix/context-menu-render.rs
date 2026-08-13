// context-menu-render: one row of a right-click menu — the label at the left
// edge, its glyph at the right. Appended to src/ui/menus.rs by
// nix/herdr-patches.nix; the vocabulary half is nix/context-menu-items.rs.
//
// The alignment is why this cannot live in the item strings. Stock herdr draws
// each item as `Line::from(*item)`, so anything an item wants to say has to be
// IN the string — and a string cannot know how wide the popup it lands in will
// be. `context_menu_rect` sizes that popup from the longest item, so the pad
// between label and glyph is only knowable here, one frame at a time. Hence a
// row builder rather than four cleverer labels.
//
// Widths are counted in BYTES on purpose: every label is ASCII (that is a rule
// of the item vocabulary, not an accident) and every glyph is one column, so
// `len()` is the column count and herdr's own `item.len()` sizing agrees with
// what is drawn here. A non-ASCII label would silently over-pad, which is why
// the separator — the one non-ASCII item — never reaches that arithmetic and
// is drawn by its own branch below.

/// The row, drawn to fill exactly `width` columns.
///
/// `width` is the list's TEXT width — the popup's inner width less the column
/// ratatui reserves for `highlight_symbol`. Right-aligning inside it puts the
/// glyph flush against the popup's right border.
///
/// Neither the label nor the glyph sets a foreground: they inherit the list's
/// style, so the selected row's highlight repaints them like any other text. A
/// span with its own colour would keep that colour ON the highlight bar, which
/// is exactly the row you most want legible. The separator is the exception,
/// and it is the one row no gesture can select.
pub(super) fn drip_menu_row(
    item: &'static str,
    width: u16,
    p: &crate::app::state::Palette,
) -> Line<'static> {
    if item == crate::app::state::DRIP_MENU_SEPARATOR {
        return Line::from(Span::styled(
            crate::app::state::DRIP_MENU_SEPARATOR.repeat(width as usize),
            Style::default().fg(p.surface_dim),
        ));
    }

    // No glyph, or a popup too narrow to hold label + gap + glyph: the label
    // alone, exactly as stock drew it. Clamped rather than truncated because
    // the menu is only ever this narrow when the screen is, and a label with
    // its tail cut off is worse than one that runs to the border.
    let Some(glyph) = crate::app::state::drip_menu_glyph(item) else {
        return Line::from(item);
    };
    let gap = (width as usize).saturating_sub(item.len() + 1);
    if gap == 0 {
        return Line::from(item);
    }

    Line::from(vec![
        Span::raw(item),
        Span::raw(" ".repeat(gap)),
        Span::raw(glyph),
    ])
}
