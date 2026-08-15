
// ---------------------------------------------------------------------------
// drip hardcore plugin: the agent panel's scope icon.
//
// Appended to src/app/state.rs by nix/herdr-patches.nix -- state.rs and not
// ui/sidebar.rs because BOTH sides need this: the renderer draws the glyph and
// `app::input` hit-tests and toggles it. `app::state` is a `pub mod` and
// `app::input` is private, so state.rs is the only one of the three either
// side can name. Same reasoning as pane-menu-labels.rs.
//
// The scope itself lives in drip.agent-scope (the plugin). This side owns the
// GESTURE -- one cell in the agent panel's header that shows which scope is on
// and flips it when clicked -- and the two meet at one file, exactly the way
// sidebar-accounts.rs and drip.gumbo-usage meet at the accounts file. Neither
// knows anything else about the other: the plugin never hears about the click,
// this never hears about the plugin.
// ---------------------------------------------------------------------------

/// The agent view this drip owns. Shared verbatim with the plugin's
/// `bin/agent-view.js`, because a view is cleared BY SOURCE -- either side
/// must be able to clear what the other set, and a source only they two use
/// is what keeps them from clearing anybody else's.
pub const DRIP_SCOPE_SOURCE: &str = "plugin:drip.agent-scope";

// The pair, and why these two: the pane menu already gives `New Space` a
// window and `New Tab` two of them, so in this drip a window has meant a space
// since that patch landed. One window is this space, more than one is every
// space -- existing vocabulary, no new glyph to learn.
//
// Both are Codicons (U+EA60-U+EBEB), which every Nerd Font since v2.3 carries
// -- deliberately NOT the Material Design plane sidebar-version's drop lives
// in, whose codepoints moved wholesale in v3 and render as tofu on an older
// patched font. A one-column cell has nothing to degrade into.
//
// Both are single width (`unicode_width` reports 1 for the BMP private use
// area), which is what lets the hit rect size itself from either one.

/// This space only: one window.
pub const DRIP_SCOPE_GLYPH_CURRENT: &str = "\u{eb7f}"; // cod-window

/// Every space: more than one window.
pub const DRIP_SCOPE_GLYPH_ALL: &str = "\u{eb23}"; // cod-multiple_windows

/// Where the scope is remembered: `$HERDR_DRIP_AGENT_SCOPE_FILE`, else
/// `$XDG_STATE_HOME/herdr-drip/agent-scope`, else the same under
/// `~/.local/state`. The same three-step resolution -- and the same state root
/// -- as the accounts rail's file and yolo-shell's pane records.
fn drip_scope_path() -> Option<std::path::PathBuf> {
    if let Some(path) = std::env::var_os("HERDR_DRIP_AGENT_SCOPE_FILE") {
        return Some(std::path::PathBuf::from(path));
    }
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".local/state"))
        })?;
    Some(state.join("herdr-drip").join("agent-scope"))
}

/// Remember the scope for the next server. Best effort and silent: the live
/// toggle has already happened by the time this runs, so a read-only state dir
/// costs the user the memory of the choice, not the choice itself. Nothing
/// here is on a render path -- it runs on a click.
fn drip_write_scope(scope: &str) {
    let Some(path) = drip_scope_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, format!("{scope}\n"));
}

impl AppState {
    /// Whether the agent panel is currently scoped to this space BY US.
    ///
    /// By source, not merely "some view is active": another plugin's view is
    /// its own business, and answering yes to it would make the icon claim a
    /// state this drip is not in and the click clear something it does not own.
    pub(crate) fn drip_scope_is_current(&self) -> bool {
        self.agent_view_override
            .as_ref()
            .is_some_and(|view| view.source == DRIP_SCOPE_SOURCE)
    }

    /// The glyph for the cell: which scope is on, in one column.
    pub(crate) fn drip_scope_glyph(&self) -> &'static str {
        if self.drip_scope_is_current() {
            DRIP_SCOPE_GLYPH_CURRENT
        } else {
            DRIP_SCOPE_GLYPH_ALL
        }
    }

    /// Flip the scope, and remember it.
    ///
    /// The two arms are the same pair of API calls the plugin makes over the
    /// socket (`agent.view.set` with this filter, `agent.view.clear` on this
    /// source) -- `handle_agent_view_set` is validation plus
    /// `replace_agent_view_override`, and that is a field and two scroll
    /// resets, all three of them right here. So the click and the action reach
    /// the identical state without this side needing an `App`, which the mouse
    /// handler does not have.
    ///
    /// "Every space" CLEARS the view rather than filtering for everything: an
    /// inactive view is stock herdr, which is what every space means.
    pub(crate) fn drip_toggle_agent_scope(&mut self) {
        if self.drip_scope_is_current() {
            self.agent_view_override = None;
            drip_write_scope("all");
        } else {
            self.agent_view_override = Some(crate::api::schema::AgentViewSetParams {
                source: DRIP_SCOPE_SOURCE.to_string(),
                // No label: the glyph IS the label, and a labelled view would
                // print its label in this very cell (see the renderer's
                // `.filter(|label| !label.is_empty())`).
                label: None,
                filter: Some(crate::api::schema::AgentViewFilter::Eq {
                    field: crate::api::schema::AgentViewField::Builtin(
                        crate::api::schema::AgentViewBuiltinField::WorkspaceId,
                    ),
                    value: crate::api::schema::AgentViewValue::Context {
                        context: crate::api::schema::AgentViewContext::CurrentWorkspaceId,
                    },
                }),
                sort: Vec::new(),
            });
            drip_write_scope("current");
        }
        // What replace_agent_view_override does after the swap, for the same
        // reason: the list under the header is now a different list, and an
        // offset into the old one means nothing.
        self.agent_panel_scroll = 0;
        self.mobile_switcher_scroll = 0;
    }
}
