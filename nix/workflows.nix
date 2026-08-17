# workflows — a way of working, turned on or off in one place.
#
# A workflow is a habit this drip's hosts have, and a habit has two halves that
# have always been provisioned separately: the AGENTS have to know about it,
# and the MACHINE has to have the things it needs. Beads is the clearest case.
# Turning it on meant adding `beads` to `plugins.plugins`, pointing
# `plugins.beadsPackage` at a `bd`, and — in an entirely different repo, in a
# free-text blob on the host — writing a paragraph into CLAUDE.md telling the
# agent the rail exists and how to read it. Turning it off meant remembering
# all three. Nobody ever did; what actually happened is that the prose outlived
# the install, and an agent read confident instructions about a rail that was
# not there.
#
# So a workflow here owns BOTH halves. `enable` gates the guidance and the
# install together, because they were never independently useful:
#
#   services.herdr-drip.workflows.beads.enable = false;
#
# drops the plugin, drops the `bd`, and drops the paragraph, in one edit that
# cannot half-apply.
#
# HOW THE GUIDANCE GETS OUT. This module does not write CLAUDE.md — it has no
# opinion about where a host keeps one, and hosts differ. It renders the
# enabled workflows' prose into `services.herdr-drip.workflowGuidance` and
# stops there. The host wires that into whatever renders its context file; on
# a peach-beach-shaped host that is one line:
#
#   services.claudeContext.extraSections = config.services.herdr-drip.workflowGuidance;
#
# Deliberately a pull and not a push. Writing into `services.claudeContext`
# from here would make this module fail to evaluate on every host that does not
# have that module — which is most of them, since claude-context.nix lives in
# one machine's /etc/nixos and this flake ships to the fleet. A read-only
# output that a host may or may not consume couples nothing.
#
# WHY THE PROSE LIVES IN THIS REPO. It is knowledge about the drip — what the
# beads rail is, what the panes do, how an agent should hand one back. It
# changes when the drip changes, in the same commit, and every host that takes
# the flake gets the new wording without anyone editing a host file. The prose
# that stays on the host is the prose about the HOST: its paths, its circuit,
# its pins.
#
# ADDING ONE. A new workflow is an attribute in `workflows` below with an
# `enable` and a `guidance`, plus — if it installs anything — a `mkIf` in the
# config section. Hosts can define their own the same way, since `workflows`
# is a plain `attrsOf submodule`: anything a host adds is rendered into
# `workflowGuidance` alongside ours, in the order the attribute names sort.
{
  config,
  lib,
  ...
}:

let
  cfg = config.services.herdr-drip;

  # The enabled workflows' prose, in attribute-name order (`attrValues` sorts,
  # so the rendered file is stable across rebuilds rather than reordering
  # itself when an unrelated workflow is added).
  #
  # Blank-line-separated rather than concatenated: each `guidance` is written
  # as a markdown section that starts with its own heading, and two headings
  # with no blank line between them is one heading and a stray line of text in
  # every renderer that matters.
  enabledGuidance = map (w: w.guidance) (
    lib.filter (w: w.enable && w.guidance != "") (lib.attrValues cfg.workflows)
  );

  workflowType = lib.types.submodule (
    { name, ... }:
    {
      options = {
        enable = lib.mkEnableOption "the ${name} workflow — its CLAUDE.md guidance and whatever it installs, together";

        guidance = lib.mkOption {
          type = lib.types.lines;
          default = "";
          description = ''
            The markdown section this workflow contributes to the host's
            CLAUDE.md, rendered into `workflowGuidance` when `enable` is
            true. Should open with its own `##` heading.

            A workflow that only installs things leaves this empty; a
            workflow that only teaches something (`agentCleanup`) is
            nothing but this.
          '';
        };
      };
    }
  );
