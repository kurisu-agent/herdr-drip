
// ---------------------------------------------------------------------------
// drip hardcore plugin: the workspace list grows a second tree layer.
//
// Appended to src/ui/sidebar.rs by nix/herdr-patches.nix. Stock herdr's
// workspace list is a tree exactly one level deep: a repo's linked worktrees
// nest under it (`   ├─ branch`) and nothing else nests at all. A workspace's
// TABS -- the thing you actually switch between all day -- are visible only in
// the tab bar of the workspace that happens to be active, so the sidebar can
// tell you a space has work in it and not what the work is.
//
// This nests them: tabs under their workspace, and under a worktree child when
// that is what owns them, so a repo with worktrees reads
//
//     ▾herdr-drip
//      ├╴1
//      ╰╴build
//        ├─ sidebar-tabs
//      ▾ │ ├╴1
//        │ ╰╴agents
//        └─ worktree-b
//          ╰╴1
//
// There is no plugin surface for it. A plugin's sidebar reach is the per-row
// TOKEN list (`$workspace`, `$branch`, `$git_status`), which is one row's
// worth of text for a row herdr has already decided exists -- it cannot add
// rows, cannot nest them, and cannot be clicked. Everything here is row
// geometry, and row geometry in this sidebar is `workspace_row_height` plus
// the four functions that measure against it.
//
// WHAT IS DELIBERATELY NOT DONE, because it is what keeps this patch small:
// `WorkspaceListEntry` is untouched. It is a one-variant enum, and five sites
// across three files destructure it with an irrefutable `let` -- a `Tab`
// variant would be five more anchors and a rewrite of every scroll and index
// path in the list. Instead a workspace's tab rows ride INSIDE its own card:
// `workspace_row_height` returns stock's height plus one row per tab, so the
// entry list, the scroll offsets, `app.selected` and every hit test that keys
// off them stay exactly as they were. The card just gets taller, and taller
// cards are a case all of that code already handles -- a multi-row space row
// is stock behaviour.
//
// The auto-split agrees for free: drip_auto_section_split (sidebar-auto-split.rs)
// sizes the workspace section from `workspace_row_height`, the same function,
// so the divider moves down to make room for tabs without a second opinion
// about how tall they are. `sidebar_section_heights`'s 3-row clamp still
// guarantees the agent panel survives a space with twenty tabs; the list
// scrolls instead, exactly as it does for twenty workspaces.
//
// GLYPH BUDGET. The sidebar is 26 columns, 25 after the separator and 24 with
// a scrollbar. herdr's navigator uses 3-column branches (`├── `, `│  `) which
// at two nesting levels is 9 columns of prefix before a label. These are 2:
// `├╴` and `╰╴`, where U+2574 (a half-width left stub) terminates the branch
// and supplies the gap to the label that a third column would otherwise buy.
// The deepest row -- a tab under a non-last worktree child -- spends
//
//     "   "(3) + "│"(1) + " "(1) + "├╴"(2) = 7 columns
//
// leaving 18 for the tab name at 25 and 17 at 24, and putting the tab's branch
// at columns 5-6 against its parent's at 3-4: the same two-column step the
// worktree level already uses. State is carried by the label's COLOUR rather
// than a status glyph, which is the one signal in this row that costs nothing.
// ---------------------------------------------------------------------------

/// The `collapsed_space_keys` entry standing for one workspace's tab block.
///
/// The set is herdr's own collapse memory -- a `HashSet<String>` of worktree
/// space keys, persisted with the session and consulted only by
/// `contains(&space.key)` -- so a namespaced key of our own rides it without
/// touching anything herdr looks up, and the tab tree remembers its state
/// across a restart for free. The `\u{1f}` separator is a unit separator: no
/// git space key can contain one, so ours can never collide with herdr's.
///
/// The cost, stated: a workspace that is closed while collapsed leaves its key
/// in the persisted set forever. It is a few dozen bytes in the session file
/// and it matches nothing, which is cheaper than a second collapse field on
/// AppState and the four constructors that would have to fill it.
pub(crate) fn drip_tab_tree_key(ws: &crate::workspace::Workspace) -> String {
    format!("drip-tabs\u{1f}{}", ws.id)
}

