//! Top-level compositor: activity bar, body (view + optional detail pane),
//! status bar, and the modal/overlay layers.

pub mod theme;
pub mod widgets;

use crate::app::App;
use crate::model::View;
use crate::views;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::Frame;

pub fn render(f: &mut Frame, app: &mut App) {
    app.hits.clear();
    let area = f.area();
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(area);

    widgets::render_activity_bar(f, rows[0], app);

    let body = rows[1];
    let want_detail = app.show_detail && app.view != View::Kanban;
    if want_detail && body.width > 70 {
        // wide: detail beside the list, with a grab bar between them.
        //
        // DRIP CHANGE: the width was `(body.width / 3).clamp(30, 52)`. It is
        // now whatever the user last dragged or typed it to (App::detail_width),
        // and the column that used to be a bare seam between two borders is a
        // real divider you can put a mouse on.
        let detail_w = app.detail_width(body.width);
        let cols = Layout::horizontal([
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(detail_w),
        ])
        .split(body);
        render_view(f, cols[0], app);
        app.hits.body = Some(body);
        app.hits.divider = Some(cols[1]);
        widgets::render_divider(f, cols[1], app.drag == Some(crate::app::Drag::Divider));
        views::detail::render_side(f, cols[2], app);
    } else if want_detail {
        // narrow sidebar: float the detail on top of the list (Clear-backed
        // modal) instead of splitting it below, so the full list stays put.
        render_view(f, body, app);
        views::detail::render_modal(f, area, app);
    } else {
        render_view(f, body, app);
    }

    widgets::render_status_bar(f, rows[2], app);

    if app.detail_modal {
        views::detail::render_modal(f, area, app);
    }
    if let Some(fm) = app.create_form.as_ref() {
        widgets::render_create_form(f, area, fm);
    }
    if app.input.is_some() {
        widgets::render_input(f, area, app);
    }
    if app.show_help {
        widgets::render_help(f, area);
    }
    if app.status_pick {
        widgets::render_status_pick(f, area, app);
    }
}

fn render_view(f: &mut Frame, area: Rect, app: &mut App) {
    match app.view {
        View::List => views::list::render(f, area, app),
        View::Table => views::table::render(f, area, app),
        View::Kanban => views::kanban::render(f, area, app),
    }
}
