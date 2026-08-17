// Open (or close again) one of the board's pane entrypoints.
//
//   bun bin/open.js dock   -- a narrow column docked on the LEFT edge
//   bun bin/open.js tab    -- a whole tab in this workspace
//
// A launcher, not a wrapper: herdr has no dock-to-edge primitive and no way to
// ask "is this plugin's pane already open", so both of those are assembled here
// out of the pane API. Upstream's scripts/ do the same job in bash + python3;
// this is bun because python3 is deliberately not on the herdr server's PATH
// (nix/plugins.nix says why) and bun already is -- it is the rail's runtime.
//
// Every herdr call is an argv vector, and every failure is soft: a launcher
// that throws leaves you with half a layout and a stack trace in the plugin
// log, where one that gives up leaves you with the pane you started from.

const HERDR = process.env.HERDR_BIN_PATH || "herdr";
const PLUGIN_ID = "drip.beads";
const TIMEOUT_MS = 10_000;

function herdr(args) {
  let proc;
  try {
    proc = Bun.spawnSync([HERDR, ...args], {
      stdout: "pipe",
      stderr: "pipe",
      timeout: TIMEOUT_MS,
    });
  } catch {
    return null;
  }
  if (proc.exitCode !== 0) return null;
  const text = proc.stdout.toString().trim();
  if (!text) return {};
  try {
    return JSON.parse(text);
  } catch {
    return {};
  }
}

function panes() {
  const list = herdr(["pane", "list"]);
  const found = list?.result?.panes;
  return Array.isArray(found) ? found : [];
}

// The OSC titles that mean "this pane is our board", for one entrypoint.
//
// The binary titles itself from HERDR_PLUGIN_ENTRYPOINT_ID, so a pane opened
// through this script says `herdr-beads-tab` or `herdr-beads-dock`. A RESTORED
// board says neither: session restore relaunches the argv without re-injecting
// any plugin environment (see plugin-panes-survive-restore in
// ../../nix/herdr-patches.nix), so the binary falls back to naming itself after
// its --mode -- `herdr-beads-board` for the tab entrypoint, and
// `herdr-beads-dock` for the dock, which happens to be the same string it
// already used. So only the tab needs the second name, and without it the key
// would open a SECOND board beside the restored one instead of finding it.
function ourMarkers(entrypoint) {
  const marker = `herdr-beads-${entrypoint}`;
  return entrypoint === "dock" ? [marker] : [marker, "herdr-beads-board"];
}

function isOurBoard(pane) {
  const title = `${pane?.terminal_title ?? ""} ${pane?.terminal_title_stripped ?? ""}`;
  return title.includes("herdr-beads-");
}

// Our own panes, by that title.
//
// This is the only handle there is: there is no plugin.pane.list RPC, and
// PaneInfo carries no plugin_id, so a pane cannot be asked which plugin opened
// it. Scoped to this workspace because that is what a toggle means here -- the
// dock is per-tab furniture and a tab is per-workspace, and closing the copy
// you have open in another space would be a surprise from a keypress.
//
// Returns the panes rather than their ids: the caller decides between focusing
// one and closing them, and both of those need to know which one has focus.
function ourPanes(entrypoint) {
  const markers = ourMarkers(entrypoint);
  const ws = process.env.HERDR_WORKSPACE_ID ?? "";
  return panes().filter((pane) => {
    if (typeof pane?.pane_id !== "string" || !pane.pane_id) return false;
    const title = `${pane?.terminal_title ?? ""} ${pane?.terminal_title_stripped ?? ""}`;
    if (!markers.some((marker) => title.includes(marker))) return false;
    return !ws || pane?.workspace_id === ws;
  });
}

// Focus a board we already have. `plugin pane focus` is the call for it and it
// REFUSES a restored one: `handle_plugin_pane_focus` requires the pane to be in
// `state.plugin_panes`, which the session snapshot does not carry, so after a
// restart herdr no longer knows the pane belongs to a plugin. Its tab is the
// fallback and is nearly as good -- the board is the only pane in its tab in
// the case this exists for, and focusing a tab is what a key that means "show
// me the board" is asking for.
function focusPane(pane) {
  if (herdr(["plugin", "pane", "focus", pane.pane_id]) !== null) return;
  if (typeof pane?.tab_id === "string" && pane.tab_id) {
    herdr(["tab", "focus", pane.tab_id]);
  }
}

