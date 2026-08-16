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

    # sidebar-git-status: the workspace row's git_status token says what the
    # claude-drip statusline says about the same checkout, and only that — the
    # short HEAD hash and the added / modified / deleted counts, as bare
    # coloured numbers with no glyph. Stock says one thing instead, `↑4 ↓1`, so
    # a space could show "4 ahead" while the pane beside it showed "and 3 files
    # uncommitted": two rows, one working tree, two vocabularies. The token now
    # speaks the pane's. herdr's arrows are not restyled or folded in, they are
    # gone — see the zeroed pair below. See sidebar-git-status.rs for the
    # layout, the colours and what the counts cost.
    #
    # No plugin surface reaches it: a plugin's `$token` is one string with one
    # style, so it cannot paint three counts in three colours, and nothing in
    # the plugin API can add a field to a sidebar token's data or to how one is
    # measured.
    cat ${./sidebar-git-status.rs} >> src/ui/sidebar.rs

    # The token needs to know WHICH checkout it is describing, which the token
    # context does not carry — ahead/behind arrives already computed, so no
    # consumer has ever needed the path. One field, and one line to fill it.
    #
    # The second anchor matches TWICE on purpose (the check script says so):
    # herdr builds this context in two places, `workspace_row_height` and the
    # renderer, and a row measured without the counts and drawn with them is a
    # clipped row. Both sites have `ws` in scope and want the identical value,
    # so one substitution is the honest way to say "both".
    substituteInPlace src/ui/sidebar/tokens.rs \
      --replace-fail '    pub suppress_git_details: bool,' '    pub suppress_git_details: bool, pub drip_repo: Option<std::path::PathBuf>,'

    substituteInPlace src/ui/sidebar.rs \
      --replace-fail 'ahead_behind: ws.git_ahead_behind(),' 'ahead_behind: ws.git_ahead_behind(), drip_repo: drip_workspace_repo(ws),'

    # The details ride ON the existing token rather than beside it, because a
    # token is what the row's width budget is computed from: a separate token
    # would be measured, separated (` · `) and truncated on its own terms, and
    # `a1c3 · 3 · 2 · 1` is not what either row says today.
    substituteInPlace src/ui/sidebar/tokens.rs \
      --replace-fail '    GitStatus { ahead: usize, behind: usize },' '    GitStatus { ahead: usize, behind: usize, drip: super::DripGitDetails },'

    # ...and the token has to SURVIVE a repo that is level with its upstream,
    # which stock drops on the floor (`filter(ahead > 0 || behind > 0)`) — the
    # case where the counts are the only thing left to say, and the one the
    # request came from. `or(Some((0, 0)))` supplies the pair the rest of the
    # chain destructures; the added disjunct is what keeps the token alive.
    #
    # Stock's two disjuncts are left in rather than replaced. They no longer
    # decide anything visible — nothing draws arrows now — so all they can do
    # is keep an EMPTY token alive for the frame or two between a workspace
    # appearing and its first `git status` landing, where herdr's cached
    # ahead/behind is already known and the drip's cache is still cold. That
    # costs one space at the end of a row nobody is looking at yet, and it is
    # cheaper than a filter that says something different from what upstream's
    # says for a reason a reader would have to reconstruct.
    substituteInPlace src/ui/sidebar/tokens.rs \
      --replace-fail '.filter(|(ahead, behind)| *ahead > 0 || *behind > 0)' '.or(Some((0, 0))).filter(|(ahead, behind)| *ahead > 0 || *behind > 0 || !super::drip_git_details_for(context.drip_repo.as_deref()).is_empty())'

    # ...and the ahead/behind pair is DROPPED on the way into the token, which
    # is how herdr's `↑6` stops being drawn. The arrow is herdr's answer to a
    # question the statusline does not ask, and the row it shares is the
    # statusline's row; two dialects in nineteen columns is what this patch
    # exists to end, so the drip's pieces are not joined by an arrow, they
    # REPLACE it.
    #
    # Zeroing the pair at construction rather than deleting herdr's arrow code
    # is deliberate. That code is three `if`s and about twenty lines in the
    # renderer plus three terms in the measurer, and unpicking it would mean
    # twenty anchors that break on any upstream reflow, to delete something
    # that `ahead: 0, behind: 0` already makes provably unreachable: every one
    # of those branches tests a constant zero, and every one of those terms
    # multiplies by `usize::from(false)`. One anchor, and stock's arrow code is
    # free to move, be rewritten or be deleted upstream without touching this.
    #
    # herdr still COMPUTES ahead/behind for this token (the `git_status` token
    # is what sets `demand.ahead_behind`, and the refresh runs off the render
    # path either way). Turning that off is a different anchor and a different
    # story; this one is only about what the row says.
    substituteInPlace src/ui/sidebar/tokens.rs \
      --replace-fail '.map(|(ahead, behind)| ResolvedTokenKind::GitStatus { ahead, behind }),' '.map(|_| ResolvedTokenKind::GitStatus { ahead: 0, behind: 0, drip: super::drip_git_details_for(context.drip_repo.as_deref()) }),'

    # Bind the new field in both arms that match the token — the one that
    # measures it and the one that paints it. Two occurrences, one meaning.
    substituteInPlace src/ui/sidebar.rs \
      --replace-fail 'ResolvedTokenKind::GitStatus { ahead, behind } => {' 'ResolvedTokenKind::GitStatus { ahead, behind, drip } => {'

    # Measure, then paint, by ADDING a term to stock's sum and a push before
    # stock's first — not by rewriting either arm. Stock's three terms and
    # three `if`s stay exactly as upstream wrote them and, fed the zeroed pair
    # above, contribute 0 and draw nothing, so the drip's term is the whole
    # width and the drip's spans are the whole token. Each anchor is one line,
    # so a herdr that reflows the arrow code fails neither of them, and the
    # only thing that fails the build is a herdr that MOVES these two lines.
    #
    # The row therefore reads `<branch> <hash> <a> <m> <d>` — character for
    # character the leading run of the claude-drip statusline, and nothing
    # after it.
    substituteInPlace src/ui/sidebar.rs \
      --replace-fail '                    + usize::from(*ahead > 0 && *behind > 0)' '                    + usize::from(*ahead > 0 && *behind > 0) + drip_git_status_width(drip)'

    substituteInPlace src/ui/sidebar.rs \
      --replace-fail '                if *ahead > 0 {' '                spans.extend(drip_git_status_spans(drip, secondary_style, p, token.style)); if *ahead > 0 {'

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

    # sidebar-quiet-chrome: the sidebar's section labels go. Five words the
    # sidebar repeats at you every frame — `new` and `menu` under the
    # workspace list, `agents` over the agent panel, `grouped`/`priority` in
    # its corner (and the accounts rail's `accounts`, retired in
    # sidebar-accounts.rs itself) — each naming something the rows beneath
    # already show. A 26-column sidebar has no room for captions, and no
    # plugin can reach any of them: they are string literals in the renderer.
    #
    # Every substitution below empties what is DRAWN and leaves every rect the
    # hit-testing reads untouched, so the click targets stay exactly where
    # they were: the footer's new/menu zones still work (they were only
    # visible on hover — mouse_capture — anyway). The menu badge dot survives
    # its label: `●` is an attention signal, not chrome.
    #
    # The corner cell is the exception, and it is not empty for long —
    # sidebar-scope-icon below moves into it. The two halves of that cell are
    # here because this is where its text died: the sort label
    # (`grouped`/`priority`), and the agent-view label under it, which would
    # otherwise read `filtered` on every frame forever now that
    # drip.agent-scope keeps a view active by default. Blanking the label's
    # FALLBACK rather than the whole expression is what leaves a labelled view
    # able to say so — and what lets the icon patch tell "no label" from "a
    # label worth its columns" with one `.filter`.
    substituteInPlace src/ui/sidebar.rs \
      --replace-fail 'Paragraph::new(Span::styled(" new", Style::default().fg(p.overlay0))),' 'Paragraph::new(Span::styled("", Style::default().fg(p.overlay0))),'

    substituteInPlace src/ui/sidebar.rs \
      --replace-fail 'Span::styled("menu", Style::default().fg(p.overlay0)),' 'Span::styled("", Style::default().fg(p.overlay0)),'

    substituteInPlace src/ui/sidebar.rs \
      --replace-fail 'Line::from(vec![Span::styled("menu", Style::default().fg(p.overlay0))])' 'Line::from(vec![Span::styled("", Style::default().fg(p.overlay0))])'

    substituteInPlace src/ui/sidebar.rs \
      --replace-fail '            " agents",' '            "",'

    substituteInPlace src/ui/sidebar.rs \
      --replace-fail '        .unwrap_or_else(|| agent_panel_sort_label(app.agent_panel_sort));' '        .filter(|label| !label.is_empty()).unwrap_or_else(|| app.drip_scope_glyph());'

    substituteInPlace src/ui/sidebar.rs \
      --replace-fail '.map(|view| view.label.as_deref().unwrap_or("filtered"))' '.map(|view| view.label.as_deref().unwrap_or(""))'

    # sidebar-auto-split: the workspace/agent divider follows the workspace
    # list instead of a dragged ratio. Stock herdr persists
    # `sidebar_section_split` and leaves it wherever the last drag put it, so
    # every workspace opened or closed either scrolls the list under empty
    # agent rows or floats it over them until someone drags the divider back.
    # The right value is not an opinion — it is however many rows the list
    # needs — and no plugin can reach it: the ratio is an AppState field the
    # renderer, hit-testing and scroll metrics all read.
    #
    # drip_auto_section_split (sidebar-auto-split.rs) computes that ratio, and
    # the ONE write below plants it in compute_view_internal before any
    # geometry is derived, so every consumer reads the same value they always
    # did. It writes the field rather than bypassing it so the whole pipeline
    # — including the persisted snapshot — stays coherent.
    #
    # Dragging is retired by consequence, not by surgery: the divider's hit
    # test and setter are untouched (herdr's drag test drives them directly),
    # but the next view pass recomputes the field, so a drag never survives to
    # a drawn frame. The clamp relaxation widens `sidebar_section_heights`'s
    # ratio guard from (0.1, 0.9) to (0.0, 1.0) so a short list on a tall
    # sidebar is not forced to hold 10% of it; the function's own 3-row
    # min/max clamps still keep both sections alive at every height.
    cat ${./sidebar-auto-split.rs} >> src/ui/sidebar.rs

    substituteInPlace src/ui.rs \
      --replace-fail '        app.workspace_scroll = normalized_workspace_scroll(app, sidebar_area, app.workspace_scroll);' '        app.sidebar_section_split = sidebar::drip_auto_section_split(app, sidebar_area); app.workspace_scroll = normalized_workspace_scroll(app, sidebar_area, app.workspace_scroll);'

    substituteInPlace src/ui/sidebar.rs \
      --replace-fail '    let ratio = split_ratio.clamp(0.1, 0.9);' '    let ratio = split_ratio.clamp(0.0, 1.0);'

    # sidebar-scope-icon: one clickable cell in the agent panel's header that
    # says which scope the list is in — a window for this space, two windows
    # for every space — and flips it when clicked. drip.agent-scope's toggle,
    # as a thing you can see and hit rather than a key you have to know.
    #
    # It lands in the cell sidebar-quiet-chrome just emptied, which is the
    # right cell twice over: it is where the agent panel has always kept its
    # one control, and that control was ALREADY DEAD in the drip's default
    # state — `on_agent_panel_sort_toggle` returns false whenever an agent
    # view is active, and drip.agent-scope keeps one active. So this takes a
    # cell that does nothing on a default host and gives it something to do.
    #
    # herdr has real click routing here and this uses it rather than inventing
    # any: the sidebar's clickable elements are each a `*_rect` geometry
    # function in ui/sidebar.rs plus an `on_*` hit test in app/input/sidebar.rs
    # that handle_mouse asks in order, and all three of those are patched
    # below — the same three pieces `sidebar_new_button_rect` and the workspace
    # rows use. Nothing here paints its own hit region or second-guesses
    # herdr's dispatch order.
    #
    # The state and the glyphs are in agent-scope-icon.rs, appended to
    # app/state.rs because the renderer and app::input both have to name them
    # and `app::input` is private — the same constraint pane-menu-labels.rs
    # works under.
    cat ${./agent-scope-icon.rs} >> src/app/state.rs

    # The hit test, freed of the guard that made it inert. That guard exists
    # because a filtered panel has no sort to cycle; the cell now toggles the
    # FILTER itself, which is precisely the thing that is still meaningful
    # while a view is active — so the condition that used to disable it is the
    # condition it is now for.
    substituteInPlace src/app/input/sidebar.rs \
      --replace-fail '        if self.sidebar_collapsed || self.agent_view_override.is_some() {' '        if self.sidebar_collapsed {'

    # ...and sized to the glyph rather than to `grouped`/`priority`. The rect
    # is what the hit test reads, and the renderer sizes the drawn cell from
    # the same helper with the same one-column string, so the region you can
    # click is exactly the glyph you can see instead of the seven blank columns
    # left of it. Either glyph gives the same width (both are single-width
    # Codicons), so the constant does not have to know which scope is on.
    #
    # The discarded `agent_panel_sort_label(sort)` call is not noise: this line
    # and the renderer's are that helper's ONLY two callers, and the other one
    # is patched above — so without it both the parameter and the function go
    # unused and the build warns twice about a herdr function this patch
    # merely stopped drawing. herdr still owns the sort; we own the cell.
    substituteInPlace src/ui/sidebar.rs \
      --replace-fail '    agent_panel_header_label_rect(area, agent_panel_sort_label(sort))' '    { let _ = agent_panel_sort_label(sort); agent_panel_header_label_rect(area, crate::app::state::DRIP_SCOPE_GLYPH_CURRENT) }'

    # The click. One line, so the `match` it used to open is left parsing and
    # its value discarded rather than the whole block being rewritten — a
    # multi-line anchor would be at the mercy of nix's indented-string
    # stripping, and this keeps the two statements after it (`scroll = 0`,
    # `mark_session_dirty`) exactly where they were, both of which a scope
    # change wants anyway.
    #
    # THE COST, stated plainly: the pane menu is not the only place this drip
    # has taken a herdr gesture, and like `Close pane` before it, this breaks
    # one of herdr's own tests — `clicking_agent_panel_toggle_switches_sort`
    # clicks this cell and asserts the SORT changed, which it no longer does.
    # The nix build does not run herdr's tests (`doCheck = false`) and neither
    # does check-herdr-patches.sh, so nothing here catches it; a `cargo test`
    # on a patched checkout would fail exactly there. The sort itself is not
    # lost — `ui.agent_panel_sort` still sets it, and its label was already
    # invisible — but the mouse can no longer cycle it.
    substituteInPlace src/app/input/mouse.rs \
      --replace-fail '                        self.agent_panel_sort = match self.agent_panel_sort {' '                        self.drip_toggle_agent_scope(); let _ = match self.agent_panel_sort {'

    # agent-scope-family: "this space" means this space AND the rest of its
    # repo — the workspace you are on plus every worktree of the same checkout.
    #
    # herdr models a worktree as a SIBLING workspace, not a child: `beads-ui`
    # and its `graph-view` / `epic-tree` / `health-panel` worktrees are four
    # workspaces sharing only a `WorktreeSpaceMembership`. So an agent view
    # filtered by workspace id could never show a repo row its own worktrees'
    # agents, which is precisely the row you stand on to ask what the whole
    # repo is doing.
    #
    # THE FILTER LANGUAGE CANNOT SAY THIS, which is why it is here and not in
    # the plugin. Its fields are status / workspace_id / tab_id / pane_id /
    # agent / seen / state_change_seq plus free `token` lookups, and its only
    # dynamic values are the two context vars `current_workspace_id` and
    # `current_tab_id` — no repo field, no repo context, and
    # `validate_field_value` whitelists context values to exactly those two
    # (field, context) pairs, so even a token carrying the repo key could only
    # ever be compared against a STATIC string.
    #
    # The documented escape — have the plugin resolve the sibling ids itself
    # and send `in: [w6, w7, w8, w9]`, refreshing on `workspace.focused` — was
    # tried on paper and rejected for one concrete reason: a static id list
    # cannot track what herdr actually filters against.
    # `presented_workspace_idx` follows the SIDEBAR SELECTION while the sidebar
    # is being navigated (`Mode::Navigate` reads `app.selected`, not
    # `app.active`), and `WorkspaceFocused` is emitted from the pane-focus path
    # — it does not fire when you merely move the selection. So the list would
    # be stale exactly while you are arrowing through spaces looking at the
    # agent panel, which is the moment it is being read. It would also spawn a
    # process and a socket round trip on every space switch, and show one frame
    # of the previous repo's agents each time.
    #
    # Evaluated in place, none of that exists: this is asked per render against
    # the same presented workspace the stock context var uses.
    #
    # It is one `||` in front of the stock comparison, so it can only ever
    # WIDEN, never narrow: the exact id match still answers first for
    # everything, and drip_workspace_family_eq (agent-scope-icon.rs) adds the
    # rest of the family only for our own view, only for the one filter shape
    # our view sends, and only when BOTH workspaces carry a worktree record.
    # A workspace that is not a git checkout has no membership, so it matches
    # nothing extra and behaves exactly as it did.
    #
    # The plugin keeps sending the stock filter unchanged — this widens what
    # that filter MEANS on a patched herdr rather than inventing a shape only
    # a patched herdr could parse, so drip.agent-scope on a stock herdr still
    # works and simply stays exact, which is what it did until now.
    substituteInPlace src/app/agent_view.rs \
      --replace-fail '            field_value(app, entry, field) == operand_value(app, value)' '            crate::app::state::drip_workspace_family_eq(app, entry, field, value) || field_value(app, entry, field) == operand_value(app, value)'
    # sidebar-tab-tree: the workspace list gets a second nesting level. A
    # workspace's TABS hang under it — and under a worktree child when that is
    # what owns them — so the sidebar can say what the work in a space is and
    # not merely that there is some. Stock nests worktrees and nothing else;
    # tabs exist in the UI only as the tab bar of whichever workspace happens
    # to be active.
    #
    # No plugin surface reaches it. A plugin's sidebar reach is the per-row
    # TOKEN list, which is text inside a row herdr has already decided exists:
    # it cannot add rows, cannot nest them, and cannot be clicked. This is row
    # geometry, and row geometry here is `workspace_row_height` plus the four
    # passes that measure against it.
    #
    # The shape of the patch is chosen to keep the blast radius at four
    # anchors. `WorkspaceListEntry` is NOT extended: it is a one-variant enum
    # that five sites across three files destructure with an irrefutable
    # `let`, so a `Tab` variant would be five more anchors and a rewrite of
    # every index and scroll path in the list — including the agent panel's
    # neighbours, which this branch is deliberately not touching. Instead the
    # tab rows ride inside the workspace's own card: the entry list, the
    # scroll offsets, `app.selected` and every hit test keyed off them are
    # byte-identical, and the card is simply taller. Tall cards are stock
    # behaviour — a multi-row space row is what `workspace_row_height` is for.
    #
    # See sidebar-tab-tree.rs for the glyphs (`├╴`/`╰╴`, two columns against
    # the navigator's three, because 26 columns cannot afford two levels of
    # `├── `) and the collapse key; sidebar-tab-tree-input.rs for the clicks.
    cat ${./sidebar-tab-tree.rs} >> src/ui/sidebar.rs
    cat ${./sidebar-tab-tree-input.rs} >> src/app/input/sidebar.rs

    # THE ONE MEASUREMENT. Stock's body is renamed and kept verbatim, and the
    # name every caller uses now answers stock's height plus one row per tab.
    # That is what makes the rest agree without being told: the four passes
    # that size, scroll and place cards (`workspace_list_visible_count`,
    # `workspace_list_bottom_start`, `compute_workspace_list_areas`) and the
    # drip's own `drip_auto_section_split` above all call this and nothing
    # else, so the divider moves down to make room for tab rows using the same
    # number the renderer draws them in. `sidebar_section_heights`'s 3-row
    # clamp still guarantees the agent panel survives a space with twenty
    # tabs; the list scrolls, exactly as it does for twenty workspaces.
    substituteInPlace src/ui/sidebar.rs \
      --replace-fail 'fn workspace_row_height(app: &AppState, ws: &crate::workspace::Workspace, indented: bool) -> u16 {' 'fn workspace_row_height(app: &AppState, ws: &crate::workspace::Workspace, indented: bool) -> u16 { return drip_stock_workspace_row_height(app, ws, indented).saturating_add(drip_tab_rows(app, ws)); } fn drip_stock_workspace_row_height(app: &AppState, ws: &crate::workspace::Workspace, indented: bool) -> u16 {'

    # The draw, in the rows the measurement just reserved. Anchored on the
    # worktree-group chevron because that is the one line after the card's
    # token-row loop and before the next card, with `card`, `is_last_child`
    # and `list_bottom` all still in scope. Where stock's rows END is NOT read
    # off the renderer's own row vector but recomputed from the measuring
    # function, so the draw and the hit test read one number and a click can
    # never land on a row other than the one under the pointer.
    substituteInPlace src/ui/sidebar.rs \
      --replace-fail '        if let Some((_, collapsed)) = parent_group {' '        drip_render_tab_rows(app, frame, card, list_bottom, is_last_child); if let Some((_, collapsed)) = parent_group {'

    # app::input reads the geometry through `crate::ui`, so the two hit-test
    # helpers join the sidebar re-export list. One line of it, and the names
    # are appended rather than the list rewritten.
    substituteInPlace src/ui.rs \
      --replace-fail '        agent_panel_toggle_rect, all_agent_panel_entries, collapsed_sidebar_sections,' '        agent_panel_toggle_rect, all_agent_panel_entries, collapsed_sidebar_sections, drip_tab_tree_caret_at, drip_tab_tree_target_at,'

    # The click, asked BEFORE `workspace_at_row` because a tab row is inside
    # its workspace's card and that function would answer for it. The caret
    # goes first for the same reason: it sits on the workspace's own first
    # row. Both sit in the LEFT-button branch at 20 spaces — the right-button
    # copy of this line is at 16 and keeps stock behaviour, so a right-click
    # anywhere in a card still opens that workspace's menu.
    substituteInPlace src/app/input/mouse.rs \
      --replace-fail '                    if let Some(idx) = self.workspace_at_row(mouse.row) {' '                    if self.drip_toggle_tab_tree_at(mouse.column, mouse.row) { return None; } if let Some((ws_idx, pane_id)) = self.drip_sidebar_tab_target_at(mouse.row) { self.mode = Mode::Terminal; return Some(MouseAction::FocusPane { ws_idx, pane_id }); } if let Some(idx) = self.workspace_at_row(mouse.row) {'

    # rename-presets: six named kinds of work in the rename dialog, one click
    # each. The modal that names a pane, a tab or a space — and the one the
    # `New Tab` and `New Space` gestures open to name what they are about to
    # create — grows a list of presets under its input: `orchestration`,
    # `implementation`, `research`, `spike`, `monitor`, `misc`, each behind a
    # Codicon that is part of the name and therefore visible afterwards in the
    # sidebar and the tab bar. Clicking one is the whole gesture: it fills the
    # field and saves in the same click, so naming a pane is one click rather
    # than a word typed the same way for the hundredth time and an Enter.
    #
    # No plugin surface reaches it, and not narrowly: a plugin's reach is
    # `[[actions]]` (the palette and the keybindings), pane placements
    # (overlay/popup/split/tab/zoomed) and the sidebar's per-row tokens. The
    # rename modal is none of those — it is a `Mode`, drawn by
    # `render_rename_overlay` and hit-tested by `handle_mouse`, both compiled
    # in, with no list for anyone to contribute a row to. The nearest a plugin
    # could get is an action that renames a pane it already knows the id of,
    # which is a different gesture entirely: it cannot be reached from the
    # dialog you are already looking at, and it cannot see what you are
    # naming.
    #
    # Two halves, for the reason the pane menu has two: the names and their
    # geometry go in app/state.rs, which `ui::dialogs` and `app::input::mouse`
    # can both name; the drawing goes in ui/dialogs.rs, which is where a
    # `Frame` is.
    cat ${./rename-presets.rs} >> src/app/state.rs
    cat ${./rename-presets-render.rs} >> src/ui/dialogs.rs

    # The modal grows to fit them. Stock passes `56, 7` in two places that have
    # to agree to the row — the renderer sizes the popup it DRAWS and
    # `rename_modal_inner` sizes the popup it HIT-TESTS — so both are pointed
    # at one constant rather than at a second literal. A modal drawn 14 rows
    # tall and hit-tested as 7 would put every button and every preset row
    # somewhere other than where it appears.
    substituteInPlace src/ui/dialogs.rs \
      --replace-fail '    let Some(inner) = render_modal_shell(frame, area, 56, 7, &app.palette) else {' '    let Some(inner) = render_modal_shell(frame, area, 56, crate::app::state::DRIP_RENAME_POPUP_HEIGHT, &app.palette) else {'

    substituteInPlace src/app/input/overlays.rs \
      --replace-fail '        self.onboarding_modal_inner(56, 7)' '        self.onboarding_modal_inner(56, crate::app::state::DRIP_RENAME_POPUP_HEIGHT)'

    # The draw, and the action row moving to the bottom of the taller modal.
    #
    # `save`/`clear`/`cancel` are placed by `centered_button_row` at a fixed
    # offset of 3 from the top of the rect it is handed, which in stock's
    # 5-row inner was the last row but in a 12-row one is the middle of it —
    # buttons stranded above the presets, with the modal's floor empty. Rather
    # than rewrite that offset (a multi-line anchor, at the mercy of nix's
    # indented-string stripping), both callers hand the function the BOTTOM
    # FOUR ROWS of the modal, whose row 3 is the modal's last. Stock's
    # arithmetic is untouched and the buttons land where every other dialog in
    # herdr puts them.
    substituteInPlace src/ui/dialogs.rs \
      --replace-fail '    let (save_rect, clear_rect, cancel_rect) = rename_button_rects(inner);' '    drip_render_rename_presets(frame, inner, &app.palette); let (save_rect, clear_rect, cancel_rect) = rename_button_rects(crate::app::state::drip_rename_button_area(inner));'

    # The hit test's half of that move. This line and the one above are the
    # function's only two NON-TEST callers, which is what makes the shift safe
    # to do at the call sites: the buttons you can see and the buttons you can
    # click are computed from the same rect, one line apart in this file. The
    # two test callers are handled below.
    substituteInPlace src/app/input/mouse.rs \
      --replace-fail '                        .map(crate::ui::rename_button_rects)' '                        .map(|inner| crate::ui::rename_button_rects(crate::app::state::drip_rename_button_area(inner)))'

    # The click that applies a preset, asked BEFORE the buttons because the
    # fallthrough below it is `unwrap_or(ModalAction::Cancel)` — in stock
    # herdr every click that is not a button closes the dialog, so a preset row
    # asked afterwards would be a row that cancels. Setting `name_input` and
    # returning `Save` is deliberately the SAME path the save button takes:
    # one click is a select and a confirm, and everything that already knows
    # what saving means in each rename mode (create the tab, create the space,
    # rename the pane, close the modal) keeps deciding it.
    #
    # `name_input_replace_on_type` is cleared alongside for the same reason
    # herdr's own `clear` action clears it: it is the "the next keystroke
    # replaces this" flag, and what is in the field is now a deliberate choice
    # rather than a prefilled suggestion. Nothing reads it after the modal
    # closes, so this only matters if a later herdr keeps the dialog open.
    substituteInPlace src/app/input/mouse.rs \
      --replace-fail '                    let action = self' '                    if let Some(preset) = self.rename_modal_inner().and_then(|inner| crate::app::state::drip_rename_preset_at(inner, mouse.column, mouse.row)) { self.name_input = preset.to_string(); self.name_input_replace_on_type = false; return Some(MouseAction::RenameModal(ModalAction::Save)); } let action = self'

    # herdr's own caret tests reproduce the modal's layout from a copy of that
    # `56, 7`, and a taller popup is centred higher — so the same constant goes
    # in, and `cargo test` on a patched checkout still measures the dialog it
    # is looking at. The nix build does not run them (`doCheck = false`) and
    # neither does check-herdr-patches.sh; this is for whoever builds a patched
    # tree by hand.
    substituteInPlace src/ui/dialogs.rs \
      --replace-fail '        let popup = super::centered_popup_rect(area, 56, 7).expect("popup fits");' '        let popup = super::centered_popup_rect(area, 56, crate::app::state::DRIP_RENAME_POPUP_HEIGHT).expect("popup fits");'

    # And the OTHER test caller of `rename_button_rects`, for the same reason.
    # `clicking_rename_save_submits_workspace_rename_through_api_path` asks
    # `rename_modal_inner` where the modal is and then asks stock's
    # `rename_button_rects` where `save` is inside it — so with the modal
    # taller and the action row moved to its floor, it would click row 3 of a
    # 12-row inner, which is now the presets' rule row: no preset, no button,
    # and `unwrap_or(ModalAction::Cancel)` closes the dialog without renaming
    # anything. The test would fail on a rename the real dialog performs fine.
    # Routing it through the same `drip_rename_button_area` the handler uses
    # keeps it clicking the button it means to click.
    #
    # Its sibling in `app/input/modal.rs` needs nothing: it feeds the rect it
    # computed straight back into `modal_action_from_buttons`, so it is
    # self-consistent at any offset.
    substituteInPlace src/app/input/mouse.rs \
      --replace-fail '        let (save, _, _) = crate::ui::rename_button_rects(inner);' '        let (save, _, _) = crate::ui::rename_button_rects(crate::app::state::drip_rename_button_area(inner));'
  '';
})
