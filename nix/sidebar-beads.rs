
// ---------------------------------------------------------------------------
// drip hardcore plugin: the beads rail.
//
// A RE-IMPLEMENTATION of herdr-beads (https://github.com/miiraheart/herdr-beads,
// MIT) for the sidebar. All the credit for the idea -- a `bd` board that lives
// in herdr, addressed the way herdr addresses everything else -- belongs to
// that project, and so does the vocabulary this file speaks: the status set,
// the status glyphs (`○ ◐ ● ❄ ✓ ◇`) and the priority ranking are herdr-beads'
// `src/model.rs`, kept deliberately identical so the two read as the same tool
// to anyone who has used the original. See the README section for the full
// note and for what was changed.
//
// What is NOT re-implemented is its shape. herdr-beads is a ratatui PANE app
// -- three views, a detail popup, keybindings, and writes back to `bd`. This
// is a rail in the sidebar chrome: read-only, one summary line you can click,
// and the bead list under it when you do. Nothing here writes to `bd`.
//
// That pane app is now in this repo too, as a FORK rather than a second
// program to run beside this one: beads/board is herdr-beads' source, and
// beads/board/UPSTREAM.md records the rev it was taken at. The two surfaces
// are one plugin -- the rail is the glance, the board is what a click on it
// opens -- so this note is not history: it is the same project's code in both
// halves, and this half is the one that was written from scratch.
//
// Appended to src/ui/sidebar.rs by nix/herdr-patches.nix. Everything is
// prefixed `drip_` so it cannot collide with anything upstream grows, and it
// reads ONE file -- it starts no process and does not know `bd` exists. The
// drip.beads plugin writes that file; when nothing writes it, every function
// here returns empty and the sidebar is exactly what it was.
//
// The file's format is one line per bead: `<status><priority> <text>`, where
// status is o/i/b/d/c/p/h/- (open, in_progress, blocked, deferred, closed,
// pinned, hooked, unknown) and priority is 0-4 or `-`. Everything after the
// separating space is the row, already laid out by the writer -- same
// deliberately dumb contract the accounts rail has with gumbo, and for the
// same reason: changing how the rail LOOKS must never mean rebuilding herdr.
//
// The rail sits directly ABOVE the accounts rail, which is a carve chained
// onto that one rather than a second opinion about where the agent panel
// ends. See `drip_beads_split` and `drip_beads_bands` for the two halves of
// that arrangement.
// ---------------------------------------------------------------------------

/// Rows above the summary line: the separator alone, matching the accounts
/// rail's opening and, through it, the break between the workspace list and
/// the agents. Under the drip's quiet-chrome patch nothing else in the sidebar
/// announces a section either.
const DRIP_BEADS_HEADER_ROWS: u16 = 1;

/// The summary line itself, which is always drawn when the rail exists -- it
/// is both the rail's whole content when closed and the thing you click to
/// open it.
const DRIP_BEADS_SUMMARY_ROWS: u16 = 1;

/// A file older than this is treated as absent, on the same reasoning as the
/// accounts rail: a watcher that was killed leaves its last frame on disk, and
/// a board that stopped being true an hour ago is worse than no board, because
/// it is the one you would plan against.
const DRIP_BEADS_STALE: std::time::Duration = std::time::Duration::from_secs(3600);

/// How often the file is actually stat'd. The sidebar redraws on every event;
/// a task board changes when someone types a `bd` command.
const DRIP_BEADS_POLL: std::time::Duration = std::time::Duration::from_millis(500);

/// One parsed bead.
#[derive(Clone)]
pub(crate) struct DripBeadLine {
    pub text: String,
    /// herdr-beads' status vocabulary, as one character. See the header.
    pub status: char,
    /// `0`-`4`, or `-` when the writer had none to give.
    pub priority: char,
}

/// Where the beads file lives: `$HERDR_DRIP_BEADS_FILE`, else
/// `$XDG_STATE_HOME/herdr-drip/sidebar-beads.txt`, else the same under
/// `~/.local/state`. The same resolution the plugin's `bin/watch` does, and the
/// same state root the accounts rail and yolo-shell already use.
fn drip_beads_path() -> Option<std::path::PathBuf> {
    if let Some(path) = std::env::var_os("HERDR_DRIP_BEADS_FILE") {
        return Some(std::path::PathBuf::from(path));
    }
    Some(drip_beads_state_dir()?.join("sidebar-beads.txt"))
}

