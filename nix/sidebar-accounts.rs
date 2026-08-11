// ---------------------------------------------------------------------------
// drip hardcore plugin: the accounts rail.
//
// Appended to src/ui/sidebar.rs by nix/herdr-patches.nix. Everything here is
// prefixed `drip_` so it cannot collide with anything upstream grows, and it
// reads ONE file -- it starts no processes, holds no credentials and knows
// nothing about gumbo. The drip.gumbo-usage plugin writes that file with
// `gumbo watch --format compact --tags`; when nothing writes it, every
// function here returns empty and the sidebar is exactly what it was.
//
// The file's format is one line per row: `<severity><kind> <text>`, where
// severity is g/y/r/- (green, yellow, red, unknown) and kind is `a` for a line
// that opens an account or `c` for one that continues it. That is gumbo's
// `--tags` contract, and it is deliberately dumb: the writer owns all the
// layout, so changing how the rail LOOKS never means rebuilding herdr.
// ---------------------------------------------------------------------------

/// Rows the agent panel keeps for itself. It renders nothing at all below three
/// rows (its own early return), so the rail must never squeeze it past its
/// header plus a couple of agents -- the accounts are the guest here.
const DRIP_AGENT_PANEL_FLOOR: u16 = AGENT_PANEL_HEADER_ROWS + 2;

/// The same floor for the collapsed rail, where an agent is one row: keep two
/// of them visible before the accounts get any space at all.
const DRIP_COLLAPSED_AGENT_FLOOR: u16 = 2;

/// A file older than this is treated as absent. A watcher that was killed
/// leaves its last frame on disk, and hour-old headroom presented as current is
/// worse than no headroom at all: it is the number you would act on.
const DRIP_ACCOUNTS_STALE: std::time::Duration = std::time::Duration::from_secs(600);

/// How often the file is actually stat'd. The sidebar redraws on every event,
/// which can be hundreds of times a second while a pane is chatty; the rail
/// changes twice a minute at most.
const DRIP_ACCOUNTS_POLL: std::time::Duration = std::time::Duration::from_millis(500);

/// One parsed row of the accounts file.
#[derive(Clone)]
pub(crate) struct DripAccountLine {
    pub text: String,
    /// The grade of the window on THIS row, not of the account -- so a green 5h
    /// above a red 7d shows as two colours, matching what the numbers say.
    pub severity: char,
    /// Whether this row opens an account. The collapsed rail groups on it.
    pub opens_account: bool,
}

/// Where the accounts file lives: `$HERDR_DRIP_ACCOUNTS_FILE`, else
/// `$XDG_STATE_HOME/herdr-drip/sidebar-accounts.txt`, else the same under
/// `~/.local/state`. The same resolution the plugin's `bin/watch` does, and the
/// same state root `yolo-shell` already uses for its per-pane records.
fn drip_accounts_path() -> Option<std::path::PathBuf> {
    if let Some(path) = std::env::var_os("HERDR_DRIP_ACCOUNTS_FILE") {
        return Some(std::path::PathBuf::from(path));
    }
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".local/state"))
        })?;
    Some(state.join("herdr-drip").join("sidebar-accounts.txt"))
}

/// The rail's rows, re-read at most every [`DRIP_ACCOUNTS_POLL`].
pub(crate) fn drip_accounts_lines() -> Vec<DripAccountLine> {
    struct Cache {
        checked: Option<std::time::Instant>,
        lines: Vec<DripAccountLine>,
    }
    static CACHE: std::sync::OnceLock<std::sync::Mutex<Cache>> = std::sync::OnceLock::new();

    let cache = CACHE.get_or_init(|| {
        std::sync::Mutex::new(Cache {
            checked: None,
            lines: Vec::new(),
        })
    });
    // A poisoned lock means some other thread panicked mid-read. The rail is
    // decoration: take the empty answer rather than propagating the panic into
    // a draw.
    let Ok(mut cache) = cache.lock() else {
        return Vec::new();
    };
    if cache
        .checked
        .is_some_and(|at| at.elapsed() < DRIP_ACCOUNTS_POLL)
    {
        return cache.lines.clone();
    }
    cache.checked = Some(std::time::Instant::now());
    cache.lines = drip_read_accounts();
    cache.lines.clone()
}

