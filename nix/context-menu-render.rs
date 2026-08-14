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
/// ratatui reserves for `highlight_symbol`. The glyph is right-aligned inside
/// it with ONE COLUMN held back, so it sits a space clear of the popup's right
/// border rather than against it. That column is paid for by the sizing patch
/// in mouse.rs (`+ 3` for a row with a glyph, not `+ 2`), so the margin comes
/// out of the popup's width and never out of the label's.
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
    // item + gap + glyph + the one-column right margin == width.
    let gap = (width as usize).saturating_sub(item.len() + 2);
    if gap == 0 {
        return Line::from(item);
    }

    Line::from(vec![
        Span::raw(item),
        Span::raw(" ".repeat(gap)),
        Span::raw(glyph),
        Span::raw(" "),
    ])
}

/// The context menu's popup shell: rounded, and neutral instead of accented.
///
/// Stock draws it with `render_panel_shell(.., p.accent, p.panel_bg)`, which
/// is square-cornered (`border::PLAIN`) and paints the border in the accent —
/// so the menu arrived as a coloured box in a UI whose accent is otherwise
/// reserved for what is ACTIVE. It is a transient popup, not a selection, and
/// a neutral frame says so. Rounded to match the pane frames, which the
/// rounded-corners patch already softened.
///
/// Not a change to `render_panel_shell` itself, deliberately: that helper is
/// shared with the dialogs, the settings panel and the navigator, and rounding
/// it there would round all of them on the strength of a request about one
/// menu. One call site, one anchor.
///
/// The colours are TOKENS, not literals — `surface1` for the border and (via
/// the highlight patch) `surface0` for the selected row — so the shade is
/// tunable from config.toml with a `herdr server reload-config`, instead of
/// costing a herdr rebuild every time an eye disagrees with it.
///
/// `Block`, `Borders` and `BorderType` are fully qualified because menus.rs
/// imports `Clear, List, ListItem, ListState, Paragraph` and none of those;
/// adding a `use` would need a second anchor for nothing.
pub(super) fn drip_menu_shell(
    frame: &mut Frame,
    area: Rect,
    p: &crate::app::state::Palette,
) -> Option<Rect> {
    if area.width < 2 || area.height < 2 {
        return None;
    }

    let block = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(Style::default().fg(p.surface1))
        .style(Style::default().bg(p.panel_bg));
    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    Some(inner)
}
