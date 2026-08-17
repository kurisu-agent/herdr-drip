//! Key and mouse dispatch. Keeps the keymap in one place; App holds the logic.

use crate::app::{App, Drag};
use crate::input::InputKind;
use crate::model::View;
use ratatui::crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

pub fn help_lines() -> Vec<(&'static str, &'static str)> {
    vec![
        ("K / Tab", "cycle view (List / Table / Kanban)"),
        ("j k ↑ ↓", "move selection"),
        ("h l ← →", "kanban: change column · list: collapse/expand"),
        ("1-9", "jump to status group N"),
        ("[ ]", "jump to prev / next status group"),
        ("Enter", "open detail"),
        ("d", "toggle detail pane / modal"),
        ("< >  , .", "detail: narrower / wider (or drag the divider)"),
        ("=", "detail: back to the default width"),
        (
            "PgUp/PgDn",
            "detail: scroll the description (wheel works too)",
        ),
        ("v", "move mode: then h/l retags status"),
        ("c", "claim (in_progress + assign you)"),
        ("x", "close (prompts for reason)"),
        ("p", "set priority (0-4)"),
        ("n", "add note (bd note)"),
        ("m", "add comment (bd comment)"),
        (
            "a",
            "new bead form (Tab: field · ←→: type/priority · Enter: create)",
        ),
        ("e", "edit selected bead (reopens the form)"),
        ("s", "set status (then pick 1-9)"),
        ("F", "focus: show only this status group"),
        ("o", "table: cycle sort (status/priority/changed)"),
        ("/", "filter  (Esc clears it)"),
        ("g", "toggle scope repo ⇄ global"),
        ("C", "toggle showing closed"),
        ("r", "refresh from bd"),
        ("f", "zoom pane fullscreen (toggle)"),
        ("? ", "this help"),
        ("q", "quit, closing this pane"),
        ("Esc", "back out a layer (never quits)"),
    ]
}

fn hit(r: &Rect, col: u16, row: u16) -> bool {
    col >= r.x
        && col < r.x.saturating_add(r.width)
        && row >= r.y
        && row < r.y.saturating_add(r.height)
}

/// Which resize handle, if any, is under the pointer. The divider is one column
/// wide, so it is grabbable one column either side as well — a 1px target is a
/// target you miss.
fn grab_target(app: &App, col: u16, row: u16) -> Option<Drag> {
    if let Some(d) = app.hits.divider {
        let wide = Rect::new(d.x.saturating_sub(1), d.y, d.width + 2, d.height);
        if hit(&wide, col, row) {
            return Some(Drag::Divider);
        }
    }
    if let Some(m) = app.hits.modal {
        if row >= m.y && row < m.y.saturating_add(m.height) {
            let right = m.x.saturating_add(m.width).saturating_sub(1);
            if col.abs_diff(m.x) <= 1 {
                return Some(Drag::ModalLeft);
            }
            if col.abs_diff(right) <= 1 {
                return Some(Drag::ModalRight);
            }
        }
    }
    None
}

pub fn handle_mouse(app: &mut App, ev: MouseEvent) {
    match ev.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            // DRIP ADDITION: a resize handle takes the press before anything
            // else, and the press itself starts the drag.
            if let Some(t) = grab_target(app, ev.column, ev.row) {
                app.drag = Some(t);
                return;
            }
            for (v, r) in app.hits.tabs.clone() {
                if hit(&r, ev.column, ev.row) {
                    app.set_view(v);
                    return;
                }
            }
            // A click inside the modal is a click on the modal, not on the row
            // it happens to be covering.
            if app.hits.modal.is_some_and(|m| hit(&m, ev.column, ev.row)) {
                return;
            }
            for (id, r) in app.hits.rows.clone() {
                if hit(&r, ev.column, ev.row) {
                    app.select(id);
                    return;
                }
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => app.drag_to(ev.column),
        MouseEventKind::Up(MouseButton::Left) => app.drag = None,
        // The wheel scrolls whatever is under it: the description if the
        // pointer is over the detail, the selection otherwise.
        MouseEventKind::ScrollDown => {
            if over_detail(app, ev.column, ev.row) {
                app.scroll_detail(3);
            } else {
                app.nav_vert(1);
            }
        }
        MouseEventKind::ScrollUp => {
            if over_detail(app, ev.column, ev.row) {
                app.scroll_detail(-3);
            } else {
                app.nav_vert(-1);
            }
        }
        _ => {}
    }
}

fn over_detail(app: &App, col: u16, row: u16) -> bool {
    app.hits.detail.is_some_and(|r| hit(&r, col, row))
}