/// `$XDG_STATE_HOME/herdr-drip`, else `~/.local/state/herdr-drip`.
fn drip_beads_state_dir() -> Option<std::path::PathBuf> {
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".local/state"))
        })?;
    Some(state.join("herdr-drip"))
}

/// The rail's beads, re-read at most every [`DRIP_BEADS_POLL`].
pub(crate) fn drip_beads_lines() -> Vec<DripBeadLine> {
    struct Cache {
        checked: Option<std::time::Instant>,
        lines: Vec<DripBeadLine>,
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
    if cache.checked.is_some_and(|at| at.elapsed() < DRIP_BEADS_POLL) {
        return cache.lines.clone();
    }
    cache.checked = Some(std::time::Instant::now());
    cache.lines = drip_read_beads();
    cache.lines.clone()
}

/// Read and parse the file. Every failure -- no path, no file, unreadable,
/// stale, malformed -- is the same answer: no beads, and the sidebar looks like
/// stock herdr.
fn drip_read_beads() -> Vec<DripBeadLine> {
    let Some(path) = drip_beads_path() else {
        return Vec::new();
    };
    let fresh = std::fs::metadata(&path)
        .and_then(|meta| meta.modified())
        .map(|at| at.elapsed().is_ok_and(|age| age <= DRIP_BEADS_STALE))
        .unwrap_or(false);
    if !fresh {
        return Vec::new();
    }
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    text.lines().filter_map(drip_parse_bead).collect()
}

/// `<status><priority> <text>` -> a bead. Anything else is dropped: a
/// half-written frame cannot reach here (the writer renames a temp into place)
/// but a file someone edited by hand can, and a rail is not worth a panic.
///
/// Unlike the accounts rail there is no empty row in this format. A bead with
/// no text is a bead you cannot act on, so it is dropped with the malformed
/// ones rather than drawn as a gap.
fn drip_parse_bead(line: &str) -> Option<DripBeadLine> {
    let mut chars = line.chars();
    let status = chars.next()?;
    let priority = chars.next()?;
    if chars.next() != Some(' ') {
        return None;
    }
    if !matches!(status, 'o' | 'i' | 'b' | 'd' | 'c' | 'p' | 'h' | '-') {
        return None;
    }
    if !matches!(priority, '0'..='4' | '-') {
        return None;
    }
    let text = chars.as_str().trim_end().to_string();
    if text.is_empty() {
        return None;
    }
    Some(DripBeadLine {
        text,
        status,
        priority,
    })
}

/// herdr-beads' status glyphs, verbatim from its `model.rs` -- with one
/// substitution. Its `pinned` is 📌, an emoji, and an emoji is two terminal
/// cells wide: in a rail whose whole budget is the sidebar's 26 columns that
/// costs a column everywhere and misaligns the one row it appears in. `◆` is
/// the same idea in one cell and in the same geometric family as the rest.
fn drip_bead_glyph(status: char) -> &'static str {
    match status {
        'o' => "○",
        'i' => "◐",
        'b' => "●",
        'd' => "❄",
        'c' => "✓",
        'p' => "◆",
        'h' => "◇",
        _ => "•",
    }
}

/// The glyph's colour: what the bead's STATUS says, in the vocabulary the rest
/// of this sidebar already uses for state -- red is the thing stopping you,
/// yellow is the thing running, and a done thing is green.
fn drip_bead_status_color(status: char, p: &Palette) -> ratatui::style::Color {
    match status {
        'b' => p.red,
        'i' => p.yellow,
        'o' => p.blue,
        'c' => p.green,
        'p' => p.mauve,
        'h' => p.teal,
        _ => p.overlay0,
    }
}

/// The text's colour: what the bead's PRIORITY says. Two dimensions in one
/// row, the same trick the accounts rail plays with the account's grade and
/// the window's -- the glyph is what state the work is in, the text is how
/// much it matters, and a P0 that is merely open still reads as a P0.
fn drip_bead_priority_color(priority: char, p: &Palette) -> ratatui::style::Color {
    match priority {
        '0' => p.red,
        '1' => p.peach,
        '2' => p.text,
        '3' => p.subtext0,
        _ => p.overlay0,
    }
}

