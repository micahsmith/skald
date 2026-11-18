---
date: 2026-11-18
status: accepted
---

# Lease worktrees from a platform-tagged pool through Claude Code's worktree hooks

A fresh worktree lacks dependencies and build caches, and in sbx the macOS host and the Linux
sandbox cannot share native build output. skald leases slots from a platform-tagged pool in
`.skald/`, runs setup where the agent runs, and owns Claude Code's `WorktreeCreate` and
`WorktreeRemove` hooks. Agents start warm, every Claude Code worktree comes from the pool, and
repository code never runs on the host for a sandboxed agent.

## Consequences

- A lease reuses a slot only when the slot is clean, idle, and of the same platform, and its branch
  has merged. Otherwise skald creates a slot, up to a limit.
- A workstream records the slot it has on loan.
- `claude --worktree <name>` leases a slot for the workstream with that name.
- Subagent worktrees lease slots with no workstream. `WorktreeRemove` does not fire when a
  subagent finishes, so skald reclaims a slot only when the slot is clean and no process runs in
  it.
- Claude Code ignores `.worktreeinclude` once a create hook is set, so skald copies the files that
  `.worktreeinclude` lists.
- Setup steps live in each machine's home config, keyed by repository identity. skald MUST NOT read
  setup steps from the repository or from `.skald/`, because a sandboxed agent can write the
  mounted tree, and a lease on the host would run what it wrote.
- The inspector shows slots and leases. It does not provision them.
- A bug in skald can break `claude --worktree` and subagent isolation.
- A worktree made with plain `git worktree add`, such as the one that deep-review's prep script
  makes, bypasses the hooks and stays outside the pool.
- It is unverified whether sbx keeps the sandbox's home directory, and with it the setup config,
  when a sandbox is recreated.

## Considered Options

- **A fresh worktree for each workstream.** Rejected because every workstream would pay the full
  cold start.
- **A separate skald command beside Claude Code's own worktrees.** Rejected because two worktree
  systems would exist, and subagents would start cold.
- **Setup steps in a committed repository file.** Rejected because a hostile repository would run
  code the moment a worktree is created.
- **Setup steps in `.skald/`.** Rejected because a sandboxed agent could plant commands that a
  lease on the host would run.
