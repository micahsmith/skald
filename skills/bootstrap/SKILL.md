---
name: bootstrap
description: Apply preferred global settings to the agent. Installs a compact status line and forces text-based question and answer interactions.
disable-model-invocation: true
---

Bootstrap the Claude Code environment by changing the global settings (by default,
`$CLAUDE_CONFIG_DIR/settings.json`, or `$HOME/.claude/settings.json` if `CLAUDE_CONFIG_DIR` is not
set).

## What It Does

1. Creates a custom status line. For example, the status line renders as follows on branch
   `feat/new-widget`, with Opus 4.8 selected and 6% of context used:

   ```text
   Opus 4.8 (1M) | 6% (64K) | feat/new-widget
   ```

2. Blocks the use of `AskUserQuestion`, forcing text-only interactions.

## Steps

1. Provide a summary of the changes that will be made and confirm that the user wants to proceed.

2. Run the installer (merges the changes into existing settings with an initial backup of the
   current settings):

   ```sh
   bash scripts/install.sh
   ```

3. Report success or failure. If installation failed, explain the cause. If the user must restart to
   see the changes, recommend it.

## Notes

- The scripts require `jq`. If `jq` is missing, the scripts emit an error.
- The old settings are backed up to `settings.json.bak`.
- Existing `settings.json` configuration will be preserved.
- Execution is idempotent.
