//! DRIP ADDITION. Bead descriptions are markdown; render them as markdown.
//!
//! Upstream put `b.description` into the detail pane as one plain `Span`, which
//! for this repo's boards is close to unreadable: every bead here carries
//! headings, most carry fenced code, and the median description is a few
//! thousand characters of structured prose. A `##` and a ``` are noise when
//! they are not rendered.
//!
//! The renderer is `tui-markdown`, which is the ratatui-native choice: it hands
//! back a plain `Text` we can own, restyle, scroll and compose into the pane we
//! already have, rather than driving the terminal itself the way a crossterm
//! renderer would.
//!
//! ## Why the version is pinned exactly
//!
//! `tui-markdown` 0.3.7 moved to ratatui 0.30 / `ratatui-core` 0.1. The `Text`
//! it returns from there is a DIFFERENT TYPE from the `Text` of the ratatui
//! 0.29 this board is built on, so a `^0.3.6` requirement resolves to 0.3.9 and
//! then does not typecheck (cargo will happily put both ratatui lineages in the
//! tree). `=0.3.6` is the newest release that speaks our ratatui, and the pin is
//! load-bearing rather than cautious: it comes off in the same change that
//! moves the board to ratatui 0.30, and not before.
//!
//! 0.3.6 also has no `from_str_with_options` — the `StyleSheet` API arrived
//! later — so its styling is a fixed set of ANSI-named colours chosen for a
//! default terminal, not for this board: `H1` is a full-width CYAN BACKGROUND
//! and inline code is white-on-black, both of which fight a Catppuccin pane
//! that otherwise keeps its background transparent. Since we own the `Text`
//! once it is handed back, we restyle it on the way past (`retheme`), which is
//! the same thing a StyleSheet would have bought us.
//!
//! The one rule that makes that safe: only ANSI-NAMED colours are remapped.
//! Syntax-highlighted code arrives as `Color::Rgb` from syntect and is passed
//! through untouched, so the two colour systems never collide.
//!
//! ## What it does not do
//!
//! `tui-markdown` describes itself as an experimental proof of concept and it
//! is honest about that. Against this repo's real descriptions the gaps that
//! actually show are:
//!
//!   - TABLES. `from_str` parses with `Options::empty()` plus strikethrough and
//!     tasklists, so `ENABLE_TABLES` is off and a GFM table is never a table —
//!     it stays paragraph text, pipes and all, one source line per rendered
//!     line. That is not a rendered table, but it is not damage either: it is
//!     what the plain-text pane showed, and in a 40-column pane it is arguably
//!     what you want.
//!   - `Rule` (`---`), HTML blocks, images and footnotes are dropped with a
//!     `tracing` warning nobody subscribes to. A horizontal rule silently
//!     vanishes; a `---` under a line of text is a setext heading and survives
//!     as one.
//!
//! Neither is worth hand-rolling `pulldown-cmark` for: the fallback would have
//! to reimplement lists, blockquotes, headings and syntect wiring to buy one
//! table style and one rule glyph, on content where 3 beads of 46 have a table.
//!
//! One gap DID have to be worked around, because it destroys content rather
//! than under-styling it — see `fence_indented_code`.

use crate::ui::theme;
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

/// Markdown → owned ratatui lines, restyled into the board's palette.
///
/// Owned (`'static`) because the pane renders from `App` state that outlives
/// the `&str` this was parsed from — `tui-markdown` borrows its input.
pub fn render(md: &str) -> Vec<Line<'static>> {
    tui_markdown::from_str(&fence_indented_code(md))
        .lines
        .iter()
        .map(own_line)
        .collect()
}

