//! herdr-beads - a beads (bd) board for herdr: List / Table / Kanban over your
//! bd issues, docked as a side panel or floating as a popup.

mod app;
mod bd;
mod config;
mod form;
mod input;
mod keys;
mod model;
mod selftest;
mod ui;
mod views;

use crate::app::App;
use crate::model::{Mode, Scope};
use anyhow::Result;
use ratatui::backend::{Backend, CrosstermBackend};
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen, SetTitle,
};
use ratatui::Terminal;
use std::io;

fn parse_args() -> (Mode, Scope, bool) {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut mode = Mode::Popup;
    let mut scope = Scope::Repo;
    let mut selftest = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--mode" => {
                if let Some(v) = args.get(i + 1) {
                    mode = if v == "dock" { Mode::Dock } else { Mode::Popup };
                    i += 1;
                }
            }
            "--dock" => mode = Mode::Dock,
            "--popup" | "--board" => mode = Mode::Popup,
            "--global" => scope = Scope::Global,
            "--selftest" => selftest = true,
            _ => {}
        }
        i += 1;
    }
    (mode, scope, selftest)
}

/// Does `dir`, or anything above it, hold a `.beads`?
///
/// DRIP CHANGE. Upstream tested `dir/.beads` and nothing else, which scores a
/// shell sitting two levels inside a checkout at zero -- and cd'ing around
/// inside a repo is the normal way to use one. `bd` itself walks up, and so
/// does the rail (`beads/bin/board.js`'s `hasBoard`, which this is the port
/// of); a board the tool would have answered from that exact cwd must not be
/// invisible to the thing whose whole job is finding it.
fn has_board(dir: &str) -> bool {
    let mut at = std::path::Path::new(dir);
    loop {
        if at.join(".beads").exists() {
            return true;
        }
        match at.parent() {
            Some(up) => at = up,
            None => return false,
        }
    }
}

/// Resolve the repo the board should scope to. herdr runs the pane in the
/// plugin dir (no `.beads`), but injects HERDR_SOCKET_PATH / HERDR_WORKSPACE_ID
/// / HERDR_PANE_ID / HERDR_BIN_PATH - so we ask `herdr pane list` for the
/// focused pane's cwd in our workspace (same trick herdr-flist uses).
///
/// DRIP CHANGE, in two places, both ported from the rail's `boardCwd`
/// (`beads/bin/board.js`) which had already met them:
///
///   - `foreground_cwd` is consulted before `cwd`, because a shell that has
///     cd'd into a worktree is still reported at its launch directory and the
///     worktree is the board you are looking at. It only wins while it is
///     still ON a board, though -- a pane that cd'd out to /tmp should not
///     lose a repo its launch directory can still name;
///   - a score below 4 resolves to NOTHING rather than to the best of a bad
///     lot. Upstream took the top-scoring pane whatever it scored, so a
///     workspace with no board in it anywhere still picked some pane's home
///     directory and the board opened on whatever `bd` says there. Refusing to
///     guess leaves the process in the cwd the launcher gave it, which is the
///     answer the launcher's `--env HERDR_DRIP_BEADS_CWD` was for.
///
/// Our own pane is still skipped, as upstream: we live in the plugin root.
/// That is exactly why the `focused * 2` term earns nothing in a TAB, where
/// the board IS the focused pane -- there, `has_board * 4` is the whole score,
/// which is the other half of why the >= 4 floor matters.
fn resolve_repo_cwd() -> Option<String> {
    let me = std::env::var("HERDR_PANE_ID").unwrap_or_default();
    let ws = std::env::var("HERDR_WORKSPACE_ID").unwrap_or_default();
    let bin = std::env::var("HERDR_BIN_PATH").unwrap_or_else(|_| "herdr".to_string());
    let out = std::process::Command::new(&bin)
        .arg("pane")
        .arg("list")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let panes = v.get("result")?.get("panes")?.as_array()?;

    let others: Vec<&serde_json::Value> = panes
        .iter()
        .filter(|p| p.get("pane_id").and_then(|x| x.as_str()).unwrap_or("") != me)
        .collect();
    let field = |p: &serde_json::Value, k: &str| {
        p.get(k)
            .and_then(|x| x.as_str())
            .filter(|s| !s.is_empty())
            .map(String::from)
    };
    let cwd = |p: &serde_json::Value| {
        let fg = field(p, "foreground_cwd");
        let launched = field(p, "cwd");
        fg.clone()
            .filter(|d| has_board(d))
            .or_else(|| launched.clone().filter(|d| has_board(d)))
            .or(fg)
            .or(launched)
    };
    let in_ws = |p: &serde_json::Value| {
        !ws.is_empty() && p.get("workspace_id").and_then(|x| x.as_str()) == Some(ws.as_str())
    };
    let focused = |p: &serde_json::Value| p.get("focused").and_then(|x| x.as_bool()) == Some(true);

    // Preference order: a pane with a real board wins (focused first, my
    // workspace next). Anything scoring under 4 is not on a board at all.
    let mut best: Option<String> = None;
    let mut best_score = 0;
    for p in &others {
        if let Some(c) = cwd(p) {
            let score = (has_board(&c) as u8) * 4 + (focused(p) as u8) * 2 + (in_ws(p) as u8);
            if score > best_score {
                best_score = score;
                best = Some(c);
            }
        }
    }
    if best_score >= 4 {
        best
    } else {
        None
    }
}

