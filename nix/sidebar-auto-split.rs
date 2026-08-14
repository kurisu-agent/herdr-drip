
// ---------------------------------------------------------------------------
// drip hardcore plugin: the section split follows the workspace list.
//
// Appended to src/ui/sidebar.rs by nix/herdr-patches.nix. Stock herdr sizes
// the workspace/agent sections from `sidebar_section_split`, a ratio the user
// drags and the session persists -- which means every workspace opened or
// closed leaves the divider where it was, and the list either scrolls under
// empty agent rows or floats over them until someone drags it back. The
// number the ratio should hold is not an opinion: it is however many rows the
// workspace list needs, and the list already knows.
//
// This computes that ratio. compute_view_internal writes it into
// `app.sidebar_section_split` before any geometry is derived, so every
// consumer -- the renderer, the click hit-testing, the scroll metrics --
// reads the same value they always did and stays consistent for free.
// Dragging the divider still "works" for the frame the drag lands on; the
// next view pass recomputes the field and the divider snaps home, which is
// the cheapest honest way to retire the gesture: the mouse path, and
// herdr's own tests that drive it, are untouched.
// ---------------------------------------------------------------------------

/// The ratio that gives the workspace section exactly the rows its entries
/// need -- header and footer included -- and the agent panel all the rest.
///
/// The row math mirrors `compute_workspace_list_areas`: each entry's
/// `workspace_row_height` (uncapped -- the cap is what we are computing) plus
/// its `workspace_entry_gap`, under `WORKSPACE_SECTION_HEADER_ROWS` and above
/// the one footer row `workspace_list_body_rect` reserves. The result rides
/// through `sidebar_section_heights` unchanged: `round(total * ws_h/total)`
/// is `ws_h` again, and the 3-row clamps there keep both sections alive when
/// the list wants more than the sidebar has -- the workspace list scrolls,
/// exactly as it does in a stock sidebar that is simply too short.
///
/// Below 6 rows `sidebar_section_heights` ignores the ratio entirely, so the
/// current value is returned untouched rather than recomputed for no reader.
pub(crate) fn drip_auto_section_split(app: &AppState, sidebar: Rect) -> f32 {
    let total_h = sidebar.height;
    if total_h < 6 {
        return app.sidebar_section_split;
    }

    let entries = workspace_list_entries(app);
    let mut body_rows: u16 = 0;
    for (entry_idx, entry) in entries.iter().enumerate() {
        let WorkspaceListEntry::Workspace { ws_idx, indented } = entry;
        let Some(ws) = app.workspaces.get(*ws_idx) else {
            continue;
        };
        body_rows = body_rows
            .saturating_add(workspace_row_height(app, ws, *indented))
            .saturating_add(workspace_entry_gap(app, &entries, entry_idx));
    }

    let ws_h = body_rows
        .saturating_add(WORKSPACE_SECTION_HEADER_ROWS)
        .saturating_add(1)
        .clamp(3, total_h.saturating_sub(3));
    f32::from(ws_h) / f32::from(total_h)
}
