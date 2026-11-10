# skald

Skills and hooks library for Claude Code and other agents.

## Installation

Give the prompt below to your agent. Use the same prompt to install, update, or reinstall. Each run
replaces any earlier copy, so every run ends in the same state.

```text
Install the `code-skillset` plugin from https://github.com/micahsmith/skald. Follow these steps in
order:

1. Fetch the latest commit of the default branch over HTTPS. Assume you do not have access to SSH.
2. Remove every earlier installation of `code-skillset`, including cached copies and any `skald` or
   `cc-loadout` marketplace entry. Leave everything else in place.
3. Install at the user level, using the native install mechanism of the agent you are running in:
   - If the agent supports Claude Code plugin marketplaces, add the repository as the `skald`
     marketplace by its HTTPS URL. Then install `code-skillset@skald`. This step also installs the
     hooks.
   - Otherwise, install each directory under `skills/` as a skill. Keep each directory name and its
     contents unchanged. Then register the hooks in `hooks/` with the agent's equivalent of a
     session-start hook. If the agent has no equivalent, skip the hooks. Ask for confirmation before
     editing any global instruction file.
4. Confirm that every skill under `skills/` is installed.
5. Report the install location, what was installed, what was skipped, and whether the agent needs to
   be restarted.
```

## Hooks

The hooks add these guides to the context at the start of every session and every subagent.

| Guide | Purpose |
| --- | --- |
| `how-to-write-comments` | Rules for writing code comments and doc comments. |
| `how-to-write-prose` | Rules for technical writing that improve clarity and comprehension. |

## Skills

| Skill | Purpose |
| --- | --- |
| `bootstrap` | Apply preferred global settings. |
| `brainstorm` | Interview the user to reach alignment on design and requirements before work begins. |
| `cleanup-comments` | Delete unnecessary code comments and tighten the comments that remain. |
| `deep-review` | Review a branch in depth and write a consolidated report. |
| `handoff` | Compact the conversation into a handoff file. |
| `merge-review` | Merge findings from another review into a `deep-review` report. |
| `resolve-review` | Validate and fix the issues in a `deep-review` report. |
| `write-adr` | Record an architecture decision as a numbered, indexed ADR. |
| `write-plan` | Write a self-executing implementation plan. |
| `write-skill` | Write a new skill. |
| `write-spec` | Write a spec. |
