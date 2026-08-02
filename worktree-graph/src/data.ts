import { $ } from "bun";
import { basename, dirname, resolve } from "node:path";

const herdr = process.env.HERDR_BIN_PATH ?? "herdr";

export interface Agent {
  paneId: string;
  status: string;
  title: string | null;
  worktree: string | null;
}

export interface Commit {
  hash: string;
  subject: string;
}

export interface Worktree {
  path: string;
  name: string;
  branch: string | null;
  head: string;
  subject: string;
  dirty: number;
  ahead: number;
  behind: number;
  commits: Commit[];
  isMain: boolean;
  agents: Agent[];
}

export interface Snapshot {
  workspaceId: string;
  repoRoot: string;
  repoName: string;
  worktrees: Worktree[];
  graph: string[];
}

async function herdrJson(args: string[]): Promise<any> {
  const out = await $`${herdr} ${args}`.quiet().text();
  return JSON.parse(out).result;
}

async function git(cwd: string, args: string[]): Promise<string> {
  return (await $`git -C ${cwd} ${args}`.quiet().text()).trim();
}

/** The workspace's repo root: any pane cwd -> its main checkout. */
async function findRepoRoot(paneCwds: string[]): Promise<string | null> {
  for (const cwd of paneCwds) {
    try {
      const common = await git(cwd, [
        "rev-parse",
        "--path-format=absolute",
        "--git-common-dir",
      ]);
      return dirname(resolve(common));
    } catch {
      /* not a git dir; try the next pane */
    }
  }
  return null;
}

export async function collect(workspaceId: string): Promise<Snapshot> {
  const panes: any[] = (await herdrJson(["pane", "list"])).panes.filter(
    (p: any) => p.pane_id.startsWith(`${workspaceId}:`),
  );

  const repoRoot = await findRepoRoot(
    panes.flatMap((p) => [p.foreground_cwd, p.cwd]).filter(Boolean),
  );
  if (!repoRoot) throw new Error("no git repo behind this workspace's panes");

  const agents: Agent[] = (await herdrJson(["agent", "list"])).agents
    .filter((a: any) => a.workspace_id === workspaceId)
    .map((a: any) => ({
      paneId: a.pane_id,
      status: a.agent_status,
      title: a.terminal_title_stripped ?? null,
      worktree: a.tokens?.worktree ?? null,
    }));

  let defaultBranch: string | null = null;
  try {
    defaultBranch = await git(repoRoot, ["symbolic-ref", "--short", "HEAD"]);
  } catch {
    /* detached main checkout; skip ahead/behind */
  }

  const porcelain = await git(repoRoot, ["worktree", "list", "--porcelain"]);
  const worktrees: Worktree[] = [];
  for (const block of porcelain.split("\n\n").filter(Boolean)) {
    const field = (key: string) =>
      block
        .split("\n")
        .find((l) => l.startsWith(`${key} `))
        ?.slice(key.length + 1) ?? null;
    const path = field("worktree");
    if (!path) continue;
    const isMain = resolve(path) === resolve(repoRoot);
    const name = basename(path);
    const branch = field("branch")?.replace("refs/heads/", "") ?? null;
    let head = field("HEAD")?.slice(0, 7) ?? "";
    let subject = "";
    let dirty = 0;
    let ahead = 0;
    let behind = 0;
    let commits: Commit[] = [];
    try {
      subject = await git(path, ["log", "-1", "--format=%s"]);
      const status = await git(path, ["status", "--porcelain"]);
      dirty = status ? status.split("\n").length : 0;
      if (!isMain && branch && defaultBranch) {
        const counts = await git(path, [
          "rev-list",
          "--left-right",
          "--count",
          `${defaultBranch}...${branch}`,
        ]);
        [behind, ahead] = counts.split("\t").map(Number);
        const log = await git(path, [
          "log",
          "--no-color",
          "--format=%h\t%s",
          `${defaultBranch}..${branch}`,
          "-n",
          "3",
        ]);
        commits = log
          .split("\n")
          .filter(Boolean)
          .map((l) => {
            const [hash, ...rest] = l.split("\t");
            return { hash, subject: rest.join("\t") };
          });
      }
    } catch {
      /* checkout may be mid-removal; show what we have */
    }
    worktrees.push({
      path,
      name,
      branch,
      head,
      subject,
      dirty,
      ahead,
      behind,
      commits,
      isMain,
      // Agents in a linked worktree carry its name as a token (reported by
      // drip.worktree-tokens); agents without one sit in the main checkout.
      agents: agents.filter((a) =>
        a.worktree ? a.worktree === name : isMain,
      ),
    });
  }

  const format = "--format=%h%d %s";
  const graphOut = await $`git -C ${repoRoot} log --graph --no-color --branches ${format} -n 60`
    .quiet()
    .text();

  return {
    workspaceId,
    repoRoot,
    repoName: basename(repoRoot),
    worktrees,
    graph: graphOut.trimEnd().split("\n"),
  };
}