/// Whether a workspace gets a tab block at all.
///
/// Two or more tabs. One tab under one workspace is a tree with a single child
/// -- a row that repeats what the row above it already said, in a sidebar with
/// no rows to spare. herdr's own agent panel draws the same line for the same
/// reason (`multi_tab || !tab.is_auto_named()`); this is the strict half of
/// it, because a named single tab still says nothing the space row does not.
pub(crate) fn drip_workspace_has_tab_tree(ws: &crate::workspace::Workspace) -> bool {
    ws.tabs.len() >= 2
}

fn drip_tab_tree_collapsed(app: &AppState, ws: &crate::workspace::Workspace) -> bool {
    app.collapsed_space_keys.contains(&drip_tab_tree_key(ws))
}

/// How many rows this workspace's tab block adds to its card.
///
/// Deliberately cheap: `workspace_row_height` is called for every entry by
/// each of the four measuring passes plus the auto-split, so this counts tabs
/// and touches no terminal state. Everything that needs to know what a tab
/// IS -- its name, its agents' state -- is on the render path, which runs once
/// per visible card.
pub(crate) fn drip_tab_rows(app: &AppState, ws: &crate::workspace::Workspace) -> u16 {
    if !drip_workspace_has_tab_tree(ws) || drip_tab_tree_collapsed(app, ws) {
        return 0;
    }
    ws.tabs.len().min(u16::MAX as usize) as u16
}

/// The one cell that collapses a tab block: the leading column of the
/// workspace's own first row.
///
/// Stock leaves it blank in both cases -- `" "` before a root workspace's
/// label, `"   "` before a worktree child's branch -- so the caret costs no
/// columns and shifts no text. It cannot collide with herdr's worktree-group
/// chevron, which lives in the LAST column of the same row.
pub(crate) fn drip_tab_tree_caret_rect(card: &crate::app::state::WorkspaceCardArea) -> Rect {
    if card.rect.width <= 2 || card.rect.height == 0 {
        return Rect::default();
    }
    Rect::new(
        card.rect.x + u16::from(card.indented),
        card.rect.y,
        1,
        1,
    )
}

/// The cards to hit-test against, the way every other sidebar hit test gets
/// them: the drawn geometry when there is one, recomputed when the view has
/// not been built yet (the case herdr's own `workspace_at_row` guards for).
fn drip_workspace_cards(app: &AppState) -> Vec<crate::app::state::WorkspaceCardArea> {
    if app.view.workspace_card_areas.is_empty() {
        compute_workspace_card_areas(app, app.view.sidebar_rect)
    } else {
        app.view.workspace_card_areas.clone()
    }
}

/// The collapse key of the tab block whose caret is at `(col, row)`, if any.
pub(crate) fn drip_tab_tree_caret_at(app: &AppState, col: u16, row: u16) -> Option<String> {
    for card in drip_workspace_cards(app) {
        let Some(ws) = app.workspaces.get(card.ws_idx) else {
            continue;
        };
        if !drip_workspace_has_tab_tree(ws) {
            continue;
        }
        let caret = drip_tab_tree_caret_rect(&card);
        if caret.width > 0 && caret.x == col && caret.y == row {
            return Some(drip_tab_tree_key(ws));
        }
    }
    None
}

/// The `(ws_idx, tab_idx)` a sidebar row addresses, if that row is a tab row.
///
/// A card's tab rows are the ones after its stock rows, so the bound is
/// computed from `drip_stock_workspace_row_height` rather than from
/// `card.rect.height - tab_count`: on a card the body truncated, the stock
/// rows are the ones that survive, and this stays right about which rows are
/// which either way.
pub(crate) fn drip_tab_tree_target_at(app: &AppState, row: u16) -> Option<(usize, usize)> {
    for card in drip_workspace_cards(app) {
        let Some(ws) = app.workspaces.get(card.ws_idx) else {
            continue;
        };
        let tab_count = drip_tab_rows(app, ws);
        if tab_count == 0 {
            continue;
        }
        let first = card
            .rect
            .y
            .saturating_add(drip_stock_workspace_row_height(app, ws, card.indented));
        let end = card
            .rect
            .y
            .saturating_add(card.rect.height)
            .min(first.saturating_add(tab_count));
        if row >= first && row < end {
            return Some((card.ws_idx, usize::from(row - first)));
        }
    }
    None
}

