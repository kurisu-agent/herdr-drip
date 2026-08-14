// ---------------------------------------------------------------------------
// drip hardcore plugin: the workspace row's git status, in the statusline's
// words.
//
// Appended to src/ui/sidebar.rs by nix/herdr-patches.nix. Everything here is
// prefixed `drip_` so it cannot collide with anything upstream grows.
//
// Stock herdr's `git_status` token says exactly one thing about a checkout:
// how far HEAD is from its upstream, `↑4 ↓1`. The claude-drip statusline --
// the row every agent pane in this drip already carries -- says four other
// things about the same checkout: the short HEAD hash, and how many files are
// added, modified and deleted, as bare coloured numbers with no glyph
// (nix-claude-drip, lib/claude.nix `mkStatusBin`). So the sidebar could show
// `↑4` for a space whose pane, two inches to the right, was showing
// `a1c3 2 3` -- two rows describing one working tree in two vocabularies,
// neither of them the other's. This closes that seam from the sidebar's side,
// by making the token speak the pane's:
//
//     main a1c3 3 2 1
//     │    │    │ │ └ deleted   (red)
//     │    │    │ └ modified    (yellow)
//     │    │    └ added         (teal)
//     │    └ short HEAD hash
//     └ branch (stock token, untouched)
//
// The arrow is GONE, not restyled and not folded in: it is herdr's answer to a
// question the statusline does not ask, and nineteen columns shared by two
// dialects is the thing being fixed. herdr's own arrow code is left standing
// in both arms of the match and simply fed a zeroed ahead/behind pair by the
// patch (see herdr-patches.nix), so every branch of it tests a constant zero
// and every term of it multiplies by false. Nothing here draws or measures an
// arrow, and nothing here has to know how herdr would have.
//
// The BRANCH is stock -- already its own token in the row -- and this patch
// does not touch its colour. The hash borrows that token's style rather than
// naming a colour of its own, so branch and hash read as one unit exactly as
// they do in the statusline (which paints both with `BRANCH`), and the
// sidebar's own rule that an unfocused row's branch goes muted keeps applying
// to both halves instead of leaving half the pair bright.
//
// WHY IT CANNOT BE A PLUGIN. A plugin's `$token` is one string with one style,
// so it cannot paint three counts in three colours; and nothing in the plugin
// API can add a field to a sidebar token's data or to how one is measured.
//
// WHAT IT COSTS. Dirty counts exist nowhere in herdr, so this runs
// `git status --porcelain` itself -- NEVER on the draw thread. A draw reads a
// cache; when that answer has gone stale the draw starts a background thread
// and paints the previous one, which is at most `DRIP_GIT_TTL` old. A repo no
// visible row asks about is never asked at all.
// ---------------------------------------------------------------------------

/// How long one repo's answer stands before a draw starts the next read.
///
/// This -- not the frame rate -- is what paces the git processes: herdr
/// redraws the sidebar on every event, and its own remote-status refresh
/// redraws it every 1.5 s regardless, so without a TTL the sidebar would spawn
/// `git status` hundreds of times a second. Five seconds is the trade between
/// "the count moved when I saved the file" and one process per repo per five
/// seconds; a refresh that lands between draws shows up on the next one, which
/// that same 1.5 s tick guarantees arrives.
const DRIP_GIT_TTL: std::time::Duration = std::time::Duration::from_secs(5);

/// The same, for a path that answered nothing -- not a checkout, an empty repo
/// with no HEAD, or no `git` on PATH at all. None of those change on a
/// five-second scale, so a miss is held six times as long. It is also the 30 s
/// herdr's own git status cache holds a non-Git path for.
const DRIP_GIT_MISS_TTL: std::time::Duration = std::time::Duration::from_secs(30);

/// What the drip adds to a `git_status` token: the short HEAD hash and the
/// working tree's counts. The default -- no hash, all zero -- is the "say
/// nothing" answer that renders byte-identically to stock herdr, and it is
/// what a non-git space, a first draw and a poisoned lock all fall back to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DripGitDetails {
    /// `git rev-parse --short=4 HEAD`, exactly what the statusline shows.
    /// Four is git's MINIMUM, not a cap -- git lengthens it when four would be
    /// ambiguous in that repo, and trimming it back would print a hash that
    /// does not resolve -- so the width code measures the string rather than
    /// assuming its length.
    pub hash: Option<String>,
    pub added: usize,
    pub modified: usize,
    pub deleted: usize,
}

impl DripGitDetails {
    /// Nothing to say. Stock drops the whole token when ahead and behind are
    /// both 0; the token now survives that only when this is false.
    pub(crate) fn is_empty(&self) -> bool {
        self.hash.is_none() && self.added == 0 && self.modified == 0 && self.deleted == 0
    }
}

/// Which colour a piece takes. An enum rather than a `Style` so that measuring
/// the token and painting it run the same code path
/// ([`drip_git_status_pieces`]): the width the layout budgets and the spans it
/// later paints cannot drift apart, because they are the same list.
#[derive(Clone, Copy)]
enum DripGitTone {
    Hash,
    Added,
    Modified,
    Deleted,
}