/// The counts the summary line reports, in the order it reports them:
/// blocked, in progress, open. Everything else -- closed, deferred, pinned,
/// hooked -- is left out on purpose. A summary is read at a glance and answers
/// one question, "what is in my way and what is moving"; the rest of the board
/// is a row away, in the rail this line opens.
pub(crate) fn drip_bead_counts(lines: &[DripBeadLine]) -> [(char, usize); 3] {
    let count = |want: char| lines.iter().filter(|line| line.status == want).count();
    [('b', count('b')), ('i', count('i')), ('o', count('o'))]
}

// ---------------------------------------------------------------------------
// Open / closed.
//
// This lives in a process global rather than in `collapsed_space_keys`, where
// the drip's other collapsible thing (the sidebar's tab tree) keeps its state,
// and the reason is where the state has to be READ from. The carve below runs
// inside herdr's own `expanded_sidebar_sections`, which is handed a `Rect` and
// a ratio and has no `AppState` to ask -- that is the whole point of carving
// there rather than in the renderer, and it is not negotiable, because the
// renderer, the click hit-testing and the scroll metrics all read their
// geometry from that function.
//
// So the flag is a `OnceLock<AtomicBool>` seeded from a file the first time
// anything asks, and written back on every toggle. That buys the same
// across-restart persistence a session key would have, on the same state root
// as the rail's own data, and costs one `read_to_string` per process.
// ---------------------------------------------------------------------------

/// Where the open/closed flag is remembered. Beside the rail's data rather
/// than in herdr's session file, because it is the drip's state and a herdr
/// with these patches removed should not be carrying it around.
fn drip_beads_open_path() -> Option<std::path::PathBuf> {
    if let Some(path) = std::env::var_os("HERDR_DRIP_BEADS_OPEN_FILE") {
        return Some(std::path::PathBuf::from(path));
    }
    Some(drip_beads_state_dir()?.join("sidebar-beads.open"))
}

fn drip_beads_open_cell() -> &'static std::sync::atomic::AtomicBool {
    static OPEN: std::sync::OnceLock<std::sync::atomic::AtomicBool> = std::sync::OnceLock::new();
    OPEN.get_or_init(|| {
        let remembered = drip_beads_open_path()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .is_some_and(|text| text.trim() == "1");
        std::sync::atomic::AtomicBool::new(remembered)
    })
}

/// Whether the rail is showing its beads. Closed is the default, including on
/// a host that has never toggled it: the summary line is the feature, and a
/// rail that opens itself would take rows from the agent panel on first sight
/// of a `.beads` directory.
pub(crate) fn drip_beads_open() -> bool {
    drip_beads_open_cell().load(std::sync::atomic::Ordering::Relaxed)
}

/// Set it without remembering it. The toggle's in-memory half, split out so
/// the tests can pin the flag rather than inherit whatever the box this build
/// runs on last left in its state directory.
fn drip_beads_set_open(open: bool) {
    drip_beads_open_cell().store(open, std::sync::atomic::Ordering::Relaxed);
}

/// Flip it, and remember. Writing is best-effort and silent: a state directory
/// that cannot be created costs the memory of the setting between sessions and
/// nothing else, which is not worth a message in a UI that has nowhere to put
/// one.
pub(crate) fn drip_beads_toggle_open() {
    let next = !drip_beads_open();
    drip_beads_set_open(next);
    let Some(path) = drip_beads_open_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, if next { "1\n" } else { "0\n" });
}

// ---------------------------------------------------------------------------
// Geometry.
// ---------------------------------------------------------------------------

/// Rows the rail wants, given the height it is being carved out of.
///
/// The chrome -- separator plus summary -- is all it asks for when closed, and
/// it asks for nothing at all when there are no beads or when the agent panel
/// cannot spare even that. Open, it asks for a row per bead on top and takes
/// whatever of that fits; a board longer than the sidebar is truncated rather
/// than scrolled, because the rail is a glance and the pane app is the place
/// to read a long board.
/// `open` is passed rather than read so this stays a pure function of the
/// numbers -- the same reason [`drip_accounts_split`] takes its rows instead of
/// fetching them.
fn drip_beads_rail_rows(detail_h: u16, lines: &[DripBeadLine], footer: u16, open: bool) -> u16 {
    if lines.is_empty() {
        return 0;
    }
    let chrome = DRIP_BEADS_HEADER_ROWS + DRIP_BEADS_SUMMARY_ROWS;
    let room = detail_h
        .saturating_sub(DRIP_AGENT_PANEL_FLOOR)
        .saturating_sub(footer);
    if room < chrome {
        return 0;
    }
    let wanted = if open {
        // `try_from` rather than `as`: a board of exactly 65 536 beads would
        // truncate to zero and ask for chrome only, which is the one board
        // length that would silently refuse to open.
        chrome.saturating_add(u16::try_from(lines.len()).unwrap_or(u16::MAX))
    } else {
        chrome
    };
    wanted.min(room)
}

