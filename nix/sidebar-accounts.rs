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
// that opens an account or `c` for one that continues it. A tag with no text
// after it is an empty row -- the blank gumbo writes between accounts. That is
// gumbo's `--tags` contract, and it is deliberately dumb: the writer owns all
// the layout, so changing how the rail LOOKS never means rebuilding herdr.
//
// The one thing this side does know about the layout is where an account's
// identity stops and its numbers start (`DRIP_HEAD_COLUMNS`), because the two
// are painted in different colours and only one of them is the account's.
// ---------------------------------------------------------------------------

/// Rows the agent panel keeps for itself. It renders nothing at all below three
/// rows (its own early return), so the rail must never squeeze it past its
/// header plus a couple of agents -- the accounts are the guest here.
const DRIP_AGENT_PANEL_FLOOR: u16 = AGENT_PANEL_HEADER_ROWS + 2;

/// The same floor for the collapsed rail, where an agent is one row: keep two
/// of them visible before the accounts get any space at all.
const DRIP_COLLAPSED_AGENT_FLOOR: u16 = 2;

/// Rows above the first account: the separator alone. The rail used to spend
/// two more here -- an `accounts` title and a blank line under it -- but the
/// rows say what they are (a percent, a reset clock, a traffic-light dot),
/// and under the drip's quiet-chrome patch every section label went; the
/// separator is the whole announcement, as it is between the workspace list
/// and the agents.
const DRIP_HEADER_ROWS: u16 = 1;

