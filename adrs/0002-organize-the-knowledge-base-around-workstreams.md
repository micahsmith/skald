---
date: 2026-11-18
status: accepted
---

# Organize each repository's knowledge base around workstreams

The documents, designs, and decisions of one piece of work need one record that ties them together
for the inspector and the worktree pool. Each repository has one knowledge base of workstreams,
and each workstream has at most one branch and at most one worktree slot. With one branch per
workstream, git alone decides a workstream's status and when its decisions ship, so no skill
maintains state.

## Consequences

- A workstream's status is derived from local git and the workstream's records:

  | Status | Condition |
  | --- | --- |
  | `planning` | The workstream has no branch. |
  | `active` | The branch exists, and no review is recorded against it. |
  | `in-review` | The latest review has unresolved issues. |
  | `reviewed` | Every issue in the latest review is fixed, rejected, or skipped. |
  | `merged` | The branch has merged into the default branch. skald detects the merge when it next runs. |
  | `abandoned` | An abandonment record exists. This is the only status set by hand. |

- A decision's state follows its workstream. It is `open` until the workstream merges, and then
  `merged`, stamped with the merge commit. It is `dropped` when the workstream is abandoned.
- A merged decision is immutable. A reversal is a new decision with a `supersedes` link, and the
  old decision reads as `superseded`. An intention is an open decision.
- Work that needs several branches becomes several workstreams, with no formal link between them.
- Repository-level records, such as conventions and distilled decisions, sit beside the
  workstreams.
- Default reads and the home view show workstreams that are neither merged nor abandoned. Merged
  and abandoned workstreams are reached by id.
- skald reads no pull-request state, so it never needs the network.
- The inspector shows one knowledge base for now. Showing several is a long-term goal.

## Considered Options

- **Records organized by kind, with workstreams as tags.** Rejected because status and worktrees
  would have no record to attach to.
- **A stack of branches per workstream.** Rejected because status would become an aggregate across
  branches.
- **Child workstreams.** Rejected to keep one level of nesting.
- **Status set by each skill.** Rejected because status drifts when someone skips a skill.
- **One knowledge base across repositories.** Rejected because it would live outside every working
  tree, where the host cannot see it in sbx (ADR-0001).