/// One row when the sidebar is collapsed: a glyph in the worst status present.
/// Three columns cannot hold a count, let alone a caret, so the collapsed rail
/// is an indicator and not a control -- it says "there is a board and this is
/// its temperature", and expanding the sidebar is how you read it.
fn drip_beads_rail_rows_collapsed(detail_h: u16, lines: &[DripBeadLine], footer: u16) -> u16 {
    if lines.is_empty() {
        return 0;
    }
    let wanted = DRIP_BEADS_HEADER_ROWS + 1;
    let room = detail_h
        .saturating_sub(DRIP_COLLAPSED_AGENT_FLOOR)
        .saturating_sub(footer);
    if room < wanted {
        return 0;
    }
    wanted
}

/// Whether the beads rail has to keep the sidebar's last row clear itself.
///
/// The accounts rail already reserves it (`expanded_sidebar_toggle_rect` draws
/// `«` there), so when that rail exists the beads rail is not the bottom-most
/// thing and needs no footer of its own. When it does not, the beads rail
/// inherits the job. Passed the accounts rail's own carve rather than its
/// lines, so the answer is "did it actually take rows", which is the question:
/// a rail with accounts to show but no room to show them reserves nothing.
pub(crate) fn drip_beads_footer(accounts: Rect) -> u16 {
    if accounts.height == 0 {
        DRIP_FOOTER_ROWS
    } else {
        0
    }
}

/// Carve the beads rail off the bottom of what the accounts rail left.
///
/// Returns `(agent_area, beads_area)`, the second empty whenever the rail
/// cannot be drawn. Chained after [`drip_accounts_split`] in herdr's own
/// `expanded_sidebar_sections`, which is what puts the beads rail ABOVE the
/// accounts one: each carve takes from the bottom of what it is given, so the
/// second carve lands between the agents and the first.
pub(crate) fn drip_beads_split(detail: Rect, lines: &[DripBeadLine], footer: u16) -> (Rect, Rect) {
    if detail.width == 0 {
        return (detail, Rect::default());
    }
    let rows = drip_beads_rail_rows(detail.height, lines, footer, drip_beads_open());
    if rows == 0 {
        return (detail, Rect::default());
    }
    let split = detail.height - (rows + footer);
    (
        Rect::new(detail.x, detail.y, detail.width, split),
        Rect::new(detail.x, detail.y + split, detail.width, rows),
    )
}

/// The same carve for the collapsed sidebar.
pub(crate) fn drip_beads_split_collapsed(
    detail: Rect,
    lines: &[DripBeadLine],
    footer: u16,
) -> (Rect, Rect) {
    if detail.width == 0 {
        return (detail, Rect::default());
    }
    let rows = drip_beads_rail_rows_collapsed(detail.height, lines, footer);
    if rows == 0 {
        return (detail, Rect::default());
    }
    let split = detail.height - (rows + footer);
    (
        Rect::new(detail.x, detail.y, detail.width, split),
        Rect::new(detail.x, detail.y + split, detail.width, rows),
    )
}

/// Split the band under the agent list into `(beads, accounts)`.
///
/// The band is [`drip_accounts_rect`]'s answer -- everything between the agent
/// list and the row the sidebar keeps for its own toggle -- and it now holds
/// two rails instead of one. Rather than carry a second copy of either rail's
/// arithmetic, this RECONSTRUCTS the height the accounts rail was carved from:
/// the two carves only ever take rows off the bottom, so the detail area
/// before either of them ran was the agents plus this band plus the reserved
/// row, and re-asking the accounts split with that height returns exactly the
/// rows it returned during the carve. What is left of the band above them is
/// ours.
///
/// That is the same discipline `drip_accounts_rect` states for itself: read
/// the boundary back out of the geometry every consumer already agrees on,
/// never recompute it beside them. With one caveat this inherits and that one
/// does not -- it re-reads `drip_accounts_lines()`, so the answer is only as
/// stable as that cache's 500 ms throttle. Two calls in a frame can straddle a
/// refresh; [`drip_beads_rects`] is why the renderer only makes one.
fn drip_beads_bands(section: Rect, agents: Rect, reserved: u16, collapsed: bool) -> (Rect, Rect) {
    let band = drip_accounts_rect(section, agents, reserved);
    if band.height == 0 {
        return (Rect::default(), Rect::default());
    }
    let raw = Rect::new(
        band.x,
        agents.y,
        band.width,
        agents
            .height
            .saturating_add(band.height)
            .saturating_add(reserved),
    );
    let accounts_rows = if collapsed {
        drip_accounts_split_collapsed(raw, &drip_account_dots(&drip_accounts_lines()))
            .1
            .height
    } else {
        drip_accounts_split(raw, &drip_accounts_lines()).1.height
    };
    drip_beads_bands_from(band, accounts_rows, reserved)
}

