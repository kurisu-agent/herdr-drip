// The beads rail's contents: one line per bead, worst status first.
//
// A RE-IMPLEMENTATION of the read half of herdr-beads
// (https://github.com/miiraheart/herdr-beads, MIT). The `bd` bridge below is
// that project's `src/bd/mod.rs` in JavaScript -- same subcommand, same
// argv-vector discipline, same PATH-widening for a CLI that is usually
// installed somewhere herdr's minimal launch environment cannot see -- and the
// status vocabulary is its `src/model.rs`. What is different is the shape of
// the answer: herdr-beads renders a board into a pane, this renders a list
// into 26 columns of sidebar. See ../nix/sidebar-beads.rs for the rest of the
// credit note.
//
// Writes to stdout; bin/sync owns the file. Prints nothing and exits 0 when
// there is no board to read, which is what makes the rail disappear rather
// than show an error on a box that has never installed bd.

import { existsSync, readFileSync } from "node:fs";
import { dirname } from "node:path";

// bd's status names, abbreviated to the one character the rail's file format
// carries. The set is herdr-beads' STATUS_ORDER plus its `_ => "•"` fallback,
// kept complete so a board using bd's less common states still reads.
const STATUS_CHAR = {
  open: "o",
  in_progress: "i",
  blocked: "b",
  deferred: "d",
  closed: "c",
  pinned: "p",
  hooked: "h",
};

// Rail order, and NOT herdr-beads' STATUS_ORDER, which opens with `open`.
// The rail is the one part of this that gets truncated -- it shows as many
// beads as the sidebar has rows left after the agent panel, and drops the
// rest -- so the order has to be worst-first or the row that gets dropped is
// the blocked one. In a pane the size of herdr-beads' board there is nothing
// to drop and its order reads better; here it would lie.
const RAIL_ORDER = ["blocked", "in_progress", "open", "hooked", "pinned", "deferred", "closed"];

const TIMEOUT_MS = 10_000;

// The plugin's config file -- the SAME file the board reads, which is the
// whole point of it: `../board/src/config.rs` documents the format and both
// surfaces answer to it, so there is no way for the rail and the board to
// disagree about what statuses a board has. herdr creates the directory and
// exports it to every plugin command; with the variable unset there is no
// config, and every default below stands.
//
// Read leniently, like everything else here: a config that will not parse is
// a rail that draws normally, not a rail that vanishes with a stack trace in
// the server log every fifteen seconds.
function readConfig() {
  const explicit = process.env.HERDR_DRIP_BEADS_CONFIG;
  const dir = process.env.HERDR_PLUGIN_CONFIG_DIR;
  const path = explicit || (dir ? `${dir}/config.json` : null);
  if (!path) return {};
  try {
    const parsed = JSON.parse(readFileSync(path, "utf8"));
    return typeof parsed === "object" && parsed !== null && !Array.isArray(parsed) ? parsed : {};
  } catch {
    return {};
  }
}

const CONFIG = readConfig();

// Which statuses may appear, or `null` for "everything the board has". Env
// (comma-separated) beats file, which is the layering every other knob here
// has and what lets a nix module set this without writing a file.
//
// Unset means unset, NOT "everything in RAIL_ORDER": a board with a custom
// status has always had it on the rail, ranked last, and an allowlist that
// silently dropped it would be a filter nobody asked for. Where the board
// reads this list as an ORDER, the rail reads it as a filter -- a rail is a
// handful of rows, so a vocabulary is the only thing it can be.
function configuredStatuses() {
  const fromEnv = process.env.HERDR_DRIP_BEADS_STATUSES;
  const raw = fromEnv !== undefined && fromEnv !== ""
    ? fromEnv.split(",")
    : Array.isArray(CONFIG.statuses)
      ? CONFIG.statuses
      : null;
  if (!raw) return null;
  const clean = raw.map((s) => (typeof s === "string" ? s.trim() : "")).filter(Boolean);
  return clean.length > 0 ? clean : null;
}

