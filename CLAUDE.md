## Overview

This repository holds two things:

- `skald`, a Rust command-line program. The crate is at the repository root: `Cargo.toml`, the
  program in `src/`, and integration tests in `tests/`.
- `code-skillset`, a plugin of skills and hooks for AI agents. The plugin is in `plugin/`. The
  marketplace manifest at `.claude-plugin/marketplace.json` points at it.

The full test suite is the local CI script. It runs `cargo fmt --check`, `cargo clippy`,
`cargo test`, and a plugin smoke test:

```sh
bash scripts/ci.sh
```

`skald dev seed --designs <N>` is a test-only command. It adds N data-model designs to the store
for the current repository, so that tests can measure the store at scale. It is hidden from
`--help`. It MUST NOT be used outside tests.

## Style

Skills and documentation are prose instructions intended for AI agents. Use the following guidance
for writing:

- Be clear. You MUST avoid excess jargon. Prefer simple and legible prose.
- Be concise. Add words only in service of clarity, and cut any other excess.
- Use RFC directives in skills and documentation. The modality of an instruction MUST be
  expressed using all capital terms like 'MUST', 'MUST NOT', 'SHOULD', 'MAY', etc. Capitalize no
  other word for emphasis.

## Authoring Hooks

Hooks that ship as part of the plugin are located in `plugin/hooks`.

## Authoring Skills

- Each skill is located at `plugin/skills/<name>/SKILL.md`. Skills MAY use progressive disclosure
  in order to reduce complexity.
- Before writing or editing a skill, read `plugin/skills/write-skill/GUIDELINES.md`. Every skill
  MUST follow it.
- A skill that saves to the knowledge base MUST contain the shared `knowledge-base` block in
  `SKILL.md` and a copy of `plugin/hooks/knowledge-base.sh` in its `scripts/` folder.
- Skills MUST NOT use `mktemp` or a system temp directory outside a bundled script.