/// The arithmetic half of [`drip_beads_bands`], with the accounts rail's carve
/// handed in rather than recomputed.
///
/// Separate because this is the part that can be wrong: everything above it is
/// reading numbers back out of functions that already agree, and this is the
/// one place a boundary is decided.
fn drip_beads_bands_from(band: Rect, accounts_rows: u16, reserved: u16) -> (Rect, Rect) {
    // The accounts rail's carve INCLUDES its blank bottom row; its rect stops
    // short of it. Drop the same row here, or the boundary between the rails
    // lands one row low and the beads rail draws over the last account.
    let accounts_h = accounts_rows.saturating_sub(reserved).min(band.height);
    let beads_h = band.height - accounts_h;
    (
        Rect::new(band.x, band.y, band.width, beads_h),
        Rect::new(band.x, band.y + beads_h, band.width, accounts_h),
    )
}

/// Both rails' rects in an expanded sidebar, as `(beads, accounts)`.
///
/// The renderer must use THIS rather than the two singular functions below.
/// [`drip_beads_bands`] reads `drip_accounts_lines()`, which re-stats the
/// accounts file whenever its 500 ms throttle has expired, so two calls in one
/// frame can straddle a refresh and return boundaries computed from different
/// account counts -- rects that overlap by however many rows the accounts rail
/// just grew. Asking once per frame is what makes the pair consistent; the
/// function returning a pair was never enough on its own.
pub(crate) fn drip_beads_rects(section: Rect, split_ratio: f32) -> (Rect, Rect) {
    let (_, agents) = expanded_sidebar_sections(section, split_ratio);
    drip_beads_bands(section, agents, DRIP_FOOTER_ROWS, false)
}

/// The beads rail's rect in an expanded sidebar. For the click hit test, which
/// asks about one rail at one instant and so cannot straddle anything; the
/// renderer wants [`drip_beads_rects`].
pub(crate) fn drip_beads_rect(section: Rect, split_ratio: f32) -> Rect {
    drip_beads_rects(section, split_ratio).0
}

/// Both rails' rects in a collapsed sidebar, for the same one-read-per-frame
/// reason as [`drip_beads_rects`]. There is no singular collapsed variant
/// because there is no collapsed click: three columns hold an indicator, not a
/// control, so the renderer is the only caller.
pub(crate) fn drip_beads_rects_collapsed(section: Rect) -> (Rect, Rect) {
    let (_, _, agents) = collapsed_sidebar_sections(section);
    drip_beads_bands(section, agents, 1, true)
}

/// Whether `(col, row)` is on the summary line of a rail drawn at `rail`.
///
/// The WHOLE line is the target, not the caret alone. The sidebar's other
/// carets (the worktree chevron, the drip's tab tree) sit on rows that already
/// mean something when clicked -- focus that space, focus that tab -- so they
/// have to claim one cell and leave the rest. This row means nothing else, so
/// making the reader aim at a single column would be a hit region chosen to
/// match a convention rather than the row.
pub(crate) fn drip_beads_summary_hit(rail: Rect, col: u16, row: u16) -> bool {
    rail.width > 0
        && rail.height > DRIP_BEADS_HEADER_ROWS
        && row == rail.y + DRIP_BEADS_HEADER_ROWS
        && col >= rail.x
        && col < rail.x + rail.width
}

// ---------------------------------------------------------------------------
// Draw.
// ---------------------------------------------------------------------------