/// Rewrite top-level INDENTED code blocks as fenced ones, because
/// tui-markdown 0.3.6 runs the two through different code paths and only one
/// of them works.
///
/// The bug, precisely: `TextWriter::text` iterates the lines of each `Text`
/// event with `itertools`' `with_position` and starts a new line only for
/// `Middle | Last`. A one-line event is `Only`, which is neither, so its span
/// is appended to the line already open. That is correct and necessary for
/// inline runs (`a `code` b` is three events on one line) — but pulldown emits
/// an INDENTED code block as one event PER LINE, while a FENCED block is a
/// single multi-line event. So
///
/// ```text
///     nix-fmt        2s
///     nix-statix     1s
/// ```
///
/// arrives as two `Only` events and renders as `nix-fmt 2snix-statix 1s`: two
/// lines of a timing table welded into one. Five of this repo's 46 descriptions
/// contain such a block, and it is unreadable in every one.
///
/// The rewrite is done with `pulldown-cmark` itself rather than a regex over
/// indentation, because "four spaces" means code at the top level and a
/// paragraph continuation inside a list, and only the parser knows which it is
/// looking at. `into_offset_iter` hands back the source range of each block, so
/// this is a substitution over ranges the parser chose, not a guess:
///
///   - only blocks OUTSIDE any list are touched. A fence would have to be
///     indented to stay inside a list item, and an indented fence is its own
///     ambiguity;
///   - the fence is bare (no info string), so nothing claims a language syntect
///     would then highlight wrongly. Indented blocks never had one to lose.
///
/// Returns the input untouched (no allocation) when there is nothing to fix,
/// which is the common case.
fn fence_indented_code(md: &str) -> std::borrow::Cow<'_, str> {
    let mut ranges: Vec<std::ops::Range<usize>> = Vec::new();
    let mut list_depth = 0usize;
    for (ev, range) in Parser::new_ext(md, Options::empty()).into_offset_iter() {
        match ev {
            Event::Start(Tag::List(_)) => list_depth += 1,
            Event::End(TagEnd::List(_)) => list_depth = list_depth.saturating_sub(1),
            Event::Start(Tag::CodeBlock(CodeBlockKind::Indented)) if list_depth == 0 => {
                ranges.push(range)
            }
            _ => {}
        }
    }
    if ranges.is_empty() {
        return std::borrow::Cow::Borrowed(md);
    }

    let mut out = String::with_capacity(md.len() + ranges.len() * 8);
    let mut at = 0;
    for r in ranges {
        // The range starts at the block's first CHARACTER, i.e. after its
        // indentation. Back up to the start of that line: an opening fence
        // that inherited those four spaces would itself be indented code.
        let start = md[..r.start].rfind('\n').map_or(0, |i| i + 1);
        if start < at {
            continue; // nested/overlapping: leave it alone rather than corrupt it
        }
        out.push_str(&md[at..start]);
        out.push_str("```\n");
        for line in md[start..r.end].lines() {
            // Indented code is defined as four spaces (or a tab) of padding;
            // strip exactly that much and leave any deeper indent alone, it is
            // part of the code.
            let body = line
                .strip_prefix("    ")
                .or_else(|| line.strip_prefix('\t'))
                .unwrap_or(line);
            out.push_str(body);
            out.push('\n');
        }
        out.push_str("```\n");
        at = r.end;
    }
    out.push_str(&md[at..]);
    std::borrow::Cow::Owned(out)
}

