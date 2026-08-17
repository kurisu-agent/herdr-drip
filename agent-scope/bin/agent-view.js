// Set the sidebar's agent view over herdr's API socket.
//
//   bun agent-view.js current   agents whose workspace is the one on screen
//   bun agent-view.js all       every space's agents, still newest-reply-first
//
// Why a script and not the CLI: `agent.view.set` has no `herdr` verb (see
// `herdr agent --help`), so the only way to reach it is the socket the CLI
// itself speaks -- one JSON line in, one JSON line out. herdr hands every
// plugin command HERDR_SOCKET_PATH pointing at the session's own socket, so
// this works under a named session without knowing its name.
//
// `current` is a FILTER, not a snapshot: `context: current_workspace_id` is
// resolved by herdr on every render against the workspace being presented
// (the active one, or the selected one while navigating), so this is invoked
// once and the panel keeps following focus.
//
// BOTH scopes set a view now, and `all` no longer clears one. herdr holds
// exactly ONE agent view -- `AppState.agent_view_override` is a single slot,
// last writer wins -- so the ORDER of the list has to travel in the same
// object as the filter or it cannot exist at all. The order is the drip's
// default (most recently replied at the top, drip.reply-age's `since_key`),
// and a default is not a property of one scope: widening the list to every
// space is not a reason to go back to herdr's stock ordering.

const TIMEOUT_MS = 5000;

// The scope filter's own source. Two halves answer to it -- this plugin and
// the sidebar-scope-icon patch, which reads `agent_view_override.source` to
// decide which glyph the agent panel's header shows -- so what is set under
// this name means "the list is narrowed to this space", and nothing else.
const SCOPE_SOURCE = "plugin:drip.agent-scope";

// The every-space view's source, and deliberately NOT the one above.
//
// In `all` there is no scope filter left; the only thing the view still
// carries is drip.reply-age's ordering, so that plugin is who owns it. Two
// consequences, both wanted: herdr clears a plugin-owned view when its plugin
// is removed or disabled, so a sort over tokens nobody reports any more goes
// away with the reporter -- and `drip_scope_is_current()` in the icon patch
// keeps answering honestly, because a view under a source that is not
// SCOPE_SOURCE is not this plugin claiming the list is narrowed. Reusing
// SCOPE_SOURCE here would leave the header showing the one-window glyph while
// the list showed every space, which is the icon lying about the only thing
// it is there to say.
const SORT_SOURCE = "plugin:drip.reply-age";

// Ascending on the padded elapsed-seconds token: smallest elapsed = replied
// most recently = top. Padded because herdr compares token sort values as
// STRINGS (`sort_value` returns `EvalValue::String`), so `"3"` would sort
// after `"10"`; drip.reply-age reports nine fixed digits for exactly this.
//
// Agents with no token -- a pane the watcher has not reached yet, a non-agent
// kind, or every row at once if the watcher died and the TTLs ran out -- land
// at the BOTTOM without our asking. herdr orders a missing sort value last in
// both directions: `compare_optional_values` reverses for `desc` only inside
// the both-present arm, and answers `(None, Some) => Greater` outside it. So
// there is nothing to encode here -- and nothing SHOULD be encoded, because a
// sentinel key for "unknown" would be indistinguishable from an agent that
// genuinely replied that long ago.
const SORT = [{ field: { token: "since_key" }, order: "asc" }];

const scope = process.argv[2] === "all" ? "all" : "current";
const socketPath =
  process.env.HERDR_SOCKET_PATH ||
  `${process.env.XDG_CONFIG_HOME || `${process.env.HOME}/.config`}/herdr/herdr.sock`;

const currentRequest = {
  method: "agent.view.set",
  params: {
    source: SCOPE_SOURCE,
    // No label on purpose. herdr draws the active view's label (or the word
    // "filtered") in the agent panel's corner, and this view is on by default
    // -- a permanent caption is exactly the sidebar chrome the drip's
    // quiet-chrome patch takes out.
    filter: {
      op: "eq",
      field: "workspace_id",
      value: { context: "current_workspace_id" },
    },
    sort: SORT,
  },
};

const allRequest = {
  method: "agent.view.set",
  params: { source: SORT_SOURCE, sort: SORT },
};

// If drip.reply-age is not installed, herdr refuses a view owned by it
// (`plugin_not_found` / `plugin_disabled`) -- and rightly: there would be no
// `since_key` to sort on either. Falling back to a clear puts `all` back to
// exactly what it did before the sort existed, rather than leaving the scope
// filter in place and the toggle looking broken. Scoped to OUR source: an
// unconditional clear would also drop a view some other plugin set, which is
// not this toggle's business.
const allFallback = {
  method: "agent.view.clear",
  params: { source: SCOPE_SOURCE },
};

// One connection per call. `agent.view.set` is not a stream and the fallback
// above is a second, rarely-taken call -- reconnecting costs a unix socket
// handshake and buys not having to demultiplex two answers on one buffer.
async function call(request) {
  // The handlers close over this rather than hanging it off `socket.data`:
  // `close` and `error` can fire before Bun.connect's promise resolves, and a
  // handler that reaches for a field assigned after the await is a race that
  // shows up as a hang on exactly the hosts where the socket is stale.
  let settle;
  let fail;
  let buffer = "";
  const answer = new Promise((resolve, reject) => {
    settle = resolve;
    fail = reject;
  });

  let socket;
  try {
    socket = await Bun.connect({
      unix: socketPath,
      socket: {
        data(sock, chunk) {
          buffer += chunk.toString();
          const newline = buffer.indexOf("\n");
          if (newline >= 0) {
            settle(buffer.slice(0, newline));
            sock.end();
          }
        },
        error(_sock, err) {
          fail(err);
        },
        close() {
          // A close before a newline is still an answer -- an empty one.
          settle(buffer);
        },
      },
    });
  } catch (err) {
    console.error(`agent-scope: cannot reach herdr at ${socketPath}: ${err}`);
    process.exit(1);
  }

  socket.write(`${JSON.stringify({ id: `agent-scope-${process.pid}`, ...request })}\n`);

  const line = await Promise.race([
    answer,
    new Promise((_, reject) =>
      setTimeout(() => reject(new Error(`no answer in ${TIMEOUT_MS}ms`)), TIMEOUT_MS),
    ),
  ]);

  if (!line.trim()) {
    console.error("agent-scope: herdr closed the socket without answering");
    process.exit(1);
  }
  return JSON.parse(line);
}

async function main() {
  let response = await call(scope === "current" ? currentRequest : allRequest);
  if (
    scope === "all" &&
    response.error &&
    (response.error.code === "plugin_not_found" || response.error.code === "plugin_disabled")
  ) {
    console.error(
      `agent-scope: drip.reply-age is ${response.error.code === "plugin_disabled" ? "disabled" : "not installed"}; every space without the reply-age sort`,
    );
    response = await call(allFallback);
  }
  if (response.error) {
    console.error(`agent-scope: ${response.error.code}: ${response.error.message}`);
    process.exit(1);
  }
  console.log(scope === "current" ? "agents: only this space" : "agents: every space");
}

main().catch((err) => {
  console.error(`agent-scope: ${err.message ?? err}`);
  process.exit(1);
});