/// Draw the rail: a separator, the summary line, and -- when it is open -- one
/// row per bead. Shaped like the accounts rail below it and like
/// `render_agent_detail`'s own section break, so the three read as one list
/// with three sections rather than as widgets stacked on each other.
pub(crate) fn drip_render_beads(
    app: &AppState,
    frame: &mut Frame,
    area: Rect,
    lines: &[DripBeadLine],
) {
    if area.width == 0 || area.height <= DRIP_BEADS_HEADER_ROWS || lines.is_empty() {
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

    let summary = Rect::new(area.x, area.y + DRIP_BEADS_HEADER_ROWS, area.width, 1);
    let body_y = summary.y + DRIP_BEADS_SUMMARY_ROWS;
    let body_h = area
        .height
        .saturating_sub(DRIP_BEADS_HEADER_ROWS + DRIP_BEADS_SUMMARY_ROWS);

    // The caret reports what is DRAWN, not what the flag says. There are
    // sidebar heights where the carve has room for the chrome and not one bead
    // -- a 34-row terminal at the default split, with the accounts rail below
    // -- and there a caret reading from the flag would turn `▸` to `▾` over an
    // unchanged rail, which reads as the click having failed rather than as
    // there being nowhere to put the answer.
    frame.render_widget(
        Paragraph::new(drip_beads_summary_line(lines, summary.width, body_h > 0, p)),
        summary,
    );

    if body_h == 0 {
        return;
    }
    for (index, line) in lines.iter().take(body_h as usize).enumerate() {
        let row = Rect::new(area.x, body_y + index as u16, area.width, 1);
        let glyph = drip_bead_glyph(line.status);
        let text_width = area
            .width
            .saturating_sub(display_width_u16(glyph).saturating_add(1));
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    format!("{glyph} "),
                    Style::default().fg(drip_bead_status_color(line.status, p)),
                ),
                Span::styled(
                    truncate_end(&line.text, text_width as usize),
                    Style::default().fg(drip_bead_priority_color(line.priority, p)),
                ),
            ])),
            row,
        );
    }
}

/// The summary line: a caret and the word, then the counts pushed to the right
/// edge.
///
/// The word `beads` earns its five columns twice over -- it is what the `bd`
/// command is called, so the row says which tool it is reporting, and it is
/// the label the caret needs to read as a control rather than as decoration on
/// the first row of a list.
///
/// The counts are dropped from the RIGHT when the width runs out, so what
/// survives a narrow sidebar is the blocked count -- the one you would act on.
///
/// `showing` is whether beads are actually on screen beneath this row, which is
/// not the same as the open flag: see the caller.
fn drip_beads_summary_line(
    lines: &[DripBeadLine],
    width: u16,
    showing: bool,
    p: &Palette,
) -> Line<'static> {
    let caret = if showing { "▾" } else { "▸" };
    let label = format!("{caret} beads");
    let mut spans = vec![Span::styled(
        label.clone(),
        Style::default()
            .fg(p.overlay1)
            .add_modifier(if showing {
                Modifier::BOLD
            } else {
                Modifier::empty()
            }),
    )];

    let mut counts: Vec<(String, ratatui::style::Color)> = Vec::new();
    for (status, count) in drip_bead_counts(lines) {
        if count == 0 {
            continue;
        }
        counts.push((
            format!("{}{count}", drip_bead_glyph(status)),
            drip_bead_status_color(status, p),
        ));
    }

    // Fit from the left: keep dropping the last count until the label, a
    // single space and what is left all fit.
    let label_width = display_width(&label);
    while !counts.is_empty() {
        let counts_width: usize = counts
            .iter()
            .map(|(text, _)| display_width(text) + 1)
            .sum();
        if label_width + counts_width <= width as usize {
            break;
        }
        counts.pop();
    }
    if counts.is_empty() {
        return Line::from(spans);
    }

    let counts_width: usize = counts
        .iter()
        .map(|(text, _)| display_width(text) + 1)
        .sum::<usize>()
        - 1;
    let gap = (width as usize).saturating_sub(label_width + counts_width);
    spans.push(Span::raw(" ".repeat(gap)));
    for (index, (text, color)) in counts.into_iter().enumerate() {
        if index > 0 {
            spans.push(Span::raw(" "));
        }
        spans.push(Span::styled(text, Style::default().fg(color)));
    }
    Line::from(spans)
}

