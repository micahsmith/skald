# skald

Skald records, versions, and renders design intent for a git repository. This repository also
holds `code-skillset`, a plugin of skills and hooks for Claude Code and other agents.

## Installation

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

The plugin also installs without skald. In Claude Code, add this repository as the `skald`
marketplace and install the plugin from it:

```text
/plugin marketplace add micahsmith/skald
/plugin install code-skillset@skald
```

Other agents MAY install each directory under `plugin/skills/` as a skill.

## Usage

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
| `tests/` | Integration tests that run the built binary. |
| `plugin/` | The `code-skillset` plugin. |
| `plugin/skills/` | One directory per skill. |
| `plugin/hooks/` | The session hooks and the guides they print. |
| `.claude-plugin/marketplace.json` | The `skald` marketplace. Its one entry points at `./plugin`. |
| `.github/workflows/ci.yml` | CI: the full test suite and a plugin smoke test. |
| `.github/workflows/release.yml` | Release: on a `v*` tag, builds the binary on Linux and macOS and attaches each to the release. |

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