/// Read and parse the file. Every failure -- no path, no file, unreadable,
/// stale, malformed -- is the same answer: no rows, and the sidebar looks like
/// stock herdr.
fn drip_read_accounts() -> Vec<DripAccountLine> {
    let Some(path) = drip_accounts_path() else {
        return Vec::new();
    };
    let fresh = std::fs::metadata(&path)
        .and_then(|meta| meta.modified())
        .map(|at| at.elapsed().is_ok_and(|age| age <= DRIP_ACCOUNTS_STALE))
        .unwrap_or(false);
    if !fresh {
        return Vec::new();
    }
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    text.lines().filter_map(drip_parse_line).collect()
}

/// `<severity><kind> <text>` -> a row. Anything else is dropped: a half-written
/// frame cannot reach here (the writer renames a temp into place) but a file
/// someone edited by hand can, and a rail is not worth a panic.
fn drip_parse_line(line: &str) -> Option<DripAccountLine> {
    let mut chars = line.chars();
    let severity = chars.next()?;
    let kind = chars.next()?;
    if chars.next() != Some(' ') || !matches!(severity, 'g' | 'y' | 'r' | '-') {
        return None;
    }
    Some(DripAccountLine {
        text: chars.as_str().to_string(),
        severity,
        opens_account: match kind {
            'a' => true,
            'c' => false,
            _ => return None,
        },
    })
}

/// The palette colour for a severity, matching the grades gumbo paints its own
/// output with and the vocabulary the agent dots already use in this sidebar.
fn drip_severity_color(severity: char, p: &Palette) -> ratatui::style::Color {
    match severity {
        'g' => p.green,
        'y' => p.yellow,
        'r' => p.red,
        _ => p.overlay0,
    }
}

/// One severity per ACCOUNT: the worst grade from each opening row up to the
/// next one. A single dot cannot say "roomy hour, spent week", and of the two
/// answers only the pessimistic one is safe to start a session on.
pub(crate) fn drip_account_dots(lines: &[DripAccountLine]) -> Vec<char> {
    let rank = |severity: char| match severity {
        'r' => 3,
        'y' => 2,
        'g' => 1,
        _ => 0,
    };
    let mut dots: Vec<char> = Vec::new();
    for line in lines {
        if line.opens_account {
            dots.push(line.severity);
        } else if let Some(last) = dots.last_mut() {
            if rank(line.severity) > rank(*last) {
                *last = line.severity;
            }
        }
    }
    dots
}

/// Carve the accounts rail off the bottom of the agent panel's area.
///
/// Returns `(agent_area, accounts_area)`, the second empty whenever the rail
/// cannot be drawn: no rows to show, or a sidebar too short to give them
/// without starving the agent list. The agents were here first.
///
/// Takes the rows rather than reading them so that layout is a pure function of
/// what there is to draw -- the caller reads once per frame and hands the same
/// rows to the split and the draw, which is also what stops the two disagreeing
/// if the file changes between them.
pub(crate) fn drip_accounts_split(detail: Rect, lines: &[DripAccountLine]) -> (Rect, Rect) {
    if lines.is_empty() || detail.width == 0 {
        return (detail, Rect::default());
    }
    // A separator and a header, then the rows.
    let wanted = lines.len() as u16 + 2;
    let rows = wanted.min(detail.height.saturating_sub(DRIP_AGENT_PANEL_FLOOR));
    if rows < 3 {
        return (detail, Rect::default());
    }
    let split = detail.height - rows;
    (
        Rect::new(detail.x, detail.y, detail.width, split),
        Rect::new(detail.x, detail.y + split, detail.width, rows),
    )
}