pub fn handle_key(app: &mut App, key: KeyEvent) {
    // 1) Text prompt swallows everything.
    if let Some(inp) = app.input.as_mut() {
        match key.code {
            KeyCode::Enter => app.submit_input(),
            KeyCode::Esc => {
                app.input = None;
            }
            KeyCode::Backspace => inp.backspace(),
            KeyCode::Char(c) => inp.push(c),
            _ => {}
        }
        return;
    }

    // 1b) Create form.
    if let Some(f) = app.create_form.as_mut() {
        match key.code {
            KeyCode::Esc => app.create_form = None,
            KeyCode::Enter => app.submit_create_form(),
            KeyCode::Tab | KeyCode::Down => f.next_field(),
            KeyCode::BackTab | KeyCode::Up => f.prev_field(),
            KeyCode::Left => f.left(),
            KeyCode::Right => f.right(),
            KeyCode::Backspace => f.backspace(),
            KeyCode::Char(c) => f.input_char(c),
            _ => {}
        }
        return;
    }

    // 2) Help overlay: any key closes it.
    if app.show_help {
        app.show_help = false;
        return;
    }

    // 2b) Status picker: a digit sets the selected bead's status; anything else cancels.
    if app.status_pick {
        match key.code {
            KeyCode::Char(c @ '1'..='9') => app.pick_status((c as u8 - b'1') as usize),
            _ => app.status_pick = false,
        }
        return;
    }

    let sel = app.selected.clone();

    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Esc => {
            // Esc never quits (use q) - it only backs out of a layer.
            if !app.filter.is_empty() {
                app.clear_filter();
            } else if app.move_mode {
                app.move_mode = false;
            } else if app.detail_modal {
                app.detail_modal = false;
            } else if app.show_detail {
                app.show_detail = false;
            }
        }
        KeyCode::Char('?') => app.show_help = true,

        KeyCode::Char('K') | KeyCode::Tab => {
            let v = app.view.next();
            app.set_view(v);
        }
        KeyCode::BackTab => {
            let v = app.view.prev();
            app.set_view(v);
        }

        KeyCode::Char('j') | KeyCode::Down => app.nav_vert(1),
        KeyCode::Char('k') | KeyCode::Up => app.nav_vert(-1),
        KeyCode::Char('h') | KeyCode::Left => {
            if app.move_mode {
                app.retag(-1);
            } else {
                app.nav_horiz(-1);
            }
        }
        KeyCode::Char('l') | KeyCode::Right => {
            if app.move_mode {
                app.retag(1);
            } else {
                app.nav_horiz(1);
            }
        }
        KeyCode::Home => app.nav_vert(i32::MIN / 2),
        KeyCode::Char('G') | KeyCode::End => app.nav_vert(i32::MAX / 2),

        KeyCode::Char(c @ '1'..='9') => {
            let n = (c as u8 - b'1') as usize;
            app.jump_group(n);
        }
        KeyCode::Char('[') => app.jump_group_rel(-1),
        KeyCode::Char(']') => app.jump_group_rel(1),

        KeyCode::Enter => app.open_detail(),
        KeyCode::Char('d') => app.toggle_detail(),
        // DRIP ADDITION: the detail split is adjustable. `<`/`>` move it, `=`
        // puts it back, PgUp/PgDn scroll inside it (and page the list when
        // there is no detail on screen to scroll).
        KeyCode::Char('<') | KeyCode::Char(',') => app.resize_detail(-1),
        KeyCode::Char('>') | KeyCode::Char('.') => app.resize_detail(1),
        KeyCode::Char('=') => app.reset_detail_size(),
        KeyCode::PageDown => {
            if app.detail_on_screen() {
                app.scroll_detail(10);
            } else {
                app.nav_vert(10);
            }
        }
        KeyCode::PageUp => {
            if app.detail_on_screen() {
                app.scroll_detail(-10);
            } else {
                app.nav_vert(-10);
            }
        }

        KeyCode::Char('v') => app.move_mode = !app.move_mode,
        KeyCode::Char('c') => app.claim_selected(),
        KeyCode::Char('x') => {
            if let Some(id) = sel {
                app.open_input(InputKind::CloseReason(id));
            }
        }
        KeyCode::Char('n') => {
            if let Some(id) = sel {
                app.open_input(InputKind::Note(id));
            }
        }
        KeyCode::Char('m') => {
            if let Some(id) = sel {
                app.open_input(InputKind::Comment(id));
            }
        }
        KeyCode::Char('p') => {
            if let Some(id) = sel {
                app.open_input(InputKind::Priority(id));
            }
        }
        KeyCode::Char('a') => app.open_create_form(),
        KeyCode::Char('e') => app.open_edit_form(),
        KeyCode::Char('s') => {
            if app.selected.is_some() {
                app.status_pick = true;
            }
        }
        KeyCode::Char('F') => app.focus_status(),
        KeyCode::Char('o') => {
            if app.view == View::Table {
                app.sort = app.sort.next();
                app.status_msg = format!("sort: {}", app.sort.label());
            }
        }

        KeyCode::Char('/') => app.open_input(InputKind::Filter),
        KeyCode::Char('g') => app.toggle_scope(),
        KeyCode::Char('C') => app.toggle_closed(),
        KeyCode::Char('r') => app.reload(),
        KeyCode::Char('f') => app.toggle_zoom(),
        _ => {}
    }
}