/// The collapsed rail: a separator and one glyph, in the worst status the
/// board is carrying. Same three columns a workspace and an agent get up the
/// same rail, saying the same kind of thing.
pub(crate) fn drip_render_beads_collapsed(
    app: &AppState,
    frame: &mut Frame,
    area: Rect,
    lines: &[DripBeadLine],
) {
    if area.width == 0 || area.height < 2 || lines.is_empty() {
        return;
    }
    let p = &app.palette;
    let buf = frame.buffer_mut();
    for x in area.x..area.x + area.width {
        buf[(x, area.y)].set_symbol("─");
        buf[(x, area.y)].set_style(Style::default().fg(p.surface_dim));
    }
    let status = drip_beads_worst(lines);
    frame.render_widget(
        Paragraph::new(Span::styled(
            drip_bead_glyph(status),
            Style::default().fg(drip_bead_status_color(status, p)),
        )),
        Rect::new(area.x, area.y + 1, area.width, 1),
    );
}

/// The status one glyph should show for a whole board: the most urgent one
/// present. Blocked beats in progress beats open beats everything the summary
/// line does not count, on the same reasoning the accounts rail grades an
/// account by its worst window -- of the answers a single cell can give, only
/// the pessimistic one is safe to plan on.
fn drip_beads_worst(lines: &[DripBeadLine]) -> char {
    for want in ['b', 'i', 'o'] {
        if lines.iter().any(|line| line.status == want) {
            return want;
        }
    }
    lines.first().map(|line| line.status).unwrap_or('-')
}

#[cfg(test)]
mod drip_beads_tests {
    use super::*;

    fn bead(status: char, priority: char, text: &str) -> DripBeadLine {
        DripBeadLine {
            text: text.to_string(),
            status,
            priority,
        }
    }

    #[test]
    fn drip_parse_bead_reads_status_and_priority() {
        let parsed = drip_parse_bead("b0 dr-14 turnpike egress").expect("parses");
        assert_eq!(parsed.status, 'b');
        assert_eq!(parsed.priority, '0');
        assert_eq!(parsed.text, "dr-14 turnpike egress");
    }

    #[test]
    fn drip_parse_bead_rejects_junk() {
        // Unknown status letter, unknown priority, missing separator, and a
        // tag with nothing after it -- every one of them a line the rail must
        // drop rather than draw.
        assert!(drip_parse_bead("z0 nope").is_none());
        assert!(drip_parse_bead("o9 nope").is_none());
        assert!(drip_parse_bead("o0nope").is_none());
        assert!(drip_parse_bead("o0 ").is_none());
        assert!(drip_parse_bead("").is_none());
    }

    #[test]
    fn drip_bead_counts_reports_blocked_first() {
        let lines = vec![
            bead('o', '2', "a"),
            bead('b', '0', "b"),
            bead('i', '1', "c"),
            bead('o', '3', "d"),
            bead('c', '2', "e"),
        ];
        assert_eq!(drip_bead_counts(&lines), [('b', 1), ('i', 1), ('o', 2)]);
    }

    #[test]
    fn drip_beads_worst_prefers_blocked() {
        assert_eq!(drip_beads_worst(&[bead('o', '2', "a")]), 'o');
        assert_eq!(
            drip_beads_worst(&[bead('o', '2', "a"), bead('i', '2', "b")]),
            'i'
        );
        assert_eq!(
            drip_beads_worst(&[bead('o', '2', "a"), bead('b', '2', "b")]),
            'b'
        );
        assert_eq!(drip_beads_worst(&[bead('c', '2', "a")]), 'c');
    }

    #[test]
    fn drip_beads_rail_rows_is_nothing_without_beads() {
        assert_eq!(drip_beads_rail_rows(40, &[], 0, false), 0);
        assert_eq!(drip_beads_rail_rows(40, &[], 0, true), 0);
    }

    #[test]
    fn drip_beads_rail_rows_is_the_chrome_when_closed() {
        // Separator plus summary, and nothing else, however many beads there
        // are: closed is closed.
        let lines = vec![bead('o', '2', "a"), bead('b', '0', "b"), bead('i', '1', "c")];
        assert_eq!(drip_beads_rail_rows(40, &lines, 0, false), 2);
    }

    #[test]
    fn drip_beads_rail_rows_adds_a_row_per_bead_when_open() {
        let lines = vec![bead('o', '2', "a"), bead('b', '0', "b")];
        assert_eq!(drip_beads_rail_rows(40, &lines, 0, true), 4);
    }