fn apply_working_dir() {
    // 1) explicit override from the launcher. HERDR_DRIP_BEADS_CWD is the
    //    rail's spelling of the same thing and the launchers here set it, so
    //    one variable pins both surfaces to one repo; HERDR_BEADS_CWD is
    //    upstream's and still wins, being the more specific of the two.
    for var in ["HERDR_DRIP_BEADS_CWD", "HERDR_BEADS_CWD"] {
        if let Ok(dir) = std::env::var(var) {
            if !dir.is_empty() {
                let _ = std::env::set_current_dir(&dir);
            }
        }
    }
    // 2) if there is still no board at or above here, ask herdr for one.
    //    Resolved absolutely, because the walk up from a relative "." reaches
    //    the top after one step and would only ever test the cwd itself.
    let here = std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| ".".to_string());
    if !has_board(&here) {
        if let Some(dir) = resolve_repo_cwd() {
            let _ = std::env::set_current_dir(&dir);
        }
    }
}

fn main() -> Result<()> {
    let (mode, scope, selftest) = parse_args();

    apply_working_dir();

    if selftest {
        return selftest::run(scope);
    }

    // Tag the pane so the launcher can find (and toggle) it via `herdr pane list`.
    let pane_title = match mode {
        Mode::Dock => "herdr-beads-dock",
        Mode::Popup => "herdr-beads-board",
    };
    enable_raw_mode()?;
    let mut out = io::stdout();
    execute!(
        out,
        EnterAlternateScreen,
        EnableMouseCapture,
        SetTitle(pane_title)
    )?;
    let mut terminal = Terminal::new(CrosstermBackend::new(out))?;

    let mut app = App::new(mode, scope);
    let res = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    // DRIP CHANGE: `q` closes the pane, not just the process. See
    // `App::close_pane` for why, and note the order -- the terminal is restored
    // first, so a pane that herdr declines to close (the last pane of the only
    // tab is a no-op) is left in a usable shell rather than in the alternate
    // screen. Only on a clean quit: an error exit keeps the pane so its message
    // can be read.
    if res.is_ok() && app.should_quit {
        app.close_pane();
    }

    res
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| ui::render(f, app))?;
        if app.should_quit {
            break;
        }
        match event::read()? {
            Event::Key(k) if k.kind == KeyEventKind::Press => keys::handle_key(app, k),
            Event::Mouse(m) => keys::handle_mouse(app, m),
            _ => {}
        }
        if app.should_quit {
            break;
        }
    }
    Ok(())
}