/// The token's pieces, in the statusline's order: what this checkout IS, then
/// what is uncommitted in it. Each is omitted when it is zero -- a clean tree
/// shows only the hash, and a space that is not a repo shows nothing at all,
/// which is what keeps a non-git space looking exactly like stock (stock's own
/// half of the token draws nothing either way now).
fn drip_git_status_pieces(drip: &DripGitDetails) -> Vec<(String, DripGitTone)> {
    let mut pieces = Vec::new();
    if let Some(hash) = drip.hash.as_ref() {
        pieces.push((hash.clone(), DripGitTone::Hash));
    }
    if drip.added > 0 {
        pieces.push((drip.added.to_string(), DripGitTone::Added));
    }
    if drip.modified > 0 {
        pieces.push((drip.modified.to_string(), DripGitTone::Modified));
    }
    if drip.deleted > 0 {
        pieces.push((drip.deleted.to_string(), DripGitTone::Deleted));
    }
    pieces
}

/// The token's width: its pieces and the single spaces between them. It is the
/// WHOLE width, not an addition -- stock's three terms are still summed beside
/// it in the arm, but with a zeroed ahead/behind pair they are three zeroes,
/// so nothing but this measures anything. Nothing trails it either: the last
/// piece is the last thing in the token, so there is no separator to budget
/// for and no stray column at the end of the row.
///
/// This is a FIXED width in `resolved_token_spans`: the row shrinks its
/// flexible tokens (the branch name) before it gives up anything here. That is
/// stock behaviour for `git_status` and it is the right way round -- a
/// truncated branch is still a branch, half a count is a lie -- but it does
/// mean a very dirty tree can push a long branch name out of a narrow sidebar.
/// The counts are 1-2 columns each and only there when non-zero, so the
/// realistic worst case is `a1c3 3 2 1`, 10 of the sidebar's 26.
pub(crate) fn drip_git_status_width(drip: &DripGitDetails) -> usize {
    let pieces = drip_git_status_pieces(drip);
    if pieces.is_empty() {
        return 0;
    }
    let content = pieces
        .iter()
        .map(|(text, _)| display_width(text))
        .sum::<usize>();
    content + pieces.len() - 1
}

/// The token, painted: the pieces and the single spaces between them, and
/// nothing after the last one -- stock's pushes still run in the same arm but
/// have a zeroed pair to test, so this is the entire token and it must not
/// leave a separator hanging for something that is never coming. `hash_style`
/// is the row's BRANCH style, passed in by the caller rather than chosen here
/// (see the header).
///
/// The counts take the statusline's three roles: added is `teal` (its
/// `SUCCESS`, and the same hex -- #94E2D5), modified `yellow`, deleted `red`.
/// All three are palette TOKENS, so the row retints from `[theme.custom]` with
/// a `herdr server reload-config` and no rebuild.
pub(crate) fn drip_git_status_spans(
    drip: &DripGitDetails,
    hash_style: Style,
    p: &Palette,
    patch: crate::config::SidebarTokenStyle,
) -> Vec<Span<'static>> {
    let pieces = drip_git_status_pieces(drip);
    let mut spans = Vec::new();
    for (index, (text, tone)) in pieces.into_iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled(" ", apply_token_style(Style::default(), patch)));
        }
        let style = match tone {
            DripGitTone::Hash => hash_style,
            DripGitTone::Added => Style::default().fg(p.teal),
            DripGitTone::Modified => Style::default().fg(p.yellow),
            DripGitTone::Deleted => Style::default().fg(p.red),
        };
        spans.push(Span::styled(text, apply_token_style(style, patch)));
    }
    spans
}

/// The checkout a workspace's `git_status` token describes.
///
/// `cached_git_status_key` is herdr's own answer to "which working tree is
/// this space": the canonicalised top level of the pane's cwd, computed off
/// the render path and per-WORKTREE rather than per-repo, so two spaces on two
/// linked worktrees of one repo get their own counts instead of both showing
/// the main checkout's. A non-git space carries its plain cwd here, which
/// simply reads as a repo that answers nothing.
pub(crate) fn drip_workspace_repo(ws: &crate::workspace::Workspace) -> Option<std::path::PathBuf> {
    let key = ws.cached_git_status_key.clone();
    (!key.as_os_str().is_empty()).then_some(key)
}

/// One repo's cached answer.
#[derive(Default)]
struct DripGitEntry {
    details: DripGitDetails,
    /// A refresh thread is running for this repo. Keeps a redraw storm from
    /// launching a second one for the same path.
    in_flight: bool,
    /// When the answer goes stale. `None` before the first one lands, which is
    /// what makes a repo's first draw ask.
    good_until: Option<std::time::Instant>,
}