// Closed beads are what is DONE, and the rail is what is left, so they are off
// unless asked for -- the same default `show_closed` gives the board.
function showClosed() {
  const fromEnv = process.env.HERDR_DRIP_BEADS_SHOW_CLOSED;
  if (fromEnv !== undefined && fromEnv !== "") {
    return ["1", "true", "yes"].includes(fromEnv.toLowerCase());
  }
  return CONFIG.show_closed === true;
}

// `hasOwn` rather than a bare lookup: `STATUS_CHAR` is an object literal, so a
// bead whose status is `toString` or `constructor` would otherwise resolve to
// an inherited function and stringify into the middle of the line -- and
// `Object.prototype.toString` carries newlines, which would split one bead
// across three rows of a file whose whole format is one line per bead.
function statusChar(status) {
  return Object.hasOwn(STATUS_CHAR, status) ? STATUS_CHAR[status] : "-";
}

function statusRank(status) {
  const at = RAIL_ORDER.indexOf(status);
  return at === -1 ? RAIL_ORDER.length : at;
}

function priorityChar(priority) {
  return Number.isInteger(priority) && priority >= 0 && priority <= 4 ? String(priority) : "-";
}

// bd counts 0 as most urgent, so priority sorts ascending. A bead with no
// usable priority sorts LAST, which is where this parts company with
// herdr-beads: it deserializes the field as a typed integer defaulting to 0 and
// so cannot tell absent from urgent, while this reads raw JSON and can. Ranking
// an unranked bead above the blocked ones would be the wrong guess.
function priorityOf(priority) {
  return Number.isInteger(priority) ? priority : 99;
}

// stdout on success, `null` when the command failed, MISSING when there was no
// such command to run. Bun throws ENOENT out of spawnSync rather than returning
// it, and an uncaught throw here would put a stack trace in the herdr server's
// log every tick on every box that has never installed bd -- which is most of
// them, and is not an error.
const MISSING = Symbol("missing");

function run(cmd, args, opts = {}) {
  let proc;
  try {
    proc = Bun.spawnSync([cmd, ...args], {
      stdout: "pipe",
      stderr: "pipe",
      timeout: TIMEOUT_MS,
      ...opts,
    });
  } catch {
    return MISSING;
  }
  if (proc.exitCode !== 0) {
    return null;
  }
  return proc.stdout.toString();
}

// Where the board is. herdr runs plugin commands in the PLUGIN's directory,
// which has no `.beads`, so the repo has to be asked for -- the same problem
// herdr-beads solves in `resolve_repo_cwd`, and the same answer: ask herdr
// which pane is focused and use its directory. `foreground_cwd` first because
// a shell that has cd'd into a worktree is still reported at its launch `cwd`,
// and the worktree is the board you are actually looking at.
// Returns the directory, `null` for "asked, and no pane is on a board", or
// UNREADABLE for "could not ask" -- which main() must not confuse with an
// empty board, for the same reason loadBeads distinguishes its two empties.
const UNREADABLE = Symbol("unreadable");

function boardCwd() {
  const pinned = process.env.HERDR_DRIP_BEADS_CWD;
  if (pinned) return pinned;

  const bin = process.env.HERDR_BIN_PATH || "herdr";
  const out = run(bin, ["pane", "list"]);
  if (typeof out !== "string" || !out) return UNREADABLE;

  let panes;
  try {
    panes = JSON.parse(out)?.result?.panes;
  } catch {
    return UNREADABLE;
  }
  if (!Array.isArray(panes)) return UNREADABLE;

  const me = process.env.HERDR_PANE_ID ?? "";
  let best = null;
  let bestScore = -1;
  for (const pane of panes) {
    if (pane?.pane_id === me) continue;
    // Where the shell went beats where it was launched, but only while it is
    // still on a board: a pane that cd'd out to /tmp should not blank a rail
    // its launch directory can still fill. Falling back on falsiness alone
    // would miss that, since an off-board path is a perfectly truthy string.
    const dir = [pane?.foreground_cwd, pane?.cwd].find((d) => d && hasBoard(d))
      ?? (pane?.foreground_cwd || pane?.cwd);
    if (!dir) continue;
    // A real board outranks everything; among boards, the focused pane wins.
    // Without the `.beads` test a focused pane in some unrelated directory
    // would blank a rail that a background pane could have filled.
    const score = (hasBoard(dir) ? 4 : 0) + (pane.focused === true ? 2 : 0);
    if (score > bestScore) {
      bestScore = score;
      best = dir;
    }
  }
  return bestScore >= 4 ? best : null;
}

