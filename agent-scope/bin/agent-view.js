// Set or clear the sidebar's agent view over herdr's API socket.
//
//   bun agent-view.js current   agents whose workspace is the one on screen
//   bun agent-view.js all       no view at all -- herdr's stock agent panel
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
// once and the panel keeps following focus. `all` clears the view rather than
// setting a filter that matches everything -- an inactive view is stock
// behaviour, which is exactly what "every space" means here.

const SOURCE = "plugin:drip.agent-scope";
const TIMEOUT_MS = 5000;

const scope = process.argv[2] === "all" ? "all" : "current";
const socketPath =
  process.env.HERDR_SOCKET_PATH ||
  `${process.env.XDG_CONFIG_HOME || `${process.env.HOME}/.config`}/herdr/herdr.sock`;

const request =
  scope === "current"
    ? {
        id: `agent-scope-${process.pid}`,
        method: "agent.view.set",
        params: {
          source: SOURCE,
          // No label on purpose. herdr draws the active view's label (or the
          // word "filtered") in the agent panel's corner, and this view is on
          // by default -- a permanent caption is exactly the sidebar chrome
          // the drip's quiet-chrome patch takes out.
          filter: {
            op: "eq",
            field: "workspace_id",
            value: { context: "current_workspace_id" },
          },
        },
      }
    : {
        id: `agent-scope-${process.pid}`,
        method: "agent.view.clear",
        // Scoped to OUR source: clearing unconditionally would also drop a
        // view some other plugin set, which is not this toggle's business.
        params: { source: SOURCE },
      };

async function main() {
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

  socket.write(`${JSON.stringify(request)}\n`);

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

  const response = JSON.parse(line);
  if (response.error) {
    console.error(`agent-scope: ${response.error.code}: ${response.error.message}`);
    process.exit(1);
  }
  console.log(
    scope === "current"
      ? "agents: only this space"
      : "agents: every space",
  );
}

main().catch((err) => {
  console.error(`agent-scope: ${err.message ?? err}`);
  process.exit(1);
});
