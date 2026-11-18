---
date: 2026-11-18
status: accepted
---

# Generate the plugin from skill sources and shared fragments

Each skill folder must stand alone, so shared text is copied into every skill that needs it and
kept in step by hand. skald serves what it owns at runtime, and a generator builds the committed
`plugin/` from skill sources and shared fragments that are each written once. Every shared line
then has one source, and a CLI change still lands in the same commit as the skill text that
describes it.

## Consequences

- This reverses the earlier rule against a build step.
- What skald owns includes schemas, formats, and knowledge-base procedure (ADR-0004, ADR-0005).
- `plugin/` is generated output, so edits MUST go to the sources. CI MUST fail when `plugin/`
  differs from a fresh build.
- The marketplace keeps pointing at `plugin/`, so installation does not change.
- The only target is the Claude Code plugin. Sources MUST keep agent-specific parts, such as
  frontmatter fields and hook config, apart from the skill body, so that other agent targets can
  be added later.
- The output MAY later move to a separate distribution repository without a change to the sources.

## Considered Options

- **Runtime serving only, with the existing shared-block test.** Rejected because shared prose
  would still be copied by hand.
- **A second repository for the output from the start.** Rejected for now because it adds a
  publishing pipeline and a second repository to keep in step.
