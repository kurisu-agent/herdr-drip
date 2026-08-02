import React, { useEffect, useRef, useState } from "react";
import { render, Box, Text, Spacer, useInput, useApp, useStdout } from "ink";
import { $ } from "bun";
import { collect, type Snapshot, type Agent } from "./data.ts";

const REFRESH_MS = 2000;
const herdr = process.env.HERDR_BIN_PATH ?? "herdr";

const STATUS: Record<string, { icon: string; color: string }> = {
  working: { icon: "●", color: "yellow" },
  idle: { icon: "●", color: "green" },
  blocked: { icon: "●", color: "red" },
  done: { icon: "●", color: "blue" },
  unknown: { icon: "○", color: "gray" },
};

function fmt(sec: number): string {
  if (sec < 60) return `${sec}s`;
  if (sec < 3600) return `${Math.floor(sec / 60)}m${sec % 60 ? String(sec % 60).padStart(2, "0") + "s" : ""}`;
  return `${Math.floor(sec / 3600)}h${String(Math.floor((sec % 3600) / 60)).padStart(2, "0")}m`;
}

/** Time each agent has spent in its current status, tracked across refreshes. */
class StatusClock {
  private seen = new Map<string, { status: string; since: number }>();
  update(agents: Agent[]) {
    const now = Date.now();
    const live = new Set<string>();
    for (const a of agents) {
      live.add(a.paneId);
      const prev = this.seen.get(a.paneId);
      if (!prev || prev.status !== a.status)
        this.seen.set(a.paneId, { status: a.status, since: now });
    }
    for (const id of this.seen.keys()) if (!live.has(id)) this.seen.delete(id);
  }
  inStatusSec(paneId: string): number {
    const e = this.seen.get(paneId);
    return e ? Math.floor((Date.now() - e.since) / 1000) : 0;
  }
}

interface Row {
  key: string;
  onClick?: () => void;
  render: (hovered: boolean) => React.ReactNode;
}

const blank = (key: string): Row => ({ key, render: () => <Text> </Text> });