// Walks up, the way bd itself finds a board and git finds `.git`. Testing only
// the directory handed in would score a pane sitting three levels inside a
// checkout at zero and blank a rail that bd would have answered from that exact
// cwd -- and cd'ing around inside a repo is the normal way to use one.
function hasBoard(dir) {
  let at = dir;
  for (;;) {
    if (existsSync(`${at}/.beads`)) return true;
    const up = dirname(at);
    if (up === at) return false;
    at = up;
  }
}

// herdr launches plugins with a minimal PATH and bd is typically a Homebrew or
// /usr/local install. herdr-beads widens the same directories in the child's
// environment AND resolves the binary against a hard-coded list in its
// `src/bd/mod.rs`; the linuxbrew entry below is from that list, and is the one
// of them that is not already on every PATH. Nix boxes get bd from PATH like
// anything else.
function bdPath() {
  const extra = ["/opt/homebrew/bin", "/usr/local/bin", "/home/linuxbrew/.linuxbrew/bin"];
  return [...extra, process.env.PATH ?? ""].filter(Boolean).join(":");
}

function bdList(cwd, args) {
  const out = run(process.env.HERDR_DRIP_BD_BIN || "bd", ["list", ...args, "--json"], {
    cwd,
    env: { ...process.env, PATH: bdPath() },
  });
  // No bd on this box is not a failure to report every fifteen seconds, it is
  // a board that does not exist -- the same shape gumbo-usage's rail has when
  // gumbo is not installed. An empty rail is the correct answer and there is
  // nothing to turn off.
  if (out === MISSING) return [];
  if (out === null) return null;
  const text = out.trim();
  if (!text) return [];
  try {
    const parsed = JSON.parse(text);
    // Well-formed JSON that is not an array is bd answering in a shape this
    // does not know -- a future envelope, an error object. That is the `null`
    // case, not the empty-board case: reporting it as `[]` would blank the rail
    // on every tick with nothing in the log to say why.
    return Array.isArray(parsed) ? parsed : null;
  } catch {
    return null;
  }
}

function loadBeads(cwd, wantClosed) {
  const beads = bdList(cwd, []);
  if (beads === null || !wantClosed) return beads;
  // `bd list` omits closed issues, so showing them takes a second ask -- the
  // board's `bd::load` merges exactly this query for exactly this reason.
  // Best-effort: a board that cannot answer the second question is still a
  // board, and the open beads are the ones the rail exists for.
  const closed = bdList(cwd, ["--status", "closed"]);
  if (!Array.isArray(closed)) return beads;
  const have = new Set(beads.map((bead) => bead?.id));
  return [...beads, ...closed.filter((bead) => !have.has(bead?.id))];
}

function positiveInt(value, fallback) {
  const parsed = Number.parseInt(value ?? "", 10);
  return Number.isInteger(parsed) && parsed > 0 ? parsed : fallback;
}

// The totals line: `#<blocked> <in progress> <open>`, first line of the file.
//
// It exists because the rail's summary counts and the rail's rows are the same
// data, and the row cap above would otherwise cap them both -- "○5" on a board
// with forty open beads, which is a summary that tells you the size of its own
// truncation. So the counts travel separately: the rail draws five rows and
// says how many there are.
//
// Counted over what the rail WOULD show uncapped -- after the status filter,
// before the cap -- so a filtered rail reports its own vocabulary rather than
// counts it is not drawing.
//
// `#` cannot collide with a bead line: a bead's first character is a status
// letter from a closed set, so an OLD reader drops this line as malformed and
// draws the rail exactly as it did before, and a new reader with an old
// writer's file finds no totals and counts the lines. Neither half has to know
// what the other is.
function totalsLine(beads) {
  const count = (status) => beads.filter((bead) => bead?.status === status).length;
  return `#${count("blocked")} ${count("in_progress")} ${count("open")}`;
}

