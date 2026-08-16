
// ---------------------------------------------------------------------------
// drip hardcore plugin: the click half of the beads rail.
//
// Appended to src/app/input/sidebar.rs by nix/herdr-patches.nix, which is
// where herdr keeps every hit test the sidebar's mouse dispatch asks in order.
// This joins that list rather than inventing any routing of its own, and it
// derives its geometry the way its neighbours do -- from `view.sidebar_rect`
// through `crate::ui`, never from a second copy of the carve.
//
// See nix/sidebar-beads.rs for that geometry and for the credit note.
// ---------------------------------------------------------------------------

impl AppState {
    /// Open or close the beads rail when `(col, row)` is on its summary line;
    /// true when it was.
    ///
    /// Expanded sidebars only. The collapsed rail is one glyph in three
    /// columns with no room to draw what opening it would show, so a click
    /// there falls through to whatever herdr would have done with the row --
    /// which is nothing, since the carve took it out of the agent list.
    pub(super) fn drip_toggle_beads_at(&mut self, col: u16, row: u16) -> bool {
        if self.sidebar_collapsed {
            return false;
        }
        let sidebar = self.view.sidebar_rect;
        if sidebar.width <= 1 || sidebar.height == 0 {
            return false;
        }
        let rail = crate::ui::drip_beads_rect(sidebar, self.sidebar_section_split);
        if !crate::ui::drip_beads_summary_hit(rail, col, row) {
            return false;
        }
        // The flag lives beside the rail's own data rather than in the herdr
        // session (see sidebar-beads.rs for why the carve cannot reach
        // `AppState`), so there is nothing here to mark dirty -- the toggle
        // has already persisted itself.
        crate::ui::drip_beads_toggle_open();
        true
    }
}