/// One row kept BELOW the last account, so the rail does not sit flush on the
/// bottom edge of the sidebar. It is breathing room first, but it is also
/// correctness: `expanded_sidebar_toggle_rect` puts the `«` toggle on
/// `area.height - 1`, so a rail drawn to the bottom shares its last row with
/// the toggle and the widest account row runs into it. The collapsed rail
/// already reserved this row (`drip_accounts_rect(.., 1)`); the expanded one
/// passed 0 and was the odd one out.
const DRIP_FOOTER_ROWS: u16 = 1;

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
///
/// The separating space belongs to the text, so a tag with nothing after it is
/// a legal EMPTY row -- that is the blank line gumbo writes between accounts.
/// Requiring the space would make the spacer depend on a trailing blank
/// surviving every editor and hook the file passes through, which is not a
/// thing to bet the rail's spacing on.
fn drip_parse_line(line: &str) -> Option<DripAccountLine> {
    let mut chars = line.chars();
    let severity = chars.next()?;
    let kind = chars.next()?;
    match chars.next() {
        None => {}
        Some(' ') => {}
        Some(_) => return None,
    }
    if !matches!(severity, 'g' | 'y' | 'r' | '-') {
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

/// Characters at the front of a row that identify the ACCOUNT rather than
/// measure this window: the marker and the dot, plus the space after them.
///
/// A constant, matching gumbo's `COMPACT_HEAD`. It used to be found by scanning
/// for the first `▰`/`▱`, because the name column was sized from the rail's
/// width and the meter was the only fixed landmark in a row. The name is gone
/// and every column ahead of the meter is fixed now, so the boundary is a
/// number both sides can state — and scanning would put it in the wrong place
/// anyway, since what follows the head is this row's own percent, which must
/// not be painted with the account's grade.
const DRIP_HEAD_COLUMNS: usize = 3;

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

/// How many of `lines` to draw in `height` rows.
///
/// All of them when they fit. When they do not, the cut lands on an account
/// boundary rather than wherever the row count ran out: a partial account is a
/// stack of numbers with nothing saying which window any of them belongs to,
/// and the later rows of a group are the ones that carry the week and the
/// Fable weekly. Trailing blanks go with it,
/// so a truncated rail never ends on the spacer that was meant to separate the
/// account it just dropped.
///
/// Zero is a legitimate answer -- a rail with room for one and a half accounts
/// shows one, and a rail with room for less than one shows none and gives the
/// space back to the agents.
fn drip_fit(lines: &[DripAccountLine], height: usize) -> usize {
    if height >= lines.len() {
        return lines.len();
    }
    // Back up to the nearest place a group begins. That is an opening row, or
    // the blank in front of one -- cutting on the blank keeps the account above
    // it whole, which is a row better than cutting on the opener would do.
    let mut end = height;
    while end > 0 && !lines[end].opens_account && !lines[end].text.trim().is_empty() {
        end -= 1;
    }
    while end > 0 && lines[end - 1].text.trim().is_empty() {
        end -= 1;
    }
    end
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
    // A separator, then the rows, then one blank row under them. The rail used
    // to spend two more rows above -- a title and a blank under it -- and the
    // blank was load-bearing while the title was there, keeping the first
    // account from reading as part of it. With no title there is nothing for
    // the first row to run into, so the separator alone opens the rail, the
    // way it separates the workspace list from the agents. The blank BELOW is
    // carved here but never drawn -- the rail's rect stops short of it
    // (`drip_accounts_rect`'s `reserved`), so it is taken from the agent panel
    // exactly once rather than eating an account.
    //
    // What is left after the agents' floor and this chrome is the rows the
    // accounts may have, and [`drip_fit`] decides how many of them are worth
    // showing. Asking it here rather than clamping to the raw count is what
    // keeps the carve and the draw agreeing: the rail is exactly as tall as
    // what will be drawn in it, so a dropped account leaves no empty band.
    let budget = detail
        .height
        .saturating_sub(DRIP_AGENT_PANEL_FLOOR)
        .saturating_sub(DRIP_HEADER_ROWS + DRIP_FOOTER_ROWS);
    let shown = drip_fit(lines, budget as usize) as u16;
    if shown == 0 {
        return (detail, Rect::default());
    }
    let rows = shown + DRIP_HEADER_ROWS + DRIP_FOOTER_ROWS;
    let split = detail.height - rows;
    (
        Rect::new(detail.x, detail.y, detail.width, split),
        Rect::new(detail.x, detail.y + split, detail.width, rows),
    )
}

/// Draw the rail: a separator, then the rows in their own colours. Shaped
/// like [`render_agent_detail`]'s section break on purpose -- under the agent
/// list it should read as the next section of one list, not as a widget
/// someone bolted on.
pub(crate) fn drip_render_accounts(
    app: &AppState,
    frame: &mut Frame,
    area: Rect,
    lines: &[DripAccountLine],
) {
    if area.width == 0 || area.height <= DRIP_HEADER_ROWS {
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

    let body = Rect::new(
        area.x,
        area.y + DRIP_HEADER_ROWS,
        area.width,
        area.height - DRIP_HEADER_ROWS,
    );
    // The marker and dot opening an account are painted with the ACCOUNT's
    // grade -- the worst of its rows -- while the rest of the row keeps its own
    // window's. That is the whole point of the two colours: a walled 7d makes
    // the dot red even though the 5h meter beside it is green and honest.
    let dots = drip_account_dots(lines);
    let mut account = 0usize;
    let shown = drip_fit(lines, body.height as usize);
    for (index, line) in lines.iter().take(shown).enumerate() {
        let line_style = Style::default().fg(drip_severity_color(line.severity, p));
        let row = Rect::new(body.x, body.y + index as u16, body.width, 1);
        if !line.opens_account {
            frame.render_widget(
                Paragraph::new(Span::styled(
                    truncate_end(&line.text, body.width as usize),
                    line_style,
                )),
                row,
            );
            continue;
        }

        let severity = dots.get(account).copied().unwrap_or(line.severity);
        account += 1;
        // Marker and dot are the account's identity and take the account's
        // grade -- the worst of its rows. Everything after them is this row's
        // own 5h reading, percent included, and keeps its own colour: a green
        // hour under a spent week must not be painted as though the hour were
        // the problem.
        let head: String = line.text.chars().take(DRIP_HEAD_COLUMNS).collect();
        let rest: String = line.text.chars().skip(DRIP_HEAD_COLUMNS).collect();
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    head,
                    Style::default().fg(drip_severity_color(severity, p)),
                ),
                Span::styled(
                    truncate_end(
                        &rest,
                        body.width.saturating_sub(DRIP_HEAD_COLUMNS as u16) as usize,
                    ),
                    line_style,
                ),
            ])),
            row,
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

    /// The rail as gumbo writes it: three accounts of three rows each -- 5h,
    /// 7d, the Fable weekly -- blanks between them. Eleven lines, and the
    /// blanks are rows like any other.
    fn rail() -> Vec<DripAccountLine> {
        [
            "ga ▸●  46% ▰▰▱▱▱▱ 11m",
            "rc     96% ▰▰▰▰▰▱ 5d",
            "gc     52% ▰▰▰▱▱▱ 5d",
            "-c",
            "ga  ●   0% ▱▱▱▱▱▱",
            "gc     44% ▰▰▰▱▱▱ 5d",
            "yc     78% ▰▰▰▰▱▱ 5d",
            "-c",
            "ga  ●  31% ▰▱▱▱▱▱ 43m",
            "gc      8% ▱▱▱▱▱▱ 6d",
            "gc     12% ▱▱▱▱▱▱ 6d",
        ]
        .into_iter()
        .map(line)
        .collect()
    }

    #[test]
    fn a_tagged_line_splits_into_grade_kind_and_text() {
        let parsed = line("ga ▸●  46% ▰▰▱▱▱▱ 11m");

        assert_eq!(parsed.severity, 'g');
        assert!(parsed.opens_account);
        assert_eq!(parsed.text, "▸●  46% ▰▰▱▱▱▱ 11m");
    }

    #[test]
    fn a_bare_tag_is_the_blank_row_between_accounts() {
        // gumbo writes the spacer as a tag with nothing after it, so no line in
        // the file ends in whitespace and the spacing survives anything that
        // strips it. It continues the account above rather than opening one.
        let parsed = line("-c");

        assert_eq!(parsed.text, "");
        assert!(!parsed.opens_account);
        // The same row with the separating space still present: the writer
        // trims it, but a consumer that has to read both cannot afford to care.
        assert_eq!(line("-c ").text, "");
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
        let lines = rail();

        // A green hour over a red week is a red account, and a yellow Fable
        // row under two green windows is a yellow one: one dot, worst news,
        // whichever of the three rows carries it. The blanks fold into the
        // account above them and change nothing -- they grade `-`, which
        // ranks below everything.
        assert_eq!(drip_account_dots(&lines), vec!['r', 'y', 'g']);
    }

    #[test]
    fn the_account_colour_stops_before_the_percent() {
        // Marker and dot say how the ACCOUNT is doing; everything after them is
        // this row's own window. The percent is on the wrong side of that line
        // to take the account's grade -- a green 46% hour under a red week has
        // to stay green, or the rail says the hour is the problem.
        let row = "▸●  46% ▰▰▱▱▱▱ 11m";

        assert_eq!(
            row.chars().take(DRIP_HEAD_COLUMNS).collect::<String>(),
            "▸● "
        );
        assert_eq!(
            row.chars().skip(DRIP_HEAD_COLUMNS).collect::<String>(),
            " 46% ▰▰▱▱▱▱ 11m"
        );
    }

    #[test]
    fn an_unmeasured_account_keeps_its_own_dot() {
        // No windows, so no percent and no meter: the note sits where they
        // would, and the head is still the head.
        let lines = vec![line("-a  ○ inference")];
        assert_eq!(drip_account_dots(&lines), vec!['-']);
        assert_eq!(
            lines[0].text.chars().take(DRIP_HEAD_COLUMNS).collect::<String>(),
            " ○ "
        );
    }

    #[test]
    fn a_short_rail_drops_whole_accounts_and_no_trailing_blank() {
        let lines = rail();

        // Everything fits: everything is drawn.
        assert_eq!(drip_fit(&lines, 11), 11);
        assert_eq!(drip_fit(&lines, 99), 11);

        // One row short of all three. Two accounts survive, and the blank that
        // was separating them from the third goes with it -- a rail must never
        // end on the spacer for something it did not draw.
        assert_eq!(drip_fit(&lines, 10), 7);
        // Exactly two accounts and their separator: the cut lands on the blank
        // itself, which keeps the account above it whole.
        assert_eq!(drip_fit(&lines, 7), 7);
        // Room for the second account's hour and week but not its Fable row:
        // back off to the first account alone rather than draw a group missing
        // the row that says how much Fable is left.
        assert_eq!(drip_fit(&lines, 6), 3);
        // Less than one whole account is nothing at all. A partial account is
        // numbers with nothing to say which window any of them belongs to.
        assert_eq!(drip_fit(&lines, 2), 0);
    }

    #[test]
    fn the_rail_never_starves_the_agent_panel() {
        let lines = rail();

        let tall = drip_accounts_split(Rect::new(0, 0, 20, 40), &lines);
        assert!(tall.0.height >= DRIP_AGENT_PANEL_FLOOR);
        // Whatever the split, the two halves tile the area they came from.
        assert_eq!(tall.0.height + tall.1.height, 40);
        // Separator, one row per line, then the footer blank.
        assert_eq!(
            tall.1.height,
            lines.len() as u16 + DRIP_HEADER_ROWS + DRIP_FOOTER_ROWS
        );

        // Tight enough that the third account does not fit. The rail asks for
        // exactly what it will draw -- seven rows, not eleven -- so the agents
        // get the difference back instead of the rail keeping an empty band.
        let tight = drip_accounts_split(Rect::new(0, 0, 20, 17), &lines);
        assert_eq!(tight.1.height, 7 + DRIP_HEADER_ROWS + DRIP_FOOTER_ROWS);
        assert_eq!(tight.0.height + tight.1.height, 17);

        // Too short to give the rail one whole account without starving the
        // agents: the agent panel keeps the area and the rail draws nothing.
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
