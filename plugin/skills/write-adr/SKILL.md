---
name: write-adr
description: Generate an architecture decision record. Use this only to codify decisions that are hard to reverse.
---

Write a lightweight architecture decision record (ADR). An ADR records what was decided and why, so
the reasoning survives after everyone who held the original context is gone. This skill covers
writing a new ADR and maintaining an index of ADRs.

This skill gives no guidance on reading ADRs after they are written. Write each ADR so it can be
read on its own, because agents can consume an ADR without instruction.

By default, synthesize the ADR from the current conversation and from exploration of the codebase.
Interview the user to fill any gap in context or reasoning.

## Process

1. **Explore.** Read the conversation, the relevant code, and recent commits. Understand the
   decision and the alternatives to it. Locate the ADR directory (see "Save Location"). Read the
   index, then read any existing ADR that bears on the decision.
2. **Check the warrant.** Proceed ONLY if the decision merits an ADR (see "Warrant").
3. **Draft.** Write the ADR from the template (see "ADR Template"). Add an optional section ONLY
   when it adds value. Resolve the location and the next number, write the file, then update the
   index (see "Output").
4. **Maintain.** If this decision changes an earlier decision, supersede or deprecate the earlier
   ADR (see "Maintenance").
5. **Review.** Scan the result and fix problems inline (see "Self-Review").
6. **Report.** Report the ADR file path and title to the user.

## Warrant

Write an ADR ONLY WHEN both conditions hold:

1. **The decision is hard to reverse.**
2. **The decision binds code beyond the component that makes it.** Other modules, services, or teams
   must conform to the decision, so its trade-offs reach past the code that chose it.

Two kinds of decision usually meet both conditions:

- **Architectural commitments** — a module structure, a service interface, or a pattern that
  addresses a core concern of the domain.
- **Infrastructure commitments** — which datastore to use, or which protocol services use to
  communicate.

When the decision meets both conditions, proceed.

When the decision misses either condition, tell the user that an ADR is likely unwarranted. Write
the ADR ONLY if the user confirms. This gate keeps the ADRs signal-rich.

## ADR Template

Each ADR is a single Markdown file. The frontmatter MUST carry two fields: `date` in ISO 8601
format, and `status`. Frontmatter follows typical Markdown conventions.

After the frontmatter, write a heading that states the decision. Below the heading, write one to
three sentences that give the context, the decision, and the reason.

```md
---
date: 2026-06-22
status: accepted
---

# {Short statement of the decision}

{One to three sentences: the context, what was decided, and why.}
```

**Status** MUST be one of:

- `proposed` — under consideration, awaiting adoption.
- `accepted` — adopted and in effect.
- `deprecated` — retired, with no replacement.
- `superseded by ADR-NNNN` — replaced by a later decision.

**Optional sections.** Every other section is OPTIONAL. Include one ONLY when it adds value:

- **Considered Options** — when the rejected alternatives are worth remembering.
- **Consequences** — when a downstream effect is worth a callout.

Do NOT pad an ADR with empty sections.

## Output

### Save Location

Resolve the ADR directory as follows:

1. **ADR Directory.** If the repository has a directory conventionally used for ADRs (`adrs/`,
   `docs/adr/`, `doc/adr/`, `docs/adrs/`, or similar), use it.
2. **Fallback.** Otherwise, create `adrs/` at the repository root, or in the current working
   directory if not in a repository.

Do NOT use `mktemp` or any system temp directory.

### Filename

Name each ADR `NNNN-slug.md`. `NNNN` is a zero-padded four-digit sequence number. `slug` is a short
kebab-case statement of the decision, as in `0007-event-sourced-writes.md`. Find the highest
existing ADR number in the directory and add one. The first ADR is `0001`. The date belongs in the
ADR frontmatter and the index, NOT the filename.

### Index

The directory MUST hold an index named `README.md`. The index describes itself, so an agent who
lands on it understands the log without this skill. Create the index alongside the first ADR, and
update it on every write.

The index holds a short preamble and then a table. The table holds one row per ADR, sorted by ID:

```md
# Architecture Decision Records

This directory records architecture decisions as numbered ADRs. Each file `NNNN-slug.md` captures
one decision: its context, what was decided, and why. Consult this index before making a decision
that may already be settled, and open an ADR for its full reasoning.

Statuses: `proposed`, `accepted`, `deprecated`, `superseded`.

| ID   | Title                       | Status                | Date       |
|------|-----------------------------|-----------------------|------------|
| [0001](0001-event-sourced-writes.md) | Use event-sourced writes for the ledger | accepted | 2026-06-22 |
```

The **Title** cell links to the ADR file. The **Status** cell mirrors the ADR frontmatter. For a
superseded ADR, write `superseded by ADR-NNNN`.

## Maintenance

When a new decision changes an earlier one, keep the history. NEVER delete an ADR file.

- **Supersede.** Set the earlier ADR's status to `superseded by ADR-NNNN`, where `NNNN` is the new
  ADR's number. In the new ADR, write `Supersedes ADR-MMMM`. Update both rows in the index.
- **Deprecate.** When a decision stops applying and has no replacement, set its status to
  `deprecated`. Update its row in the index.
- **Repair the index.** When the user asks you to fix the index, rebuild the table from the ADR
  files on disk. Read the title, status, and date from each `NNNN-slug.md`. Correct the ordering,
  the missing rows, and the stale statuses.

## Self-Review

Before the final report, scan the result and fix problems inline. Confirm each of the following:

- The decision meets the warrant, or the user confirmed an exception.
- The frontmatter carries a valid `date` and a valid `status`.
- The sequence number is the highest existing ADR number plus one.
- Each ADR number in the directory is unique.
- The title states the decision, rather than naming a vague topic.
- The body gives the context, the decision, and the reason in one to three sentences.
- The index has a row for the new ADR, and its link resolves.
- The index and the ADR files agree on every superseded and deprecated status.
- The ADR is free of placeholders and TODOs.