// One line per bead: `<status><priority> <text>`, the format nix/sidebar-beads.rs
// parses. Newlines cannot survive a line-per-bead file and a title carrying one
// would silently become two beads, so they collapse to spaces here rather than
// being dropped by the reader.
function railLine(bead) {
  // `null` is an element bd should never emit, but an unguarded `bead.id`
  // throws on it, and an escaping throw is a stack trace in the herdr server's
  // log every fifteen seconds -- the thing the MISSING sentinel above exists to
  // avoid. A bead with no id and no title drops out below anyway.
  if (typeof bead !== "object" || bead === null) return null;
  const id = typeof bead.id === "string" ? bead.id : "";
  const title = typeof bead.title === "string" ? bead.title : "";
  const text = `${id} ${title}`.replace(/\s+/g, " ").trim();
  if (!text) return null;
  return `${statusChar(bead.status)}${priorityChar(bead.priority)} ${text}`;
}

function main() {
  const cwd = boardCwd();
  if (cwd === UNREADABLE) process.exit(1);
  // No pane is sitting on a board: an empty rail is the honest answer, and is
  // how the rail gets out of the way when you move to a repo without one.
  if (!cwd) return;

  const allowed = configuredStatuses();
  const closedToo = showClosed();

  const beads = loadBeads(cwd, closedToo);
  // The two empty answers are NOT the same, and the exit code is the only
  // place the difference survives. `[]` is "this board has nothing on it", so
  // an empty rail is the truth and bin/sync should write it. `null` is "bd
  // could not answer" -- absent, erroring, mid-migration -- where an empty
  // rail is a lie, so fail and let bin/sync keep the last board it had.
  if (beads === null) process.exit(1);
  if (beads.length === 0) return;

  // Two ceilings, and they mean different things.
  //
  // ROWS is the rail's SHAPE: the open rail asks for a row per line it is
  // given, so without a cap a forty-bead board pushes the agent panel down to
  // its floor. Five is a glance -- worst first, so those five are the ones you
  // would have read anyway.
  //
  // LIMIT is the old outer ceiling, and it stays for what it was for: not
  // re-reading a thousand-bead board from disk twice a second. The smaller of
  // the two wins, so setting LIMIT low still works and setting it high does
  // not un-cap the rail.
  const rows = positiveInt(process.env.HERDR_DRIP_BEADS_ROWS ?? CONFIG.rail_rows, 5);
  const limit = positiveInt(process.env.HERDR_DRIP_BEADS_LIMIT, 40);
  const cap = Math.min(rows, limit);

  // `closed` is gated by its own setting even when listed, so that one key
  // means the same thing on both surfaces: the board special-cases closed in
  // exactly this way (`board_statuses`), and a vocabulary that happens to
  // mention it must not quietly turn it on.
  const keep = (bead) => {
    const status = bead?.status;
    if (!closedToo && status === "closed") return false;
    return allowed === null || allowed.includes(status);
  };

  const showing = beads.filter(keep).sort((a, b) => {
    const byStatus = statusRank(a?.status) - statusRank(b?.status);
    if (byStatus !== 0) return byStatus;
    const byPriority = priorityOf(a?.priority) - priorityOf(b?.priority);
    if (byPriority !== 0) return byPriority;
    return String(b?.updated_at ?? "").localeCompare(String(a?.updated_at ?? ""));
  });

  const lines = showing
    .map(railLine)
    .filter((line) => line !== null)
    .slice(0, cap);

  if (lines.length > 0) {
    process.stdout.write(`${totalsLine(showing)}\n${lines.join("\n")}\n`);
  }
}

main();