/// Draw the rail: a separator, an `accounts` header, then the rows in their own
/// colours. Shaped like [`render_agent_detail`]'s header on purpose -- under
/// the agent list it should read as the next section of one list, not as a
/// widget someone bolted on.
pub(crate) fn drip_render_accounts(
    app: &AppState,
    frame: &mut Frame,
    area: Rect,
    lines: &[DripAccountLine],
) {
    if area.width == 0 || area.height < 3 {
        return;
    }
    let p = &app.palette;
    frame.render_widget(
        Paragraph::new(Span::styled(
            "─".repeat(area.width as usize),
            Style::default().fg(p.surface_dim),
        )),
        Rect::new(area.x, area.y, area.width, 1),
    );
    frame.render_widget(
        Paragraph::new(Span::styled(
            " accounts",
            Style::default().fg(p.overlay0).add_modifier(Modifier::BOLD),
        )),
        Rect::new(area.x, area.y + 1, area.width, 1),
    );

    let body = Rect::new(area.x, area.y + 2, area.width, area.height - 2);
    for (index, line) in lines.iter().take(body.height as usize).enumerate() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                truncate_end(&line.text, body.width as usize),
                Style::default().fg(drip_severity_color(line.severity, p)),
            )),
            Rect::new(body.x, body.y + index as u16, body.width, 1),
        );
    }
}

/// The collapsed rail's share: one row per account at the bottom of the agent
/// column. Three columns is all there is, so an account is a number and a dot,
/// which is exactly what a workspace and an agent already are up the same rail.
pub(crate) fn drip_accounts_split_collapsed(detail: Rect, dots: &[char]) -> (Rect, Rect) {
    if dots.is_empty() || detail.width == 0 {
        return (detail, Rect::default());
    }
    // A divider row, one row per account, and the bottom row the collapsed
    // sidebar keeps for its own toggle glyph -- claimed here so the rail's rows
    // and that glyph cannot land on each other.
    let wanted = dots.len() as u16 + 2;
    let rows = wanted.min(detail.height.saturating_sub(DRIP_COLLAPSED_AGENT_FLOOR));
    if rows < 3 {
        return (detail, Rect::default());
    }
    let split = detail.height - rows;
    (
        Rect::new(detail.x, detail.y, detail.width, split),
        Rect::new(detail.x, detail.y + split, detail.width, rows),
    )
}

/// The rail's own rect: the rows [`drip_accounts_split`] took off the bottom of
/// the agent area.
///
/// Derived from where the carve left the boundary rather than recomputed from
/// the section geometry, which is the point: the split is patched into herdr's
/// own `*_sidebar_sections`, so every consumer -- the draw, the click
/// hit-testing, the scroll metrics -- already agrees on where the agents end,
/// and this cannot drift from that answer the way a second copy of the maths
/// would.
///
/// `reserved` is rows at the very bottom the caller draws its own chrome in:
/// one for the collapsed rail's toggle, none for the expanded sidebar (where
/// the toggle has always been drawn over the agent list's last row anyway).
pub(crate) fn drip_accounts_rect(section: Rect, agents: Rect, reserved: u16) -> Rect {
    let bottom = (section.y + section.height).saturating_sub(reserved);
    let top = agents.y + agents.height;
    if top >= bottom || agents.width == 0 {
        return Rect::default();
    }
    Rect::new(agents.x, top, agents.width, bottom - top)
}

/// Draw the collapsed rail's dots.
pub(crate) fn drip_render_accounts_collapsed(
    app: &AppState,
    frame: &mut Frame,
    area: Rect,
    dots: &[char],
) {
    if area.width == 0 || area.height < 2 {
        return;
    }
    let p = &app.palette;
    let buf = frame.buffer_mut();
    for x in area.x..area.x + area.width {
        buf[(x, area.y)].set_symbol("─");
        buf[(x, area.y)].set_style(Style::default().fg(p.surface_dim));
    }
    for (index, severity) in dots
        .iter()
        .take(area.height.saturating_sub(1) as usize)
        .enumerate()
    {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    format!("{:<2}", index + 1),
                    Style::default().fg(p.overlay0),
                ),
                Span::styled(
                    // Hollow for an account nothing could be read from, filled
                    // for one that answered -- the same distinction the agent
                    // dots draw between a live agent and an unknown one.
                    if *severity == '-' { "○" } else { "●" },
                    Style::default().fg(drip_severity_color(*severity, p)),
                ),
            ])),
            Rect::new(area.x, area.y + 1 + index as u16, area.width, 1),
        );
    }
}

