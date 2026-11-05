## Overview

This repository is a library of skills for AI agents. There is no build/test capability available.

## Style

Skills and documentation are prose instructions intended for AI agents. Use the following guidance
for writing:

- Be clear. You MUST avoid excess jargon. Prefer simple and legible prose.
- Be concise. Add words only in service of clarity, and cut any other excess.
- Use RFC directives in skills and documentation. The modality of an instruction MUST be
  expressed using all capital terms like 'MUST', 'MUST NOT', 'SHOULD', 'MAY', etc. Capitalize no
  other word for emphasis.

## Authoring Hooks

Hooks that ship as part of the plugin are located in `hooks`.

## Authoring Skills

- Each skill is located at `skills/<name>/SKILL.md`. Skills MAY use progressive disclosure in order
  to reduce complexity.
- Before writing or editing a skill, read `skills/write-skill/GUIDELINES.md`. Every skill MUST
  follow it.
- Skills that write files MUST try to resolve save location using the same convention. These
  skills MUST NOT use `mktemp` or a system temp directory outside a bundled script. Instructions
  on file path resolution MUST be duplicated inline for any skill that writes files. Any change to
  this convention MUST be synced across all skills. The order for resolving save location is as
  follows:
  1. an output-specific directory established by convention (e.g., if the repository has a `specs/`
     directory then specs MUST be written there)
  2. an existing scratch directory (e.g., `tmp/`, `temp/`, `scratch/`, especially if git-ignored)
  3. the repository root or the current working directory if not in a repository