    #[test]
    fn drip_beads_rail_rows_keeps_the_agent_floor() {
        let lines = vec![bead('o', '2', "a")];
        // Everything the agent panel needs and one row over: too little for a
        // rail once the footer is taken.
        assert_eq!(
            drip_beads_rail_rows(DRIP_AGENT_PANEL_FLOOR + 1, &lines, 1, false),
            0
        );
        // ...and exactly enough without one.
        assert_eq!(
            drip_beads_rail_rows(DRIP_AGENT_PANEL_FLOOR + 2, &lines, 0, false),
            2
        );
    }

    #[test]
    fn drip_beads_rail_rows_truncates_a_long_board() {
        let lines: Vec<DripBeadLine> = (0..40).map(|_| bead('o', '2', "a")).collect();
        let room = 20u16 - DRIP_AGENT_PANEL_FLOOR;
        assert_eq!(drip_beads_rail_rows(20, &lines, 0, true), room);
    }

    #[test]
    fn drip_beads_split_carves_off_the_bottom() {
        drip_beads_set_open(false);
        let lines = vec![bead('o', '2', "a"), bead('b', '0', "b")];
        let detail = Rect::new(0, 10, 25, 20);
        let (agents, rail) = drip_beads_split(detail, &lines, 0);
        assert_eq!(rail.height, 2);
        assert_eq!(agents.height, 18);
        assert_eq!(agents.y, detail.y);
        assert_eq!(rail.y, agents.y + agents.height);
    }

    #[test]
    fn drip_beads_split_reserves_the_footer_when_it_is_bottom_most() {
        drip_beads_set_open(false);
        let lines = vec![bead('o', '2', "a")];
        let detail = Rect::new(0, 0, 25, 20);
        let (agents, rail) = drip_beads_split(detail, &lines, 1);
        assert_eq!(rail.height, 2);
        // The blank row is taken from the agents as well as the rail's own
        // rows, so the rail's last line never shares with the `«` toggle.
        assert_eq!(agents.height, 17);
        assert_eq!(rail.y + rail.height, detail.y + detail.height - 1);
    }

    #[test]
    fn drip_beads_split_leaves_the_agents_alone_when_empty() {
        let detail = Rect::new(0, 10, 25, 20);
        assert_eq!(drip_beads_split(detail, &[], 0), (detail, Rect::default()));
    }

    #[test]
    fn drip_beads_bands_gives_the_whole_band_away_without_accounts() {
        // A host that has never installed gumbo: no account rows, so the beads
        // rail is the only thing in the band and takes all of it.
        let band = Rect::new(0, 12, 25, 3);
        let (beads, accounts) = drip_beads_bands_from(band, 0, DRIP_FOOTER_ROWS);
        assert_eq!(beads, band);
        assert_eq!(accounts.height, 0);
    }

    #[test]
    fn drip_beads_bands_puts_the_beads_above_the_accounts() {
        // Six rows carved for the accounts (four rows, a separator and the
        // blank the rail's rect stops short of) and three for the beads, so
        // the band is eight rows and the boundary sits three rows down.
        let band = Rect::new(0, 12, 25, 8);
        let (beads, accounts) = drip_beads_bands_from(band, 6, DRIP_FOOTER_ROWS);
        assert_eq!(beads, Rect::new(0, 12, 25, 3));
        assert_eq!(accounts, Rect::new(0, 15, 25, 5));
        assert_eq!(beads.y + beads.height, accounts.y);
        assert_eq!(beads.height + accounts.height, band.height);
    }

    #[test]
    fn drip_beads_bands_never_overruns_the_band() {
        // The accounts claiming more than the band holds must leave the beads
        // rail empty rather than a rect with a wrapped-around height.
        let band = Rect::new(0, 12, 25, 3);
        let (beads, accounts) = drip_beads_bands_from(band, 99, DRIP_FOOTER_ROWS);
        assert_eq!(beads.height, 0);
        assert_eq!(accounts, band);
    }

    #[test]
    fn drip_beads_summary_hit_is_the_whole_row() {
        let rail = Rect::new(2, 7, 24, 4);
        let summary = rail.y + DRIP_BEADS_HEADER_ROWS;
        assert!(drip_beads_summary_hit(rail, 2, summary));
        assert!(drip_beads_summary_hit(rail, 25, summary));
        // Not the separator above it, not the beads below it, not the column
        // past the rail's right edge.
        assert!(!drip_beads_summary_hit(rail, 10, rail.y));
        assert!(!drip_beads_summary_hit(rail, 10, summary + 1));
        assert!(!drip_beads_summary_hit(rail, 26, summary));
        assert!(!drip_beads_summary_hit(Rect::default(), 0, 0));
    }
}