/// The attention state of each tab, folded from its panes with herdr's own
/// priority order -- the same one `space_aggregate_state` uses to decide what
/// a collapsed worktree group reports.
fn drip_tab_states(app: &AppState, ws: &crate::workspace::Workspace) -> Vec<(AgentState, bool)> {
    let mut states = vec![(AgentState::Unknown, true); ws.tabs.len()];
    for detail in ws.pane_details(&app.terminals) {
        let Some(slot) = states.get_mut(detail.tab_idx) else {
            continue;
        };
        if workspace_attention_priority(detail.state, detail.seen)
            > workspace_attention_priority(slot.0, slot.1)
        {
            *slot = (detail.state, detail.seen);
        }
    }
    states
}

/// Draw the caret, then the tab rows, in the space `workspace_row_height`
/// reserved for them under the card's stock rows.
///
/// Where those rows START is taken from `drip_stock_workspace_row_height` and
/// not from the renderer's own row vector, so this and `drip_tab_tree_target_at`
/// read the identical number and a click can never land on a row other than
/// the one it is pointing at.
///
/// `is_last_child` is the caller's -- it is what decides whether the worktree
/// group's `│` continues down the left of these rows, and the renderer has
/// already computed it for stock's own continuation rows.
pub(crate) fn drip_render_tab_rows(
    app: &AppState,
    frame: &mut Frame,
    card: &crate::app::state::WorkspaceCardArea,
    list_bottom: u16,
    is_last_child: bool,
) {
    let Some(ws) = app.workspaces.get(card.ws_idx) else {
        return;
    };
    if !drip_workspace_has_tab_tree(ws) {
        return;
    }

    let p = &app.palette;
    let collapsed = drip_tab_tree_collapsed(app, ws);
    let caret = drip_tab_tree_caret_rect(card);
    if caret.width > 0 && caret.y < list_bottom {
        frame.render_widget(
            Paragraph::new(Span::styled(
                if collapsed { "▸" } else { "▾" },
                Style::default().fg(p.overlay0),
            )),
            caret,
        );
    }
    if collapsed {
        return;
    }

    let states = drip_tab_states(app, ws);
    let stock_rows = drip_stock_workspace_row_height(app, ws, card.indented);
    let card_bottom = card.rect.y.saturating_add(card.rect.height);
    let last_tab = ws.tabs.len().saturating_sub(1);
    let workspace_is_active = app.active == Some(card.ws_idx);

    for tab_idx in 0..ws.tabs.len() {
        let y = card
            .rect
            .y
            .saturating_add(stock_rows)
            .saturating_add(tab_idx.min(u16::MAX as usize) as u16);
        if y >= card_bottom || y >= list_bottom {
            break;
        }

        let mut spans = Vec::new();
        let prefix_width = if card.indented {
            spans.push(Span::raw("   "));
            spans.push(Span::styled(
                if is_last_child { " " } else { "│" },
                Style::default().fg(p.overlay0),
            ));
            spans.push(Span::raw(" "));
            7
        } else {
            spans.push(Span::raw(" "));
            3
        };
        spans.push(Span::styled(
            if tab_idx == last_tab { "╰╴" } else { "├╴" },
            Style::default().fg(p.overlay0),
        ));

        let (state, seen) = states
            .get(tab_idx)
            .copied()
            .unwrap_or((AgentState::Unknown, true));
        let is_active_tab = ws.active_tab == tab_idx;
        let label_style = Style::default()
            .fg(state_label_color(state, seen, p))
            .add_modifier(if is_active_tab {
                Modifier::BOLD
            } else {
                Modifier::DIM
            });
        let label = ws
            .tab_display_name(tab_idx)
            .unwrap_or_else(|| (tab_idx + 1).to_string());
        spans.push(Span::styled(
            truncate_end(&label, card.rect.width.saturating_sub(prefix_width) as usize),
            label_style,
        ));

        if is_active_tab && workspace_is_active {
            let buf = frame.buffer_mut();
            for x in card.rect.x..card.rect.x + card.rect.width {
                buf[(x, y)].set_style(Style::default().bg(p.surface1));
            }
        }

        frame.render_widget(
            Paragraph::new(Line::from(spans)),
            Rect::new(card.rect.x, y, card.rect.width, 1),
        );
    }
}
