---
date: 2026-11-18
status: accepted
---

# Make structured records the canonical form of every kind

Markdown documents cannot give stable ids to requirements, plan items, or review issues, which the
inspector needs for status and annotations. Every kind has a schema from the start, skills submit
records in TOON, and the knowledge base stores no markdown. Every backend then holds the same data,
and every element that a person or agent refers to has a stable id.

## Consequences

- The first kinds are workstream, spec, plan, review with issues and triage status, handoff,
  design (data model, interface, or freeform HTML), and decision.
- The inspector adds four kinds: annotation, answer, abandonment, and the record that a skill
  writes when it addresses an annotation (ADR-0008).
- People read records in the inspector, or through a command that renders one record as markdown
  on request, such as `skald kb render`. skald stores no rendering, so no derived copy can go
  stale.
- Changes go through skald or arrive as annotations.
- No `md` content format exists. Markdown documents written before this decision are not imported.
- skald serves each schema through a command, so skills do not embed formats (ADR-0006).
- ADRs stay as committed files outside the knowledge base.
- The first version needs a schema for every kind before it ships.

## Considered Options

- **Markdown templates that skald parses.** Rejected because parsing markdown is fragile, and every
  template change would need a parser change.
- **Opaque markdown until a kind earns a schema.** Rejected in favor of one form for every kind
  from the start.
- **Stored `.md` renderings beside each record.** Rejected because each record would have two
  files, and an edit to the rendering would be lost on the next write.
