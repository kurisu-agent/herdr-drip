
// ---------------------------------------------------------------------------
// drip hardcore plugin: the click half of the sidebar's tab tree.
//
// Appended to src/app/input/sidebar.rs by nix/herdr-patches.nix, which is
// where herdr keeps every `*_at_row` the sidebar's mouse dispatch asks in
// order. These two join that list rather than inventing any routing of their
// own; mouse.rs asks them ahead of `workspace_at_row`, because a tab row lives
// INSIDE its workspace's card and that function would otherwise answer for it.
//
// See nix/sidebar-tab-tree.rs for the geometry both of these read.
// ---------------------------------------------------------------------------

impl AppState {
    /// The pane a sidebar tab row should focus, if `row` is one.
    ///
    /// A tab is addressed by its focused pane rather than by index because
    /// that is the target herdr already knows how to reach across workspaces:
    /// `MouseAction::FocusPane` goes to `focus_pane_internal_via_api`, the
    /// same route the agent panel's rows take, and it brings the workspace,
    /// the tab and the pane along with it. `MouseAction::FocusTab` cannot --
    /// it is the tab bar's action and only fires for the active workspace.
    pub(super) fn drip_sidebar_tab_target_at(
        &self,
        row: u16,
    ) -> Option<(usize, crate::layout::PaneId)> {
        let (ws_idx, tab_idx) = crate::ui::drip_tab_tree_target_at(self, row)?;
        let pane_id = self
            .workspaces
            .get(ws_idx)?
            .tabs
            .get(tab_idx)?
            .layout
            .focused();
        Some((ws_idx, pane_id))
    }

    /// Toggle the tab block whose caret is at `(col, row)`; true when one was.
    ///
    /// The collapse lives in `collapsed_space_keys` under a key of our own, so
    /// this is the same insert/remove pair herdr's worktree-group chevron
    /// runs, against the same set, and gets the same persistence.
    pub(super) fn drip_toggle_tab_tree_at(&mut self, col: u16, row: u16) -> bool {
        let Some(key) = crate::ui::drip_tab_tree_caret_at(self, col, row) else {
            return false;
        };
        if !self.collapsed_space_keys.remove(&key) {
            self.collapsed_space_keys.insert(key);
        }
        self.mark_session_dirty();
        true
    }
}