#[cfg(test)]
mod drip_accounts_tests {
    use super::*;

    fn line(tagged: &str) -> DripAccountLine {
        drip_parse_line(tagged).expect("parses")
    }

    #[test]
    fn a_tagged_line_splits_into_grade_kind_and_text() {
        let parsed = line("ga ▸● hext2   ▰▰▱▱▱  11m");

        assert_eq!(parsed.severity, 'g');
        assert!(parsed.opens_account);
        assert_eq!(parsed.text, "▸● hext2   ▰▰▱▱▱  11m");
    }

    #[test]
    fn malformed_lines_are_dropped_not_drawn() {
        // No tag, an unknown grade, an unknown kind, an empty line: all of them
        // are somebody's half-finished edit, and none of them is worth a panic
        // inside a draw.
        for bad in ["", "x", "za text", "gx text", "gatext"] {
            assert!(drip_parse_line(bad).is_none(), "{bad:?} should not parse");
        }
    }

    #[test]
    fn an_accounts_dot_is_the_worst_of_its_rows() {
        let lines = vec![
            line("ga ▸● hext2   ▰▰▱▱▱  11m"),
            line("rc         7d ▰▰▰▰▱   5d"),
            line("gc         fb ▰▱▱▱▱     "),
            line("ga  ● hext3   ▱▱▱▱▱ 3h31"),
            line("gc         7d ▱▱▱▱▱   6d"),
        ];

        // A green hour over a red week is a red account: one dot, worst news.
        assert_eq!(drip_account_dots(&lines), vec!['r', 'g']);
    }

    #[test]
    fn an_unmeasured_account_keeps_its_own_dot() {
        let lines = vec![line("-a  ○ hext1   inference")];
        assert_eq!(drip_account_dots(&lines), vec!['-']);
    }

    #[test]
    fn the_rail_never_starves_the_agent_panel() {
        // Three accounts, six rows: the agents keep their floor and the rail
        // takes what is left -- or nothing at all, in a sidebar this short.
        let lines: Vec<DripAccountLine> = (0..6)
            .map(|i| line(if i % 2 == 0 { "ga  ● acct  ▱▱▱▱▱  1h" } else { "gc     7d   ▱▱▱▱▱  6d" }))
            .collect();

        let tall = drip_accounts_split(Rect::new(0, 0, 20, 40), &lines);
        assert!(tall.0.height >= DRIP_AGENT_PANEL_FLOOR);
        // Whatever the split, the two halves tile the area they came from.
        assert_eq!(tall.0.height + tall.1.height, 40);
        assert_eq!(tall.1.height, lines.len() as u16 + 2);

        // Too short to give the rail three rows without starving the agents:
        // the agent panel keeps the whole area and the rail draws nothing.
        let short = drip_accounts_split(Rect::new(0, 0, 20, 6), &lines);
        assert_eq!(short.1, Rect::default());
        assert_eq!(short.0, Rect::new(0, 0, 20, 6));
    }

    #[test]
    fn no_rows_means_the_sidebar_is_stock_herdr() {
        // The whole "does nothing when nothing feeds it" promise, in one line:
        // no file, no rows, no geometry change for the agent panel.
        let detail = Rect::new(0, 3, 25, 30);
        assert_eq!(drip_accounts_split(detail, &[]), (detail, Rect::default()));
        assert_eq!(
            drip_accounts_split_collapsed(detail, &[]),
            (detail, Rect::default())
        );
    }
}
