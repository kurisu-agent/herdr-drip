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

// Our own panes, by the OSC title the binary sets from its entrypoint id.
//
// This is the only handle there is: there is no plugin.pane.list RPC, and
// PaneInfo carries no plugin_id, so a pane cannot be asked which plugin opened
// it. Scoped to this workspace because that is what a toggle means here -- the
// dock is per-tab furniture and a tab is per-workspace, and closing the copy
// you have open in another space would be a surprise from a keypress.
function ourPanes(entrypoint) {
  const marker = `herdr-beads-${entrypoint}`;
  const ws = process.env.HERDR_WORKSPACE_ID ?? "";
  return panes()
    .filter((pane) => {
      const title = `${pane?.terminal_title ?? ""} ${pane?.terminal_title_stripped ?? ""}`;
      if (!title.includes(marker)) return false;
      return !ws || pane?.workspace_id === ws;
    })
    .map((pane) => pane?.pane_id)
    .filter((id) => typeof id === "string" && id);
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

function openPane(entrypoint, placement) {
  const from = invokingPane();
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
          ...(from?.pane_id ? ["--target-pane", from.pane_id] : []),
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

  // Toggle. A second press closes what the first opened, which is the whole
  // reason this is bound to a key rather than being a menu item.
  const open = ourPanes(which);
  if (open.length > 0) {
    for (const id of open) herdr(["pane", "close", id]);
    return;
  }

  if (which === "tab") {
    openPane("tab", "tab");
    return;
  }

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

main();