fn own_line(l: &Line<'_>) -> Line<'static> {
    Line {
        spans: l
            .spans
            .iter()
            .map(|s| Span {
                content: s.content.to_string().into(),
                style: retheme(s.style),
            })
            .collect(),
        style: retheme(l.style),
        alignment: l.alignment,
    }
}

/// Map tui-markdown's ANSI-named styling onto the board's palette, and take the
/// backgrounds out. `Color::Rgb` (syntect's highlighting) is left alone.
fn retheme(s: Style) -> Style {
    let mut out = s;
    out.fg = s.fg.map(map_color);
    match s.bg {
        // H1's full-width cyan banner: keep the emphasis, lose the paint.
        Some(Color::Cyan) | Some(Color::LightCyan) => {
            out.bg = None;
            if out.fg.is_none() {
                out.fg = Some(theme::MAUVE);
            }
        }
        // Inline code's white-on-black chip.
        Some(Color::Black) => out.bg = Some(theme::SURFACE0),
        _ => {}
    }
    out
}

fn map_color(c: Color) -> Color {
    match c {
        Color::Cyan => theme::MAUVE,         // h1/h2/h3
        Color::LightCyan => theme::LAVENDER, // h4/h5/h6
        Color::Green => theme::TEAL,         // blockquote
        Color::Blue => theme::BLUE,          // links
        Color::LightBlue => theme::SKY,      // ordered-list markers
        Color::White => theme::PEACH,        // inline code
        other => other,                      // syntect's Rgb, and anything new
    }
}

/// Height this many lines would occupy once `Wrap` has had them, to the nearest
/// line. Used only to stop scrolling at the bottom of the content: ratatui's
/// exact figure (`Paragraph::line_count`) is behind an unstable feature, and
/// word wrapping only ever breaks EARLIER than this counts, so the estimate is
/// low by a line or two on a paragraph and never high.
pub fn wrapped_height(lines: &[Line<'static>], width: u16) -> usize {
    if width == 0 {
        return lines.len();
    }
    let w = width as usize;
    lines.iter().map(|l| l.width().max(1).div_ceil(w)).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real bead description, verbatim from this project's tracker: headings,
    /// a fenced code block, a GFM table, nested bullets, a blockquote, an
    /// ordered list and long prose. It is a fixture rather than a hand-written
    /// sample because the point of the test is that REAL content survives.
    const REAL: &str = include_str!("../tests/fixtures/description.md");

    #[test]
    fn renders_a_real_description_without_losing_it() {
        let lines = render(REAL);
        // Structure, not a golden file: upstream is free to restyle.
        assert!(lines.len() > 40, "got {} lines", lines.len());
        let flat: String = lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        // Prose, a heading, both fenced blocks and a table row made it through.
        assert!(
            flat.contains("the guest simply stops answering"),
            "prose missing"
        );
        assert!(flat.contains("Finding A"), "heading missing");
        assert!(
            flat.contains("tcpdump -i kartbr0 arp"),
            "plain code block missing"
        );
        assert!(flat.contains("UpstreamResponse"), "rust code block missing");
        // A GFM table stays paragraph text (see above), pipes and all — the
        // backticks are gone because the cells ARE styled as inline code.
        assert!(
            flat.contains("| ping <ip> | no route to host |"),
            "table row missing"
        );
        // The HTML comment at the top of the fixture is dropped, not printed.
        assert!(!flat.contains("<!--"), "html block leaked into the pane");
        // Heading markers are rendered, so the source `##` is gone from them.
        assert!(
            lines
                .iter()
                .any(|l| l.spans.first().is_some_and(|s| s.content.starts_with("##"))),
            "no rendered heading marker"
        );
    }

    #[test]
    fn no_ansi_named_colour_survives_rethemeing() {
        for line in render(REAL) {
            for c in std::iter::once(line.style.fg)
                .chain(line.spans.iter().map(|s| s.style.fg))
                .flatten()
            {
                assert!(
                    matches!(c, Color::Rgb(..)),
                    "unthemed colour {c:?} reached the pane"
                );
            }
        }
    }

    #[test]
    fn empty_description_renders_nothing() {
        assert!(render("").is_empty());
    }

    fn flatten(md: &str) -> Vec<String> {
        render(md)
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    }

    /// The bug `fence_indented_code` exists for: without it these three lines
    /// come back welded into one.
    #[test]
    fn an_indented_code_block_keeps_its_lines() {
        let out = flatten(
            "Measured:\n\n    nix-fmt        2s\n    nix-statix     1s\n    nix-deadnix    1s\n\nAfter.\n",
        );
        assert!(out.iter().any(|l| l == "nix-fmt        2s"), "{out:?}");
        assert!(out.iter().any(|l| l == "nix-statix     1s"), "{out:?}");
        assert!(out.iter().any(|l| l == "nix-deadnix    1s"), "{out:?}");
        assert!(out.iter().any(|l| l == "After."), "{out:?}");
    }

    /// Only the four spaces that MAKE it a code block come off; the code's own
    /// indentation is its own.
    #[test]
    fn an_indented_code_block_keeps_its_shape() {
        let out = flatten("x\n\n    fn f() {\n        g();\n    }\n");
        assert_eq!(out, ["x", "", "```", "fn f() {", "    g();", "}", "```"]);
    }

    /// A four-space paragraph under a bullet is a list continuation, not code,
    /// and must not be fenced out of its list.
    #[test]
    fn indentation_inside_a_list_is_left_alone() {
        let md = "- a bullet\n\n    still the bullet\n\n- another\n";
        assert_eq!(fence_indented_code(md), md);
    }

    #[test]
    fn markdown_with_no_indented_code_is_not_copied() {
        let md = "# h\n\nsome text\n\n```\ncode\n```\n";
        assert!(matches!(
            fence_indented_code(md),
            std::borrow::Cow::Borrowed(_)
        ));
    }

    #[test]
    fn wrapped_height_counts_the_wrap() {
        let lines = vec![Line::raw("x".repeat(30)), Line::raw("short")];
        assert_eq!(wrapped_height(&lines, 10), 4);
        assert_eq!(wrapped_height(&lines, 0), 2);
    }
}