/// Keyed by the repo path, so two spaces on one checkout share a single read.
/// Bounded by the checkouts the sidebar has drawn this session -- a handful of
/// `PathBuf`s -- and never swept, so a space that comes back finds its counts
/// already there instead of a blank row.
fn drip_git_cache(
) -> &'static std::sync::Mutex<std::collections::HashMap<std::path::PathBuf, DripGitEntry>> {
    static CACHE: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<std::path::PathBuf, DripGitEntry>>,
    > = std::sync::OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// What to draw for this repo right now -- and, when that answer has gone
/// stale, a background read for the next draw. Never blocks on git: the lock
/// is held for a map lookup and dropped before the thread is spawned, and the
/// thread takes it again only to write its result.
pub(crate) fn drip_git_details_for(repo: Option<&std::path::Path>) -> DripGitDetails {
    let Some(repo) = repo else {
        return DripGitDetails::default();
    };
    // A poisoned lock means a refresh thread panicked mid-write. These counts
    // are decoration: take the empty answer rather than carry the panic into a
    // draw.
    let Ok(mut cache) = drip_git_cache().lock() else {
        return DripGitDetails::default();
    };
    let entry = cache.entry(repo.to_path_buf()).or_default();
    let details = entry.details.clone();
    let due = !entry.in_flight
        && entry
            .good_until
            .is_none_or(|at| at <= std::time::Instant::now());
    if due {
        entry.in_flight = true;
        let repo = repo.to_path_buf();
        drop(cache);
        std::thread::spawn(move || drip_git_refresh(repo));
    }
    details
}

/// The background half: read, then publish under the lock.
fn drip_git_refresh(repo: std::path::PathBuf) {
    let details = drip_read_git_details(&repo);
    let ttl = if details.hash.is_some() {
        DRIP_GIT_TTL
    } else {
        DRIP_GIT_MISS_TTL
    };
    if let Ok(mut cache) = drip_git_cache().lock() {
        let entry = cache.entry(repo).or_default();
        entry.details = details;
        entry.in_flight = false;
        entry.good_until = Some(std::time::Instant::now() + ttl);
    }
}

/// Two git calls, in the order that makes the second one skippable: no HEAD
/// means no repo (or an empty one), and then there is nothing to count either.
fn drip_read_git_details(repo: &std::path::Path) -> DripGitDetails {
    let Some(hash) = drip_git_output(repo, &["rev-parse", "--short=4", "HEAD"]) else {
        return DripGitDetails::default();
    };
    let hash = hash.trim().to_string();
    if hash.is_empty() {
        return DripGitDetails::default();
    }
    let mut details = DripGitDetails {
        hash: Some(hash),
        ..DripGitDetails::default()
    };
    // --no-optional-locks: `git status` normally rewrites the index to save
    // the stat cache it just refreshed, which takes .git/index.lock. This runs
    // every few seconds, unasked, against trees people are working in -- it
    // must not be able to lose a race with the git command someone actually
    // typed. The flag costs a little repeated work per call and nothing else.
    let Some(status) = drip_git_output(repo, &["--no-optional-locks", "status", "--porcelain"])
    else {
        return details;
    };
    for line in status.lines() {
        let mut chars = line.chars();
        let x = chars.next().unwrap_or(' ');
        let y = chars.next().unwrap_or(' ');
        // The claude-drip statusline's table, transcribed rather than
        // improved. Where the two could differ they must not: the point of the
        // patch is that the two rows agree about one tree. Two consequences
        // worth knowing, both inherited: `??` counts as ADDED (untracked is
        // what you are about to add, and git reports an untracked directory as
        // one entry rather than one per file), and a file that is staged and
        // then edited again (`MM`) matches nothing here and goes uncounted.
        // Fix that in lib/claude.nix first, then here.
        match (x, y) {
            ('?', '?') => details.added += 1,
            ('A', ' ') | (' ', 'A') => details.added += 1,
            ('M', ' ') | ('T', ' ') | (' ', 'M') | (' ', 'T') | ('R', ' ') | ('C', ' ') => {
                details.modified += 1
            }
            ('D', ' ') | (' ', 'D') => details.deleted += 1,
            ('U', 'U') | ('A', 'A') | ('D', 'D') | ('A', 'U') | ('U', 'A') | ('D', 'U')
            | ('U', 'D') => details.deleted += 1,
            _ => {}
        }
    }
    details
}

/// Run git for its stdout, or `None` for any failure at all -- missing binary,
/// not a repo, non-zero exit. The caller's fallback is always "say nothing".
///
/// The stdout comes back raw, deliberately untrimmed: porcelain status encodes
/// the index in column 1 and the working tree in column 2, so a leading space
/// is DATA, and trimming the buffer would turn the first line's ` M` into `M`.
/// `crate::noninteractive_process::command` is herdr's own subprocess builder
/// -- the one its own git calls already go through.
fn drip_git_output(repo: &std::path::Path, args: &[&str]) -> Option<String> {
    let output = crate::noninteractive_process::command("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}
