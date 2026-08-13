# Hardcore plugins — the drip's source patches on herdr itself.
#
# A regular plugin lives in a top-level directory here and talks to herdr
# through its plugin surface. A HARDCORE plugin is for the things herdr has
# no surface for at all: it patches herdr's source and rides the build. Same
# curation idea as the rest of the drip — our opinions about how herdr
# should behave, in one repo — just applied one layer down.
#
# Rules of the set:
#   - One function over the herdr package, so every consumer (the
#     nix-claude-drip herdr knob, a host pinning its own herdr build)
#     applies the identical set: `herdr-drip.lib.patchHerdr herdrPkg`.
#     The `rev` argument is NOT part of that contract — flake.nix applies it
#     ahead of time (`import ./nix/herdr-patches.nix { rev = self.rev or null; }`),
#     so what consumers hold stays the same one-argument function it was.
#     Anything importing this file directly supplies its own rev, or none.
#   - Every patch FAILS LOUDLY when upstream moves: `substituteInPlace
#     --replace-fail` (or a context patch) errors the build rather than
#     silently no-opping, so a herdr bump can never shed a patch without
#     someone reading why.
#   - Each patch carries its story: what it changes, and why it cannot be a
#     real plugin. When herdr grows a surface for one, the patch graduates
#     into a plugin directory and leaves this file.
#   - Applying the set twice is a build error by construction (the
#     --replace-fail no longer matches) — a host overriding
#     `services.claude-code.herdr.package` supplies an UNPATCHED build and
#     lets the module patch it.
{
  rev ? null,
}:
herdrPkg:
let
  # The drip's own commit, for the sidebar-version patch below. Six chars is
  # what the user asked for and plenty to name a commit in a repo this size;
  # `self.rev` is a full 40, and `self.shortRev` is 7, so neither is usable
  # as-is. Rendered with a LEADING space so the empty case concatenates to
  # exactly the old string rather than a trailing one.
  #
  # No rev means a dirty checkout — the same condition nix/plugins.nix warns
  # about and skips its installs for. Here it degrades quietly instead: a
  # working tree has no commit to name, and a build failure (or the word
  # "dirty" in the chrome of every dev shell) would be a poor trade for
  # information the developer already has. The header just reads " herdr
  # 0.8.0", as it did before this patch learned about revs.
  dripRev = if rev == null then "" else " " + builtins.substring 0 6 rev;