// Is this the label herdr gives a pane of ours? The manifest title, which herdr
// makes the pane's MANUAL label (`set_manual_label`) -- a codicon, a space and
// the word. Compared on the word alone: the glyph has changed once already and
// the word was capitalised until recently, and a stale snapshot carries
// whichever spelling was current when it was written.
function isBeadsLabel(label) {
  return typeof label === "string" && label.replace(/[^A-Za-z]/g, "").toLowerCase() === "beads";
}

// A pane that herdr thinks IS the board and that is not running it: our label,
// somebody else's process. That is what a restore used to leave behind before
// the patch above -- the label was saved, the command was not, and the default
// shell on this box is a claude -- and it is still what a board whose process
// died leaves, or a pane restored by an older herdr.
//
// It is worth finding because its TAB is a beads tab: that is the tab the user
// opened for the board, still named after it, and opening a third one beside it
// is how a workspace ends up with a row of tabs called beads.
//
// The pane's own label rather than the tab's, deliberately. A tab label can be
// set by hand and can be inherited from whatever pane is active, so hunting
// tabs called beads would let this script split into a tab somebody named that
// themselves and filled with their own work. The pane label is narrower and
// says something specific: HERDR set this, from our manifest, for a pane it
// opened for us.
//
// What is NOT distinguishable, and why nothing here closes anything: the
// artefact is a claude, and a claude somebody is using is also a claude.
// `pane list` offers `agent`, `agent_status` and `label` and no plugin
// ownership, so "fresh restore nobody has touched" and "the agent I asked a
// question ten minutes ago" look alike -- and the second one is not something a
// keypress may kill. So the board opens BESIDE it and the stray pane is left
// exactly as it was.
function strayBoardPane() {
  const ws = process.env.HERDR_WORKSPACE_ID ?? "";
  return (
    panes().find((pane) => {
      if (typeof pane?.pane_id !== "string" || !pane.pane_id) return false;
      if (ws && pane?.workspace_id !== ws) return false;
      return isBeadsLabel(pane?.label) && !isOurBoard(pane);
    }) ?? null
  );
}

// The repo to open on, as a hint. The binary resolves this for itself and is
// stricter about it than we can be here -- it walks up for `.beads` and refuses
// to guess -- so this is the belt to that braces, and matters most for the tab,
// where the board becomes the focused pane and has nobody else's focus to read.
// The pane this was invoked FROM -- the focused one in OUR workspace, which is
// not always the focused one full stop: a key press comes from the space you
// are looking at, but `herdr plugin action invoke` from a shell elsewhere
// carries that space's id while the open would land wherever the focus is.
// Naming it keeps the open and the toggle talking about the same workspace;
// otherwise a second press opens a second board instead of closing the first.
function invokingPane() {
  const ws = process.env.HERDR_WORKSPACE_ID ?? "";
  const all = panes();
  const mine = ws ? all.filter((pane) => pane?.workspace_id === ws) : all;
  const from = mine.length > 0 ? mine : all;
  return from.find((pane) => pane?.focused === true) ?? from[0] ?? null;
}

// `target`, when given, is the pane to split BESIDE -- which is how the board
// lands in an existing tab rather than a new one. The cwd hint still comes from
// the invoking pane and not from the target: the repo the board should open on
// is the one you are working in, and the pane being reused may be a shell that
// has wandered off to /tmp.
function openPane(entrypoint, placement, target = null) {
  const from = invokingPane();
  const at = target ?? from;
  const cwd = from?.foreground_cwd || from?.cwd || "";
  // How you name the workspace depends on the placement, and there is no form
  // that works for both: a split "targets an existing pane; use
  // target_pane_id" and rejects --workspace outright, while a tab rejects
  // --target-pane and --direction. Both roads lead to the same space.
  const where =
    placement === "tab"
      ? ["--placement", "tab", ...(process.env.HERDR_WORKSPACE_ID ? ["--workspace", process.env.HERDR_WORKSPACE_ID] : [])]
      : [
          "--placement",
          "split",
          "--direction",
          "right",
          ...(at?.pane_id ? ["--target-pane", at.pane_id] : []),
        ];
  const args = [
    "plugin",
    "pane",
    "open",
    "--plugin",
    PLUGIN_ID,
    "--entrypoint",
    entrypoint,
    ...where,
    ...(cwd ? ["--env", `HERDR_DRIP_BEADS_CWD=${cwd}`] : []),
    "--focus",
  ];
  const out = herdr(args);
  return out?.result?.plugin_pane?.pane?.pane_id ?? null;
}