function App() {
  const { exit } = useApp();
  const { stdout } = useStdout();
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [updatedAt, setUpdatedAt] = useState("");
  const [showGraph, setShowGraph] = useState(false);
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const [hover, setHover] = useState(-1);
  const [scroll, setScroll] = useState(0);
  const clock = useRef(new StatusClock());
  const rowsRef = useRef<Row[]>([]);
  const scrollRef = useRef(0);
  scrollRef.current = scroll;

  const workspaceId =
    process.env.WORKTREE_GRAPH_WORKSPACE ??
    process.env.HERDR_WORKSPACE_ID ??
    "";

  const refresh = () =>
    collect(workspaceId)
      .then((s) => {
        clock.current.update(s.worktrees.flatMap((w) => w.agents));
        setSnapshot(s);
        setError(null);
        setUpdatedAt(new Date().toLocaleTimeString());
      })
      .catch((e) => setError(String(e)));

  useEffect(() => {
    refresh();
    const timer = setInterval(refresh, REFRESH_MS);
    return () => clearInterval(timer);
  }, []);

  // Mouse: SGR reporting straight from the pty — Ink has no mouse layer.
  useEffect(() => {
    process.stdout.write("\x1b[?1002;1003;1006h");
    const onData = (buf: Buffer) => {
      for (const m of buf.toString().matchAll(/\x1b\[<(\d+);(\d+);(\d+)([Mm])/g)) {
        const [, code, , yRaw, kind] = m;
        const b = Number(code);
        const idx = Number(yRaw) - 1 + scrollRef.current;
        if (b === 64) setScroll((s) => Math.max(0, s - 2));
        else if (b === 65)
          setScroll((s) =>
            Math.min(Math.max(0, rowsRef.current.length - 5), s + 2),
          );
        else if (b === 35 || (b & 32) === 32) setHover(idx);
        else if (b === 0 && kind === "M") rowsRef.current[idx]?.onClick?.();
      }
    };
    process.stdin.on("data", onData);
    return () => {
      process.stdin.off("data", onData);
      process.stdout.write("\x1b[?1002;1003;1006l");
    };
  }, []);

  useInput((input) => {
    if (input === "q") {
      process.stdout.write("\x1b[?1002;1003;1006l");
      exit();
    }
    if (input === "r") refresh();
    if (input === "g") setShowGraph((v) => !v);
  });

  if (error)
    return (
      <Box padding={1}>
        <Text color="red" wrap="truncate-end">{error}</Text>
      </Box>
    );
  if (!snapshot)
    return (
      <Box padding={1}>
        <Text dimColor>reading worktrees…</Text>
      </Box>
    );

  const rows: Row[] = [];
  rows.push({
    key: "header",
    render: () => (
      <Box>
        <Text bold>{snapshot.repoName}</Text>
        <Text dimColor> {workspaceId}</Text>
        <Spacer />
        <Text dimColor>{updatedAt} · g graph · q quit</Text>
      </Box>
    ),
  });
  rows.push(blank("b0"));

  for (const wt of snapshot.worktrees) {
    const open = !collapsed.has(wt.path);
    const lead = wt.agents[0]
      ? STATUS[wt.agents[0].status] ?? STATUS.unknown
      : { icon: "○", color: "gray" };
    rows.push({
      key: wt.path,
      onClick: () =>
        setCollapsed((c) => {
          const next = new Set(c);
          next.has(wt.path) ? next.delete(wt.path) : next.add(wt.path);
          return next;
        }),
      render: (h) => (
        <Text wrap="truncate-end" inverse={h}>
          <Text color={lead.color}>{lead.icon}</Text>
          <Text bold> {wt.branch ?? wt.name}</Text>
          {wt.ahead > 0 && <Text color="green"> +{wt.ahead}</Text>}
          {wt.behind > 0 && <Text color="red"> -{wt.behind}</Text>}
          {wt.dirty > 0 && <Text color="yellow"> ±{wt.dirty}</Text>}
          {!open && <Text dimColor> …</Text>}
        </Text>
      ),
    });
    if (!open) continue;
    for (const a of wt.agents) {
      const s = STATUS[a.status] ?? STATUS.unknown;
      const inStatus = clock.current.inStatusSec(a.paneId);
      rows.push({
        key: a.paneId,
        onClick: () => void $`${herdr} agent focus ${a.paneId}`.quiet().nothrow(),
        render: (h) => (
          <Text wrap="truncate-end" inverse={h}>
            <Text dimColor>│ </Text>
            <Text color={s.color}>{a.status}</Text>
            <Text color={s.color} dimColor> {fmt(inStatus)}</Text>
            {a.uptimeSec != null && <Text dimColor> · up {fmt(a.uptimeSec)}</Text>}
            <Text> {a.title ?? ""}</Text>
            {h && <Text dimColor> ↵ focus</Text>}
          </Text>
        ),
      });
    }
    for (const [i, c] of wt.commits.entries()) {
      rows.push({
        key: `${wt.path}:${c.hash}`,
        render: (h) => (
          <Text wrap="truncate-end" inverse={h}>
            <Text dimColor>{i === wt.commits.length - 1 ? "╰" : "│"} </Text>
            <Text color="yellow">{c.hash}</Text> <Text>{c.subject}</Text>
          </Text>
        ),
      });
    }
    if (wt.isMain)
      rows.push({
        key: `${wt.path}:head`,
        render: (h) => (
          <Text wrap="truncate-end" inverse={h}>
            <Text dimColor>╰ </Text>
            <Text color="yellow">{wt.head}</Text> <Text>{wt.subject}</Text>
          </Text>
        ),
      });
  }

  if (showGraph) {
    rows.push(blank("b1"));
    for (const [i, line] of snapshot.graph.entries()) {
      const m = line.match(/^([ *|\\/_.-]*)([0-9a-f]{7,40})?\s?(\([^)]*\))?\s?(.*)$/);
      rows.push({
        key: `g${i}`,
        render: (h) =>
          m ? (
            <Text wrap="truncate-end" inverse={h}>
              <Text dimColor>{m[1]}</Text>
              {m[2] && <Text color="yellow">{m[2]}</Text>}
              {m[3] && <Text color="cyan"> {m[3]}</Text>}
              {m[4] && <Text> {m[4]}</Text>}
            </Text>
          ) : (
            <Text wrap="truncate-end">{line}</Text>
          ),
      });
    }
  }

  rowsRef.current = rows;
  const windowRows = Math.max(5, (stdout?.rows ?? 40) - 1);
  const top = Math.min(scroll, Math.max(0, rows.length - windowRows));
  const visible = rows.slice(top, top + windowRows);

  return (
    <Box flexDirection="column" paddingX={1}>
      {visible.map((row, i) => (
        <React.Fragment key={row.key}>
          {row.render(top + i === hover && !!row.onClick)}
        </React.Fragment>
      ))}
    </Box>
  );
}

render(<App />);