in
herdrPkg.overrideAttrs (old: {
  postPatch = (old.postPatch or "") + ''
    # sidebar-version: the sidebar's workspace-list header hardcodes the
    # label " spaces" — pure redundancy over a list that is visibly spaces.
    # Herdr has no plugin surface for sidebar chrome, so render the running
    # herdr version there instead, which the UI otherwise shows nowhere.
    #
    # The drip's rev rides along, because the herdr version alone does not
    # identify what is on screen: the patches below can change the sidebar
    # while CARGO_PKG_VERSION sits still at 0.8.0, and then "which build is
    # this?" has no answer anywhere in the UI. Two words, one line:
    # `herdr 0.8.0 85c510`.
    #
    # It stays a `concat!` of literals, so the whole label is still resolved
    # at COMPILE time and the anchor keeps its shape — the rev arrives as a
    # nix interpolation into the replacement text, not as a runtime lookup.
    # The row is a 1-line Paragraph at the sidebar's full width (26 by
    # default), so it truncates rather than wraps; 19 columns leaves room.
    substituteInPlace src/ui/sidebar.rs \
      --replace-fail '" spaces",' 'concat!(" herdr ", env!("CARGO_PKG_VERSION"), "${dripRev}"),'

    # sidebar-accounts: a rail under the agent list showing each Anthropic
    # account's headroom and reset — how much time is left, per account, where
    # you are already looking. herdr's plugin surface cannot reach it: pane
    # placements are overlay/popup/split/tab/zoomed (no sidebar), and the
    # sidebar's own row tokens are per-workspace and per-agent, with nothing
    # global under them.
    #
    # The patched code reads ONE file and nothing else — no process, no
    # credential, no knowledge of gumbo — so the thing it displays stays
    # replaceable: drip.gumbo-usage feeds it with `gumbo watch --format compact
    # --tags`, and with nothing feeding it every function returns empty and the
    # sidebar is byte-identical to stock. See sidebar-accounts.rs for the
    # format and the layout rules.
    cat ${./sidebar-accounts.rs} >> src/ui/sidebar.rs

    # The carve goes in herdr's OWN section geometry, not in the draw. Both
    # `*_sidebar_sections` are what every consumer asks where the agent panel
    # is — the renderer, the click hit-testing (app/input/sidebar.rs
    # `agent_panel_rect`), the scroll metrics — so taking the rows here is what
    # keeps a click on an account row from selecting the agent that used to be
    # drawn under the pointer. Carving in the renderer alone would look right
    # and mis-route every click on the rail.
    # Every anchor below is ONE line, replaced by one line. Nix's indented
    # strings strip the block's common leading whitespace, which silently eats
    # the indentation of any continuation line inside a pattern — and a pattern
    # that no longer matches is a build failure at best. Statements separated by
    # `;` on a single line cost nothing here: rustfmt never sees this.
    substituteInPlace src/ui/sidebar.rs \
      --replace-fail '    let detail_area = Rect::new(content.x, content.y + ws_h, content.width, detail_h);' '    let detail_area = Rect::new(content.x, content.y + ws_h, content.width, detail_h); let (detail_area, _) = drip_accounts_split(detail_area, &drip_accounts_lines());'

    substituteInPlace src/ui/sidebar.rs \
      --replace-fail '    let detail_area = Rect::new(content.x, divider_y + 1, content.width, detail_h as u16);' '    let detail_area = Rect::new(content.x, divider_y + 1, content.width, detail_h as u16); let (detail_area, _) = drip_accounts_split_collapsed(detail_area, &drip_account_dots(&drip_accounts_lines()));'

    # Expanded: draw the rail in the rows the carve left, under the agents.
    # `DRIP_FOOTER_ROWS` rather than 0 keeps the rail off the sidebar's last
    # row — which is both a blank line under the accounts and the row
    # `expanded_sidebar_toggle_rect` draws `«` into. The collapsed call below
    # has always reserved it; this one had not.
    substituteInPlace src/ui/sidebar.rs \
      --replace-fail '    render_sidebar_toggle(app, frame, area, false, p);' '    drip_render_accounts(app, frame, drip_accounts_rect(area, detail_area, DRIP_FOOTER_ROWS), &drip_accounts_lines()); render_sidebar_toggle(app, frame, area, false, p);'

    # Collapsed: the same rail reduced to what three columns hold — one
    # numbered row and one traffic-light dot per account, under the agent dots.
    # Drawn before the agent rows rather than after, because the toggle call it
    # would otherwise anchor on appears twice (the early return draws it too)
    # and this spot appears once.
    substituteInPlace src/ui/sidebar.rs \
      --replace-fail '    let detail_content_area = Rect::new(' '    drip_render_accounts_collapsed(app, frame, drip_accounts_rect(area, detail_area, 1), &drip_account_dots(&drip_accounts_lines())); let detail_content_area = Rect::new('

    # last-close-quits: closing the last pane stops the server, instead of
    # leaving an empty herdr running. Stock herdr treats "no workspaces left"
    # as a state to sit in — the sidebar empties and the server keeps its
    # session, its sockets and, crucially, its OLD BINARY. That makes the
    # obvious reload gesture impossible: a rebuild reaches the UI only after
    # the server restarts, and nothing short of `herdr server stop` from
    # another terminal restarts it. With this, closing your way out of herdr
    # is the reload — the next `herdr` is a new server on the new binary.
    #
    # There is no plugin surface for it. `[[events]]` cannot extend or veto a
    # close, and the decision is a field on AppState that the API only ever
    # sets from `server stop` (Method::ServerStop -> state.should_quit).
    # Setting that same field gives the identical shutdown: the headless loop
    # sees should_quit, sends ServerShutdown to every client, removes the
    # sockets and exits, so the terminal is restored like any clean quit.
    #
    # Two anchors, because herdr empties the workspace list two ways and
    # "closed the last shell" means both:
    #   - close_selected_workspace — the close pane/tab/workspace commands,
    #     which all funnel here once the close takes the workspace with it.
    #   - handle_pane_died — the pane's process exiting on its own (Ctrl-D,
    #     `exit`, the agent quitting), which removes the workspace inline
    #     rather than calling the above.
    # Both anchors sit INSIDE that function's `if self.workspaces.is_empty()`
    # arm, so nothing fires while a single pane is left anywhere, and neither
    # can fire on a server that has not opened a workspace yet — the arms run
    # on a close, not on an empty list.
    substituteInPlace src/app/actions.rs \
      --replace-fail '            self.workspace_scroll = 0;' '            self.workspace_scroll = 0; self.should_quit = true;'

    substituteInPlace src/app/actions.rs \
      --replace-fail '                if self.mode == Mode::Terminal {' '                self.should_quit = true; if self.mode == Mode::Terminal {'

    # shell-panes: a new workspace or tab opens a TERMINAL, not an agent.
    # Stock herdr spawns every new pane with `default_shell`, and the drip's
    # default_shell is yolo-shell, which starts claude — so the gestures that
    # mean "give me somewhere to work" (the sidebar's +, the new-workspace and
    # new-tab keys, the name prompt, opening a worktree) each burned a claude
    # session before you had decided you wanted one. Splits still start the
    # agent, which is where the decision belongs.
    #
    # The mechanism is herdr's own: every pane-spawning API call already
    # carries a launch `env` map to the new pane (`herdr pane split --env`,
    # `workspace create --env`), so this sets ONE variable and yolo-shell
    # reads it. Nothing here knows what a shell is, and a host whose
    # default_shell is a real shell inherits one unused variable and behaves
    # exactly as stock — the same replaceability rule sidebar-accounts follows.
    #
    # Three anchors on two paths:
    #   - runtime_workspace_create / runtime_tab_create are the TUI's own
    #     mutation calls (`tui.workspace.create`, `tui.workspace.create_named`,
    #     `tui.workspace.create_cwd`, `tui.tab.create`), so patching the two
    #     dispatch lines covers every workspace and tab the UI creates while
    #     leaving the API's own `workspace.create` alone: a plugin or a
    #     `herdr tab create` still gets stock behaviour, and one that wants a
    #     shell asks for it with --env. `or_insert_with` keeps that promise the
    #     other way too — an explicit env from the caller is never overwritten.
    #   - create_workspace_with_options is what the rest goes through, and it
    #     carries no env at all to make an exception with: opening a worktree
    #     (`New worktree`, `Open worktree...`, and `herdr worktree open` — one
    #     path, whoever called it) and the workspace herdr seeds at startup
    #     when it restores no session. It takes the pair directly.
    substituteInPlace src/app/runtime_mutations.rs \
      --replace-fail '        self.dispatch_runtime_mutation(id, Method::WorkspaceCreate(params))' '        self.dispatch_runtime_mutation(id, Method::WorkspaceCreate({ let mut params = params; params.env.entry("HERDR_DRIP_PANE_KIND".to_string()).or_insert_with(|| "shell".to_string()); params }))'

    substituteInPlace src/app/runtime_mutations.rs \
      --replace-fail '        self.dispatch_runtime_mutation(id, Method::TabCreate(params))' '        self.dispatch_runtime_mutation(id, Method::TabCreate({ let mut params = params; params.env.entry("HERDR_DRIP_PANE_KIND".to_string()).or_insert_with(|| "shell".to_string()); params }))'

    substituteInPlace src/app/creation.rs \
      --replace-fail '        self.create_workspace_with_launch_env(initial_cwd, focus, Vec::new())' '        self.create_workspace_with_launch_env(initial_cwd, focus, vec![("HERDR_DRIP_PANE_KIND".to_string(), "shell".to_string())])'

    # shell-splits: four split entries on the pane menu instead of two — a
    # shell and an agent, each way — for a terminal next to the agent instead
    # of a second agent. With shell-panes above, splitting is now the ONLY
    # gesture that starts claude, so the pane menu is where both answers have
    # to live.
    #
    # No plugin can add them: a plugin's [[actions]] reach the command palette
    # and keybindings, never herdr's context menus, and the menu's items are a
    # `&'static str` list compiled into ContextMenuState::items.
    #
    # The list is ordered by DIRECTION — right (shell), right (agent), down
    # (shell), down (agent) — so the two rows you pick between sit adjacent,
    # and each carries two Codicon glyphs naming the direction and the kind.
    # Both labels and glyphs live in pane-menu-labels.rs, appended to state.rs
    # because that is the module the dispatcher can name (see the file). Note
    # that `Split right` no longer means "an agent" by omission: the agent rows
    # say so, which is the whole point of shell-panes being upstream of them.
    #
    # The two live arms in apply_context_menu_action_via_api are widened to
    # accept both of their labels rather than duplicated, so the shell split
    # keeps the focus and mode handling of the stock one and the only
    # difference is the launch env. The 16-space indentation is what makes each
    # anchor unique: the same lines exist at 12 spaces in the #[cfg(test)] copy
    # of the dispatcher, which keeps matching herdr's own tests — those drive
    # the menu by item POSITION or by "Close pane", never by a split label, so
    # renaming these four is invisible to them.
    cat ${./pane-menu-labels.rs} >> src/app/state.rs
    cat ${./pane-menu-splits.rs} >> src/app/input/modal.rs

    substituteInPlace src/app/state.rs \
      --replace-fail '                items.extend(["Split right", "Split down", "Zoom"]);' '                items.extend([DRIP_SPLIT_RIGHT_SHELL, DRIP_SPLIT_RIGHT_AGENT, DRIP_SPLIT_DOWN_SHELL, DRIP_SPLIT_DOWN_AGENT, "Zoom"]);'

    substituteInPlace src/app/input/modal.rs \
      --replace-fail '                Some("Split right"),' '                Some(crate::app::state::DRIP_SPLIT_RIGHT_SHELL | crate::app::state::DRIP_SPLIT_RIGHT_AGENT),'

    substituteInPlace src/app/input/modal.rs \
      --replace-fail '                self.split_focused_pane_via_api(crate::api::schema::SplitDirection::Right);' '                self.drip_split_focused_pane(crate::api::schema::SplitDirection::Right, item == Some(crate::app::state::DRIP_SPLIT_RIGHT_SHELL));'

    substituteInPlace src/app/input/modal.rs \
      --replace-fail '                Some("Split down"),' '                Some(crate::app::state::DRIP_SPLIT_DOWN_SHELL | crate::app::state::DRIP_SPLIT_DOWN_AGENT),'

    substituteInPlace src/app/input/modal.rs \
      --replace-fail '                self.split_focused_pane_via_api(crate::api::schema::SplitDirection::Down);' '                self.drip_split_focused_pane(crate::api::schema::SplitDirection::Down, item == Some(crate::app::state::DRIP_SPLIT_DOWN_SHELL));'

    # pane-menu-trim: three items off the pane menu, because a menu is also a
    # list of things you can hit by accident.
    #   - `Rename pane` and `Clear pane name` — the pane label is the agent's
    #     terminal title, which says what the pane is doing right now; a manual
    #     name freezes that at whatever was true when you typed it. Renaming a
    #     pane is still there for anyone who wants it, through the API
    #     (`herdr pane rename`) — this takes it off the two-item-away gesture.
    #   - `Send right-clicks to pane` — passthrough means this menu can no
    #     longer be opened here, so the click that turns it on is the click
    #     that hides the way back (the modifier in
    #     `ui.right_click_passthrough_modifier`, if a host has set one).
    #
    # Only the SET half goes. The item is one slot that reads
    # `Use Herdr right-click menu` while a pane is in passthrough, and that
    # half stays: `herdr pane input --right-click pane` and `pane split
    # --right-click pane` can still put a pane there, and the menu should
    # remain the way out. Filtering the finished list, rather than unpicking
    # the pushes that build it, is what keeps this to one anchor and leaves
    # every dispatch arm in place.
    substituteInPlace src/app/state.rs \
      --replace-fail '                items.push("Close pane");' '                items.push("Close pane"); items.retain(|item| !matches!(*item, "Rename pane" | "Clear pane name" | "Send right-clicks to pane"));'
  '';
})
