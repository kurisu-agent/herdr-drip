import React, { useEffect, useState } from "react";
import { render, Box, Text, Spacer, useInput, useApp, useStdout } from "ink";
import { collect, type Snapshot, type Worktree } from "./data.ts";

const REFRESH_MS = 2000;

const STATUS: Record<string, { icon: string; color: string }> = {
  working: { icon: "●", color: "yellow" },
  idle: { icon: "●", color: "green" },
  blocked: { icon: "●", color: "red" },
  done: { icon: "●", color: "blue" },
  unknown: { icon: "○", color: "gray" },
};

function WorktreeCard({ wt }: { wt: Worktree }) {
  const lead = wt.agents[0]
    ? STATUS[wt.agents[0].status] ?? STATUS.unknown
    : { icon: "○", color: "gray" };
  return (
    <Box flexDirection="column">
      <Text wrap="truncate-end">
        <Text color={lead.color}>{lead.icon}</Text>
        <Text bold> {wt.name}</Text>
        {wt.branch && wt.branch !== wt.name && (
          <Text color="cyan"> {wt.branch}</Text>
        )}
        {wt.ahead > 0 && <Text color="green"> +{wt.ahead}</Text>}
        {wt.behind > 0 && <Text color="red"> -{wt.behind}</Text>}
        {wt.dirty > 0 && <Text color="yellow"> ±{wt.dirty}</Text>}
      </Text>
      {wt.agents.map((a) => {
        const s = STATUS[a.status] ?? STATUS.unknown;
        return (
          <Text key={a.paneId} wrap="truncate-end">
            <Text dimColor>│ </Text>
            <Text color={s.color}>{a.status}</Text>
            <Text dimColor> {a.title ?? ""}</Text>
          </Text>
        );
      })}
      {wt.commits.map((c, i) => (
        <Text key={c.hash} wrap="truncate-end">
          <Text dimColor>{i === wt.commits.length - 1 ? "╰" : "│"} </Text>
          <Text color="yellow">{c.hash}</Text> <Text>{c.subject}</Text>
        </Text>
      ))}
      {wt.isMain && (
        <Text wrap="truncate-end">
          <Text dimColor>╰ </Text>
          <Text color="yellow">{wt.head}</Text> <Text>{wt.subject}</Text>
        </Text>
      )}
    </Box>
  );
}

/** One plain graph line, styled by us: glyphs dim, hash yellow, refs cyan. */
function GraphLine({ line }: { line: string }) {
  const m = line.match(/^([ *|\\/_.-]*)([0-9a-f]{7,40})?\s?(\([^)]*\))?\s?(.*)$/);
  if (!m) return <Text wrap="truncate-end">{line}</Text>;
  const [, glyphs, hash, refs, subject] = m;
  return (
    <Text wrap="truncate-end">
      <Text dimColor>{glyphs}</Text>
      {hash && <Text color="yellow">{hash}</Text>}
      {refs && <Text color="cyan"> {refs}</Text>}
      {subject && <Text> {subject}</Text>}
    </Text>
  );
}

function App() {
  const { exit } = useApp();
  const { stdout } = useStdout();
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [updatedAt, setUpdatedAt] = useState<string>("");
  const [showGraph, setShowGraph] = useState(false);

  const workspaceId =
    process.env.WORKTREE_GRAPH_WORKSPACE ??
    process.env.HERDR_WORKSPACE_ID ??
    "";

  const refresh = () =>
    collect(workspaceId)
      .then((s) => {
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

  useInput((input) => {
    if (input === "q") exit();
    if (input === "r") refresh();
    if (input === "g") setShowGraph((v) => !v);
  });

  if (error)
    return (
      <Box padding={1}>
        <Text color="red" wrap="truncate-end">
          {error}
        </Text>
      </Box>
    );
  if (!snapshot)
    return (
      <Box padding={1}>
        <Text dimColor>reading worktrees…</Text>
      </Box>
    );

  const rows = stdout?.rows ?? 40;
  const summaryRows = snapshot.worktrees.reduce(
    (n, wt) => n + 1 + wt.agents.length + wt.commits.length + (wt.isMain ? 1 : 0),
    0,
  );
  const graphBudget = Math.max(0, rows - summaryRows - 4);
  const graph = showGraph ? snapshot.graph.slice(0, graphBudget) : [];

  return (
    <Box flexDirection="column" paddingX={1}>
      <Box>
        <Text bold>{snapshot.repoName}</Text>
        <Text dimColor> {workspaceId}</Text>
        <Spacer />
        <Text dimColor>
          {updatedAt} · g graph · q quit
        </Text>
      </Box>
      <Box flexDirection="column" marginTop={1} gap={0}>
        {snapshot.worktrees.map((wt) => (
          <WorktreeCard key={wt.path} wt={wt} />
        ))}
      </Box>
      {showGraph && (
        <Box flexDirection="column" marginTop={1}>
          {graph.map((line, i) => (
            <GraphLine key={i} line={line} />
          ))}
        </Box>
      )}
    </Box>
  );
}

render(<App />);