in
{
  options.services.herdr-drip = {
    workflows = lib.mkOption {
      type = lib.types.attrsOf workflowType;
      default = { };
      description = ''
        Ways of working, each gating its own guidance and its own installs.

        The drip defines `beads`, `agentDispatch` and `agentCleanup` below;
        a host may add its own, and may override any of theirs — setting
        `guidance` replaces our wording without forking the module.
      '';
    };

    workflowGuidance = lib.mkOption {
      type = lib.types.lines;
      readOnly = true;
      description = ''
        The enabled workflows' guidance, concatenated, for a host to feed
        into whatever renders its CLAUDE.md. Read-only: it is computed from
        `workflows`, and the way to change it is to enable, disable or
        reword one of those.
      '';
    };
  };

  config = {
    services.herdr-drip.workflowGuidance = lib.concatStringsSep "\n" enabledGuidance;

    # ── the drip's own workflows ──────────────────────────────────────────
    #
    # All three default OFF, like every other `mkEnableOption` in this repo: a
    # flake input should not start teaching a host's agents new habits or
    # installing a tracker because someone bumped a pin. A host opts in.
    services.herdr-drip.workflows = {

      # beads — the tracker rail and board. The one workflow here that
      # installs as well as teaches; see the plugin gating below.
      beads.guidance = lib.mkDefault ''
        ## beads

        - This host runs the drip's beads plugin: a rail in the sidebar
          showing the in-progress beads for the workspace's repo, and a
          board (two keys, same keys again to close) over the whole pane.
          Both read the repo's `.beads/` through `bd`, which is scoped to
          the plugin — it is NOT on your PATH, so `bd` at a shell prompt
          is expected to fail even where the rail is full.
        - Treat the rail as the answer to "what is being worked on here",
          not as a thing to keep tidy. Do not open, close or re-prioritise
          beads unless asked; another agent very likely owns the one you
          are looking at.
      '';

      # agentDispatch — how to hand work to another pane, and how it comes
      # back. Guidance only.
      agentDispatch.guidance = lib.mkDefault ''
        ## Dispatching work to another pane

        Wire the return path as part of the dispatch, never after it. A
        dispatched agent that cannot report back is one you will find idle
        an hour later holding a question.

        - **Give it your address.** Your own pane id is in `$HERDR_PANE_ID`
          — read it, do not infer it from `herdr agent list`, where two
          workspaces can both have a tab labelled `implementation` and
          terminal titles lag reality by a step.
        - **Put the protocol in the brief**, early rather than as a closing
          line: on finish, on stopping early, or on hitting a decision that
          is the user's to make, the agent calls

              herdr agent prompt "$HERDR_PANE_ID" '<name> (<pane>): <report>'

          A report says what landed, the commit sha if it committed, what
          it verified and how, and what it could not do.
        - **Keep a watcher as the backstop.** A recurring check that reads
          `herdr agent list` and, for any dispatched agent sitting `idle`
          without having reported, reads its tail with `herdr agent read`
          and either answers from the brief or surfaces the question. The
          watcher observes and nudges — say so in its prompt, or it starts
          implementing. Drop it once the work reports in.

        The two failure modes are different and neither covers the other: a
        report-back can never fire, and a watcher alone means learning
        things late and by polling.
      '';

      # agentCleanup — what to do with a pane once its work is done.
      # Guidance only.
      agentCleanup.guidance = lib.mkDefault ''
        ## Cleaning up a finished agent

        When an agent session is 100% done — work landed and reported, and
        nothing left that would need its context — clean the pane up rather
        than leaving it idle. An idle agent is indistinguishable at a glance
        from one that is stuck, and a wall of them is how a real question
        goes unanswered.

        1. **Exit the agent**, so the pane drops back to a shell. `yolo-shell`
           lands you in a normal shell rather than closing the pane, so this
           leaves a live pane at a bare prompt.
        2. **Then look at the tab.** `herdr tab list` reports `pane_count`:

           - more than one pane → `herdr pane close <pane-id>`. The tab has
             other work in it and does not need the empty one.
           - the last pane → **leave it**, as the bare shell from step 1.
             The tab and its label survive, and there is somewhere to type.

        So a tab is never closed by cleanup and never accumulates dead
        agents: it converges on exactly one empty shell.

        Do not clean up a pane you did not dispatch, and do not clean one up
        on the strength of an `idle` status alone — `idle` is also what
        "waiting for an answer" looks like. Done means it reported done.
      '';
    };

    # ── the install half ──────────────────────────────────────────────────
    #
    # Only beads has one. Gating by NAME rather than by rewriting
    # `plugins.plugins` is what lets a host state its plugin list explicitly
    # and still have this toggle bite: subtraction commutes with whatever the
    # host set, where a `mkForce`d list would silently discard it.
    #
    # Guarded on `plugins.enable` so that a host using this module for its
    # guidance alone does not get an opinion about a plugins module it never
    # turned on.
    services.herdr-drip.plugins = lib.mkIf config.services.herdr-drip.plugins.enable (
      lib.mkIf (!cfg.workflows.beads.enable) {
        disabledPlugins = [ "beads" ];

        # And the `bd` with it. Without this the package stays in the closure
        # and on the plugin's runtime inputs for a plugin that is no longer
        # linked — harmless, but it makes `nix why-depends` lie about why a
        # tracker is on a host that disabled the tracker.
        beadsPackage = lib.mkForce null;
      }
    );
  };
}
