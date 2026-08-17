//! The detail pane/modal: full bead info, enriched by `bd show` when cached.

use crate::app::App;
use crate::bd::types::Bead;
use crate::markdown;
use crate::model::{status_glyph, status_label};
use crate::ui::theme;
use crate::ui::widgets::centered_rect;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

pub fn build_lines(b: &Bead) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(Span::styled(
        b.id.clone(),
        Style::default().fg(theme::OVERLAY1),
    )));
    lines.push(Line::from(Span::styled(
        b.title.clone(),
        Style::default()
            .fg(theme::TEXT)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::raw(""));
    lines.push(Line::from(vec![
        Span::styled(
            format!("{} {}", status_glyph(&b.status), status_label(&b.status)),
            Style::default().fg(theme::status_color(&b.status)),
        ),
        Span::raw("   "),
        Span::styled(
            theme::priority_glyph(b.priority).to_string(),
            Style::default().fg(theme::priority_color(b.priority)),
        ),
        Span::raw("   "),
        Span::styled(b.issue_type.clone(), Style::default().fg(theme::SUBTEXT)),
        Span::raw("   "),
        Span::styled(
            format!("@{}", b.assignee()),
            Style::default().fg(theme::OVERLAY0),
        ),
    ]));
    if let (Some(c), Some(u)) = (&b.created_at, &b.updated_at) {
        lines.push(Line::from(Span::styled(
            format!("created {c} · updated {u}"),
            Style::default().fg(theme::OVERLAY0),
        )));
    }
    lines.push(Line::raw(""));
    if !b.description.is_empty() {
        lines.push(Line::from(Span::styled(
            "Description",
            Style::default().fg(theme::YELLOW),
        )));
        // DRIP CHANGE: this was one plain `Span` holding the whole description.
        // bd descriptions are markdown and this repo's are heavily so, which
        // meant the pane showed `##` and ``` as literal noise wrapped into a
        // single paragraph. See `crate::markdown` for the renderer and for what
        // it does not handle.
        lines.extend(markdown::render(&b.description));
        lines.push(Line::raw(""));
    }
    if !b.dependencies.is_empty() {
        lines.push(Line::from(Span::styled(
            format!("Dependencies ({})", b.dependencies.len()),
            Style::default().fg(theme::YELLOW),
        )));
        for d in &b.dependencies {
            lines.push(Line::from(Span::styled(
                format!("  • {}", d.label()),
                Style::default().fg(theme::SUBTEXT),
            )));
        }
        lines.push(Line::raw(""));
    }
    lines.push(Line::from(Span::styled(
        format!(
            "deps {} · dependents {} · comments {}",
            b.dependency_count, b.dependent_count, b.comment_count
        ),
        Style::default().fg(theme::OVERLAY0),
    )));
    lines
}

fn lines_for(app: &App) -> Vec<Line<'static>> {
    match app.detail_bead() {
        Some(b) => build_lines(&b),
        None => vec![Line::from(Span::styled(
            "no selection",
            Style::default().fg(theme::OVERLAY0),
        ))],
    }
}

/// Draw the detail text into `inner`, scrolled, and record what the scroll
/// keys need to know: how tall the content came out and how much of it fits.
///
/// DRIP ADDITION. Upstream drew the whole `Text` into the block and let ratatui
/// drop whatever fell off the bottom, which was survivable when a description
/// was one wrapped paragraph. Rendered as markdown, and against boards whose
/// descriptions run to thousands of lines, everything past the first screen was
/// simply unreachable.
fn render_text(f: &mut Frame, block: Block<'static>, rect: Rect, app: &mut App) {
    let lines = lines_for(app);
    let inner = block.inner(rect);
    app.detail_content_h = markdown::wrapped_height(&lines, inner.width);
    app.detail_view_h = inner.height as usize;
    app.hits.detail = Some(rect);
    // The content may have shrunk since the offset was set (a smaller bead, a
    // wider pane), so re-clamp rather than trusting the stored value.
    let scroll = app.detail_scroll.min(app.detail_max_scroll());
    app.detail_scroll = scroll;
    f.render_widget(
        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0))
            // Unstyled markdown text inherits the pane's own foreground; spans
            // that carry a colour (headings, code, syntect) still win.
            .style(Style::default().fg(theme::SUBTEXT).bg(Color::Reset)),
        rect,
    );
}

/// `12%` when there is more below, so a truncated description does not look
/// like the whole of one.
fn scroll_title(app: &App) -> String {
    let max = app.detail_max_scroll();
    if max == 0 {
        String::new()
    } else {
        let pct = (app.detail_scroll as u32 * 100 / max as u32).min(100);
        format!("{pct}% ")
    }
}

pub fn render_side(f: &mut Frame, area: Rect, app: &mut App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme::SURFACE2))
        .title(format!(" Detail {}", scroll_title(app)))
        .style(Style::default().bg(Color::Reset));
    render_text(f, block, area, app);
}

pub fn render_modal(f: &mut Frame, area: Rect, app: &mut App) {
    let rect = centered_rect(app.modal_pct, 80, area);
    app.hits.modal = Some(rect);
    app.hits.modal_area = Some(area);
    f.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme::MAUVE))
        .title(format!(
            " Detail {}- < > resize · PgUp/PgDn scroll · Esc to close ",
            scroll_title(app)
        ))
        .style(Style::default().bg(Color::Reset));
    render_text(f, block, rect, app);
}
