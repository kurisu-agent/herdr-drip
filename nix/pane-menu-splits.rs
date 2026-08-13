// pane-menu-splits: the shell half of the pane context menu's split entries.
// Appended to src/app/input/modal.rs by nix/herdr-patches.nix, whose two
// one-line anchors widen the existing "Split right" / "Split down" arms to
// accept both of that direction's labels and route them here. The labels
// themselves are in nix/pane-menu-labels.rs, which also fixes their order in
// the menu; the `shell` argument below is just which of the pair was clicked.
//
// All this adds over herdr's own `split_focused_pane_via_api` is the launch
// env: PaneSplitParams already carries an `env` map to the spawned pane (that
// is how `herdr pane split --env` works), so a split that wants a plain shell
// is the ordinary split with one variable set. yolo-shell reads it; a host
// whose default_shell is a real shell gets one unused variable and no change
// in behaviour.
//
// It lives in a file rather than inline in the nix string because the params
// struct is worth writing once: herdr growing a field breaks THIS as a compile
// error naming the missing field, which is a better failure than two long
// interpolated lines that have drifted apart.
impl App {
    /// Split the focused pane, optionally marking the new pane as a plain
    /// shell rather than an agent.
    pub(crate) fn drip_split_focused_pane(
        &mut self,
        direction: crate::api::schema::SplitDirection,
        shell: bool,
    ) {
        self.runtime_pane_split(
            "tui.pane.split",
            crate::api::schema::PaneSplitParams {
                workspace_id: None,
                target_pane_id: None,
                direction,
                ratio: None,
                cwd: None,
                focus: true,
                right_click: Default::default(),
                env: if shell {
                    drip_shell_pane_env()
                } else {
                    Default::default()
                },
            },
        );
    }
}

/// The one variable that tells the drip's `default_shell` this pane is a
/// terminal, not an agent. Also used by the workspace/tab creation patches in
/// nix/herdr-patches.nix, which build the same pair by hand — this is the
/// spelling they must agree on.
fn drip_shell_pane_env() -> std::collections::HashMap<String, String> {
    std::collections::HashMap::from([("HERDR_DRIP_PANE_KIND".to_string(), "shell".to_string())])
}
