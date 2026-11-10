# skald

This repository holds two things:

- `code-skillset`, a plugin of skills and hooks for Claude Code and other agents.
- `skald`, a command-line program that records, versions, and renders design intent for a git
  repository.

The plugin works without skald.

## Installation

Give the prompt below to your agent to install the plugin. Use the same prompt to install, update,
or reinstall. Each run replaces any earlier copy, so every run ends in the same state.

```text
Install the `code-skillset` plugin from https://github.com/micahsmith/skald. Follow these steps
in order:

1. Fetch the latest commit of the default branch over HTTPS. Assume you do not have access to SSH.
2. Remove every earlier installation of `code-skillset`, including cached copies and any `skald` or `cc-loadout`
   marketplace entry. Leave everything else in place.
3. Install at the user level, using the native install mechanism of the agent you are running in:
   - If the agent supports Claude Code plugin marketplaces, add the repository as the `skald`
     marketplace by its HTTPS URL. Then install `code-skillset@skald`. This step also installs
     the hooks.
   - Otherwise, install each directory under `plugin/skills/` as a skill. Keep each directory name
     and its contents unchanged. Then register the hooks in `plugin/hooks/` with the agent's
     equivalent of a session-start hook. If the agent has no equivalent, skip the hooks. Ask for
     confirmation before editing any global instruction file.
4. Confirm that every skill under `plugin/skills/` is installed.
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

## skald

### Installation

Installation takes two steps. First, install the `skald` binary:

```sh
cargo install --locked --git https://github.com/micahsmith/skald skald
```

Then run setup:

```sh
skald setup
```

Setup installs the Claude Code session hook and the `code-skillset` plugin. Running setup again
changes nothing. Pass `--no-plugin` to skip the plugin.

### Usage

`skald --help` lists every command. `skald <command> --help` lists the flags of one command.

Output goes to stdout as TOON. The `--json` flag switches it to JSON. Errors are structured and
also go to stdout. stderr carries diagnostics only.

| Exit code | Meaning |
| --- | --- |
| `0` | Success, including no-ops. |
| `1` | Error. |
| `2` | Usage error: unknown flag, unknown command, or missing argument. |

These environment variables change where skald reads and writes files:

| Variable | Default | Purpose |
| --- | --- | --- |
| `SKALD_HOME` | `~/.skald` | The data directory. It holds the store file `skald.db` and the adapter markers. |
| `CLAUDE_CONFIG_DIR` | `~/.claude` | The Claude Code configuration directory. Setup writes the session hook to its `settings.json`. |
| `CLAUDE_CODE_PLUGIN_CACHE_DIR` | `plugins` in the Claude Code configuration directory | The directory that holds `installed_plugins.json`, which shows whether the plugin is installed. |

## Layout

| Path | Contents |
| --- | --- |
| `Cargo.toml`, `src/` | The `skald` crate and its binary. |
| `tests/` | Integration tests that run the built binary, and checks of repository files. |
| `plugin/` | The `code-skillset` plugin. |
| `plugin/skills/` | One directory per skill. |
| `plugin/hooks/` | The session hooks and the guides they print. |
| `.claude-plugin/marketplace.json` | The `skald` marketplace. Its one entry points at `./plugin`. |
| `scripts/ci.sh` | Local CI: format, lint, tests, and a plugin smoke test. |
