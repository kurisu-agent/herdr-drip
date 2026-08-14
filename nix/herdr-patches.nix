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
#     The `rev`/`dirtyRev` arguments are NOT part of that contract — flake.nix
#     applies them ahead of time (see `lib.patchHerdr` there), so what
#     consumers hold stays the same one-argument function it was. Anything
#     importing this file directly supplies its own revs, or none.
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
  dirtyRev ? null,
}:
herdrPkg:
let
  # The drip's own commit, for the sidebar-version patch below. Six chars is
  # what the user asked for and plenty to name a commit in a repo this size;
  # `self.rev` is a full 40, `self.shortRev` is 7, and `self.dirtyShortRev` is
  # 7 plus a `-dirty` suffix, so none is usable as-is. Rendered with a LEADING
  # space so the empty case concatenates to exactly the old string rather than
  # a trailing one.
  #
  # A dirty tree gets the commit it SITS ON plus a trailing `*` — the git-prompt
  # convention, and six columns cheaper than spelling the word. The mark is not
  # decoration: the whole point of the header is answering "which build is
  # this?", and a modified tree is precisely the case where the bare sha would
  # answer it wrongly. `*` says "that commit, plus whatever was uncommitted at
  # build time" — which is as much as any build off a working tree can honestly
  # claim.
  #
  # Neither rev means a source with no git at all (a `path:` input, or a
  # tarball). That degrades quietly to the stock-shaped " herdr 0.8.0": there
  # is no commit to name and nothing to qualify, so there is nothing to say.
  dripRev =
    if rev != null then
      " " + builtins.substring 0 6 rev
    else if dirtyRev != null then
      " " + builtins.substring 0 6 dirtyRev + "*"
    else
      "";
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
    # this?" has no answer anywhere in the UI. Three tokens, one line:
    # `󰖌 herdr 0.8.0 85c510`, and `85c510*` off a modified tree.
    #
    # The leading drop is nf-md-water (U+F058C) — the drip's mark, saying at a
    # glance that this herdr is a PATCHED one and not stock. Written as a rust
    # `\u{}` escape rather than the literal character for the same reason
    # context-menu-items.rs does: the codepoint stays greppable against
    # nerd-fonts' glyphnames.json, and it survives the nix-to-shell-to-source
    # trip as plain ASCII. Note this is a Material Design Icon, which Nerd
    # Fonts moved to the 5-digit U+F0001.. range in v2.3.0 — unlike the
    # Codicons elsewhere in the set, a pre-v3 patched font renders it as tofu.
    #
    # It stays a `concat!` of literals, so the whole label is still resolved
    # at COMPILE time and the anchor keeps its shape — the rev arrives as a
    # nix interpolation into the replacement text, not as a runtime lookup.
    # The row is a 1-line Paragraph at the sidebar's full width (26 by
    # default), so it truncates rather than wraps; 22 columns leaves room.
    substituteInPlace src/ui/sidebar.rs \
      --replace-fail '" spaces",' 'concat!(" \u{f058c} herdr ", env!("CARGO_PKG_VERSION"), "${dripRev}"),'

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

    # pane-menu: the pane's right-click menu, rewritten — what it offers, in
    # what order, what each row is called, and an icon column down the right.
    #
    #     New Tab
    #     New Space
    #     ─────────────────────────────────────────
    #     Zoom
    #     ─────────────────────────────────────────
    #     Agent Right
    #     Agent Down
    #     Shell Right
    #     Shell Down
    #     ─────────────────────────────────────────
    #     Close
    #
    # Four things here, and only the first has anywhere else it could live:
    #
    #   - THE SPLITS. Stock offers `Split right`/`Split down`, both of which
    #     start claude, because default_shell is yolo-shell. With shell-panes
    #     above, a split is now the ONLY gesture that starts an agent, so the
    #     menu carries both answers and names them: Agent, or Shell. A plugin's
    #     [[actions]] reach the command palette and the keybindings, never a
    #     context menu, whose items are a `&'static str` list compiled in.
    #   - THE CREATIONS. `New Tab` and `New Space` are not on the pane menu at
    #     all in stock herdr — they live on the tab and sidebar menus, which is
    #     a trip to the sidebar for the two things you most often want next to
    #     the pane you are looking at. They reuse herdr's own entry points, so
    #     they inherit its name prompts and, through shell-panes, open shells.
    #   - THE SEPARATORS. Three groups: what makes something new, what
    #     rearranges what exists, and what splits. A separator is an item like
    #     any other in a `Vec<&'static str>`, so it has to be made
    #     unselectable in both directions — the keyboard skips it
    #     (`drip_menu_move`) and the hit test refuses it (below).
    #   - THE ICON COLUMN, right-aligned. This is the reason the labels stopped
    #     carrying their own glyphs: the pad between label and icon depends on
    #     the popup's width, which is computed per frame from the longest item,
    #     so only the renderer can know it. See context-menu-render.rs.
    #
    # `pane-menu-trim` used to be a separate patch that retained three items
    # away (`Rename pane`, `Clear pane name`, `Send right-clicks to pane`).
    # It is gone as a patch and kept as an OUTCOME: drip_pane_menu names every
    # row it wants, so those three are absent by omission. The reasons stand —
    # a pane's label is the agent's terminal title and a manual name freezes it
    # (`herdr pane rename` still works), and the click that turns passthrough
    # on is the click that hides the menu it would be turned off from. Only the
    # SET half went: `Use Herdr right-click menu` is carried through whenever
    # stock offers it, because the CLI can still put a pane in passthrough and
    # this menu should remain the way out.
    #
    # The two live split arms are WIDENED to take both of their labels rather
    # than duplicated, so a shell split keeps the focus and mode handling of
    # the stock one and differs only in the launch env. Their 16-space
    # indentation is what makes each anchor unique: the same lines exist at 12
    # spaces in the #[cfg(test)] copy of the dispatcher. herdr's own tests find
    # menu rows by POSITION or by the string "Close pane" — renaming that row
    # to "Close" would fail those tests if anyone ran them, which the nix build
    # does not (`doCheck = false`) and check-herdr-patches.sh does not either.
    # That is a real cost of the rename, paid knowingly.
    cat ${./context-menu-items.rs} >> src/app/state.rs
    cat ${./context-menu-render.rs} >> src/ui/menus.rs
    cat ${./pane-menu-splits.rs} >> src/app/input/modal.rs

    # The whole list, in our order, built from the one stock just finished
    # assembling — so which CONDITIONAL rows exist stays herdr's decision (a
    # swap only mid-swap, the passthrough exit only while in passthrough) and
    # ours is only order and wording. Shadowing `items` right before the arm's
    # tail expression is what makes this one anchor instead of four.
    substituteInPlace src/app/state.rs \
      --replace-fail '                items.push("Close pane");' '                items.push("Close pane"); let items = drip_pane_menu(items);'

    # Two arms of our own, injected at the head of the live dispatcher, where
    # `match (menu.kind, item) {` sits at 8 spaces (the #[cfg(test)] copy has
    # it at 4). They set no trailing mode of their own: open_new_tab_dialog
    # always moves to the tab-name prompt, while begin_tui_workspace_create
    # only prompts when `prompt_new_workspace_name` is set and otherwise
    # creates silently — leaving the mode on ContextMenu for a menu that has
    # already been taken. Hence the leave_modal for exactly that case.
    substituteInPlace src/app/input/modal.rs \
      --replace-fail '        match (menu.kind, item) {' '        match (menu.kind, item) { (ContextMenuKind::Pane { ws_idx, pane_id, .. }, Some(crate::app::state::DRIP_NEW_TAB)) => { self.focus_pane_internal_via_api(ws_idx, pane_id); open_new_tab_dialog(&mut self.state); } (ContextMenuKind::Pane { ws_idx, pane_id, .. }, Some(crate::app::state::DRIP_NEW_SPACE)) => { self.focus_pane_internal_via_api(ws_idx, pane_id); self.begin_tui_workspace_create("tui.menu.workspace.create"); if self.state.mode == Mode::ContextMenu { leave_modal(&mut self.state); } }'

    substituteInPlace src/app/input/modal.rs \
      --replace-fail '                Some("Split right"),' '                Some(crate::app::state::DRIP_AGENT_RIGHT | crate::app::state::DRIP_SHELL_RIGHT),'

    substituteInPlace src/app/input/modal.rs \
      --replace-fail '                self.split_focused_pane_via_api(crate::api::schema::SplitDirection::Right);' '                self.drip_split_focused_pane(crate::api::schema::SplitDirection::Right, item == Some(crate::app::state::DRIP_SHELL_RIGHT));'

    substituteInPlace src/app/input/modal.rs \
      --replace-fail '                Some("Split down"),' '                Some(crate::app::state::DRIP_AGENT_DOWN | crate::app::state::DRIP_SHELL_DOWN),'

    substituteInPlace src/app/input/modal.rs \
      --replace-fail '                self.split_focused_pane_via_api(crate::api::schema::SplitDirection::Down);' '                self.drip_split_focused_pane(crate::api::schema::SplitDirection::Down, item == Some(crate::app::state::DRIP_SHELL_DOWN));'

    substituteInPlace src/app/input/modal.rs \
      --replace-fail '                Some("Close pane"),' '                Some(crate::app::state::DRIP_CLOSE),'

    # Draw each row through the row builder instead of `Line::from(*item)`.
    # `inner.width - 1` is the TEXT width: ratatui reserves the highlight
    # symbol's column on every row, selected or not, so right-aligning inside
    # what is left is what the glyph is positioned against.
    substituteInPlace src/ui/menus.rs \
      --replace-fail '        .map(|item| ListItem::new(Line::from(*item)))' '        .map(|item| ListItem::new(drip_menu_row(*item, inner.width.saturating_sub(1), p)))'

    # ...and let the popup size itself for that column: THREE more columns for
    # any row that has a glyph, none for a menu whose rows do not. Three, not
    # two: one for the glyph, one for the gap that separates it from the label,
    # and one for the margin that keeps it off the right border. Widening the
    # `+ 4` instead would pad every menu in the app whether it needed it or not.
    substituteInPlace src/app/input/mouse.rs \
      --replace-fail '            .map(|item| item.len() as u16)' '            .map(|item| item.len() as u16 + if crate::app::state::drip_menu_glyph(item).is_some() { 3 } else { 0 })'

    # The menu's frame: rounded, and neutral rather than accent-coloured. Only
    # the ONE call site moves — `render_panel_shell` itself is shared with the
    # dialogs, settings and navigator, and rounding it there would round every
    # panel in the app off a request about this menu. See drip_menu_shell.
    substituteInPlace src/ui/menus.rs \
      --replace-fail 'let Some(inner) = render_panel_shell(frame, menu_rect, p.accent, p.panel_bg) else {' 'let Some(inner) = drip_menu_shell(frame, menu_rect, p) else {'

    # ...and the selected row with it. Stock fills it with the accent, which on
    # a menu whose frame is no longer accented is the last coloured block left.
    # `surface0` is the same neutral the unselected tab carries, so a menu
    # highlight and a tab read as the same kind of thing.
    #
    # The foreground has to move WITH it and cannot be left to
    # `panel_contrast_fg`: that helper answers `surface_dim` whenever panel_bg
    # is `Reset` (which it is, the chrome being transparent), and dark text was
    # legible on the accent but would be dark-on-dark here. `text` is the
    # foreground every other row in the list already uses, so the highlight now
    # differs from its neighbours by background alone.
    substituteInPlace src/ui/menus.rs \
      --replace-fail '                .bg(p.accent)' '                .bg(p.surface0)'

    substituteInPlace src/ui/menus.rs \
      --replace-fail '                .fg(panel_contrast_fg(p))' '                .fg(p.text)'

    # A separator is not a row you can land on. The hit test is the narrow
    # place to say so once: it backs BOTH the click that activates a row and
    # the mouse-move that highlights one, so a pointer crossing a separator
    # leaves the highlight where it was and a click on one is treated like a
    # click outside the menu.
    substituteInPlace src/app/input/mouse.rs \
      --replace-fail '            Some((row - inner_y) as usize)' '            { let idx = (row - inner_y) as usize; if self.context_menu.as_ref().and_then(|menu| menu.items().get(idx).copied()) == Some(crate::app::state::DRIP_MENU_SEPARATOR) { None } else { Some(idx) } }'

    # The keyboard half of the same rule. MenuListState is shared with menus
    # that have no separators and cannot see the items, so the skip lives with
    # the list instead. Both anchors are the LIVE handler at 20 spaces; the
    # #[cfg(test)] copy sits at 16 and keeps stock behaviour.
    substituteInPlace src/app/input/modal.rs \
      --replace-fail '                    menu.list.move_prev();' '                    crate::app::state::drip_menu_move(menu, -1);'

    substituteInPlace src/app/input/modal.rs \
      --replace-fail '                    menu.list.move_next(menu.items().len());' '                    crate::app::state::drip_menu_move(menu, 1);'

    # single-pane-borders: draw a pane's border even when it is the only pane.
    #
    # This is NOT the `ui.pane_borders` / `ui.pane_outer_borders` config — both
    # are already true by default. herdr ANDs them with a hardcoded
    # `pane_count() > 1`, so a lone pane loses its frame no matter what the
    # config says, and there is no key to turn that off. A plugin cannot reach
    # it either: this is the renderer, three expressions deep in ui/panes.rs.
    #
    # The frame is what tells you WHICH pane has focus and where a pane ends,
    # and a workspace that opens with one pane spends its first minutes with
    # neither. Splitting to get a border is a poor trade.
    #
    # The flag is patched where it is DEFINED, not at its five use sites: one
    # answer, and no conditions rewritten. Every other thing `multi_pane` gates
    # is already a no-op for a lone pane — `pane_to_right`/`pane_below` find no
    # neighbour to find, and the gap shrink is keyed on having found one — so
    # the only behaviour that changes is the border.
    #
    # The one use that is NOT about chrome is `should_dim`, which greys
    # UNFOCUSED panes. It cannot fire here either: with one pane in the layout
    # that pane is the focused one, so `!info.is_focused` is false whatever the
    # flag says. That is why the third substitution can take both occurrences
    # of its line (the second is the dimming pass) — check-herdr-patches.sh
    # notes the count, and both are meant.
    substituteInPlace src/ui/panes.rs \
      --replace-fail '    let multi_pane = panes.len() > 1;' '    let multi_pane = true;'

    substituteInPlace src/ui/panes.rs \
      --replace-fail '    let multi_pane = tab.layout.pane_count() > 1;' '    let multi_pane = true;'

    substituteInPlace src/ui/panes.rs \
      --replace-fail '    let multi_pane = ws.layout.pane_count() > 1;' '    let multi_pane = true;'

    # rounded-corners: pane frames get zellij's rounded corners (╭ ╮ ╰ ╯)
    # instead of the square ┌ ┐ └ ┘. Cosmetic, and deliberately only the
    # corners — the straight runs stay ─ │, so this is the same frame with its
    # four ends softened.
    #
    # herdr's config has no border style at all (`herdr --default-config` has
    # pane_borders / pane_outer_borders / pane_gaps /
    # show_agent_labels_on_pane_borders, all booleans), so there is nothing to
    # set and nothing for a plugin to reach: the glyphs are a `&'static str`
    # match compiled into the renderer.
    #
    # Two sites compose a pane frame, and they compose it differently:
    #   - The tiled grid does NOT use ratatui's Block. `render_pane_borders`
    #     accumulates a `LineCell { up, down, left, right }` per screen cell
    #     across every pane rect and split line, then `line_cell_symbol` picks
    #     one glyph for the resulting arm set — which is how a shared edge
    #     between two panes becomes one ├ rather than two overlapping frames.
    #     So there is no BorderType to set here; the four corner ARMS are
    #     patched, one line each, and every other arm set is left alone.
    #   - `render_popup_pane` is a real `Block`, so it takes ratatui's own
    #     `BorderType::Rounded` (ROUNDED = the same four glyphs). Fully
    #     qualified because panes.rs imports `Block, Borders, Clear, Paragraph`
    #     and not BorderType, and adding a `use` would need a second anchor.
    #
    # The tees and the cross where splits MEET stay square — ┬ ┴ ├ ┤ ┼ have no
    # rounded counterpart in Unicode's box-drawing block, and zellij draws them
    # square for the same reason. Every glyph outside a pane frame (the
    # sidebar's tree, dialogs, the status bar) is untouched.
    substituteInPlace src/ui/panes.rs \
      --replace-fail '(false, true, false, true) => "┌",' '(false, true, false, true) => "╭",'

    substituteInPlace src/ui/panes.rs \
      --replace-fail '(false, true, true, false) => "┐",' '(false, true, true, false) => "╮",'

    substituteInPlace src/ui/panes.rs \
      --replace-fail '(true, false, false, true) => "└",' '(true, false, false, true) => "╰",'

    substituteInPlace src/ui/panes.rs \
      --replace-fail '(true, false, true, false) => "┘",' '(true, false, true, false) => "╯",'

    substituteInPlace src/ui/panes.rs \
      --replace-fail '.border_style(Style::default().fg(app.palette.accent))' '.border_style(Style::default().fg(app.palette.accent)).border_type(ratatui::widgets::BorderType::Rounded)'
  '';
})
