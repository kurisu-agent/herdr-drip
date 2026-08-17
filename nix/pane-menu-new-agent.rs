// pane-menu-new-agent: `New Agent` — a space at $HOME whose one pane is an
// agent. Appended to src/app/input/modal.rs by nix/herdr-patches.nix, which
// routes the row here with one arm at the head of the context-menu dispatcher;
// the label, and where it sits in the menu, are in nix/context-menu-items.rs.
//
// It is `New Space` with the two things that row INHERITS pinned instead, and
// the pinning is the whole reason for a second row:
//
//   - THE CWD is $HOME, not the clicked pane's. `New Space` follows
//     new_terminal_cwd, whose default (Follow) means "wherever the pane you
//     right-clicked is" — right for a space you open to keep working in the
//     same tree, wrong for the one you want when the thought has nothing to do
//     with the repo in front of you. herdr's own `Home` policy answers it
//     rather than a `std::env::var("HOME")` here, so the fallbacks (the
//     process cwd, then `/`) are the ones every other new-terminal cwd in
//     herdr already uses.
//   - THE PANE is an agent. shell-panes makes every new workspace a plain
//     terminal by inserting HERDR_DRIP_PANE_KIND=shell into the launch env of
//     exactly this call — through `entry().or_insert_with()`, so a caller that
//     names the variable itself keeps its own value. Naming it here is
//     therefore both what makes this pane an agent and the only thing stopping
//     shell-panes from making it a shell.
//
// No name prompt, whatever `prompt_new_workspace_name` says — unlike
// `New Space`, which honours it. A space at $HOME already reads `~` from
// herdr's cwd-derived label, which is what this space IS, and the gesture is
// "an agent, now" rather than "set me up somewhere to work". The sidebar's
// `Rename` is still there for the ones that turn into something.
//
// A file rather than two interpolated lines in the nix string, for the reason
// pane-menu-splits.rs is one: the params struct is worth writing where herdr
// growing a field fails as a compile error naming that field.
impl App {
    /// Open a new space at $HOME with an agent in its one pane.
    ///
    /// Nothing about the right-clicked pane is read, which is why the arm that
    /// calls this does not focus it first the way `New Space` does: the cwd is
    /// pinned, so there is no cwd to inherit from the click, and the new space
    /// takes the focus itself.
    pub(crate) fn drip_new_agent_workspace(&mut self) {
        let home = crate::app::creation::resolve_new_terminal_cwd(
            &crate::config::NewTerminalCwdConfig::Home,
            None,
        );
        self.runtime_workspace_create(
            "tui.menu.workspace.create_agent",
            crate::api::schema::WorkspaceCreateParams {
                cwd: Some(home.display().to_string()),
                focus: true,
                label: None,
                env: drip_agent_pane_env(),
            },
        );
    }
}

/// The launch env that says "this pane is an agent": the variable
/// nix/pane-menu-splits.rs sets to `shell`, carrying the other value.
///
/// yolo-shell only ever tests for `shell`, so what does the work here is that
/// the entry EXISTS — shell-panes' `or_insert_with` finds it already filled and
/// leaves it alone. It is spelled positively anyway rather than as an empty
/// map, because an empty map is exactly how the shell default arrives, and a
/// caller whose whole point is the opposite should not be saying nothing.
fn drip_agent_pane_env() -> std::collections::HashMap<String, String> {
    std::collections::HashMap::from([("HERDR_DRIP_PANE_KIND".to_string(), "agent".to_string())])
}