// herdr splits right and down only, so the left edge is reached by swapping
// leftwards until there is nothing left to swap with. `pane neighbor` answers
// with a null neighbor_pane_id rather than failing at the edge, which is what
// ends this loop; the trip count is a backstop against a layout that somehow
// never runs out of left.
function dockLeft(paneId) {
  const target = paneId ? ["--pane", paneId] : ["--current"];
  for (let i = 0; i < 8; i += 1) {
    const neighbor = herdr(["pane", "neighbor", "--direction", "left", ...target]);
    if (!neighbor?.result?.neighbor?.neighbor_pane_id) return;
    if (herdr(["pane", "swap", "--direction", "left", ...target]) === null) return;
  }
}

function main() {
  const which = process.argv[2];
  if (which !== "dock" && which !== "tab") {
    console.error("usage: bun bin/open.js dock|tab");
    process.exit(2);
  }

  // Both keys toggle, and neither is the same toggle, because the two
  // entrypoints differ in the one thing a toggle depends on: whether you can SEE
  // what the key would act on from where you pressed it.
  const open = ourPanes(which);

  if (which === "dock") {
    return toggleDock(open);
  }
  return toggleTabBoard(open);
}

// The dock is furniture in ONE tab, and every pane in that tab is looking at
// it, so the press that means "not now" is a close and it takes one press.
//
// Scoped to the TAB rather than to the space, which is narrower than this used
// to be: a dock open in another tab is not on your screen, so closing it from
// here was a keypress with no visible effect and a board thrown away. That one
// is left alone and this tab gets its own. `ourPanes` has already kept us out of
// other spaces; HERDR_TAB_ID is the tab the action was invoked from, with the
// invoking pane's own tab as the fallback for an invocation that carries no tab.
function toggleDock(open) {
  const tab = process.env.HERDR_TAB_ID || invokingPane()?.tab_id || "";
  const here = tab ? open.filter((pane) => pane?.tab_id === tab) : open;
  if (here.length > 0) {
    for (const pane of here) herdr(["pane", "close", pane.pane_id]);
    return;
  }

  // No tab to reuse on this road: the dock is a split in the tab you are in, so
  // the pane it lands beside is the one you pressed the key from either way.
  const paneId = openPane("dock", "split");
  dockLeft(paneId);
  // Split panes cannot be sized on open (width/height are popup-only), so the
  // column is narrowed afterwards. The direction is the dock's own side: it is
  // on the left now, so its inner edge moves left to make it thinner. `amount`
  // is a DELTA and a split starts at 50/50, so 0.25 lands the dock at about a
  // quarter of the tab -- enough for a glyph, an id and most of a title, which
  // is what the list view is.
  herdr([
    "pane",
    "resize",
    "--direction",
    "left",
    "--amount",
    "0.25",
    ...(paneId ? ["--pane", paneId] : ["--current"]),
  ]);
}

// The tab board is the opposite case: it is a whole tab, so from anywhere else
// in the space you cannot see it at all. Pressed ON it the key dismisses it;
// pressed anywhere else it SUMMONS the one that exists rather than opening a
// second -- which is what a plain close made impossible, since from another tab
// it silently threw away the board you had open, its scroll, its selection and
// its detail pane, to no visible effect. One key still both summons and
// dismisses, which is why this is a key and not a menu item; the pane under your
// cursor is what decides which.
//
// Every copy goes when it does go, so a space that somehow collected two boards
// is tidied by the press that dismisses them instead of needing one press each.
function toggleTabBoard(open) {
  if (open.length > 0) {
    if (open.some((pane) => pane?.focused === true)) {
      for (const pane of open) herdr(["pane", "close", pane.pane_id]);
    } else {
      focusPane(open[0]);
    }
    return;
  }

  // A beads tab holding something else gets the board back in the tab it is
  // already named after -- see strayBoardPane for what is and is not knowable
  // about that something else. A split, and never a close.
  const stray = strayBoardPane();
  if (stray) {
    openPane("tab", "split", stray);
    return;
  }
  openPane("tab", "tab");
}

main();
