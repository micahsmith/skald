---
name: handoff
description: Compact the conversational context into a persisted handoff file.
argument-hint: "What will you use the handoff for?"
disable-model-invocation: true
---

Create a file that condenses the current context into a standalone file. An agent with an empty
context MUST be able to read the handoff and continue the work without additional input.

If an argument is provided, treat it as a description of the handoff's intended use and tailor the
content toward that goal.

<!-- shared: knowledge-base -->

## Knowledge Base

Read `knowledge_base` from the session context. If it is missing, or its `dir` is outside the
repository you are working in, run `bash scripts/knowledge-base.sh` to print it. If it reports an
`error`, show the error to the user and stop. Before the first save, run
`bash scripts/knowledge-base.sh init` if `exists` is false. After `init` in a git repository, if
`ignored` is false, git tracks the folder, so tell the user before you save there. After you create
or edit any file under `work/`, run `bash scripts/knowledge-base.sh index`.

<!-- end shared -->

## Save Location

Save the handoff to `handoffs/handoff-<summary>-<date>.md`, relative to the `dir` of
`knowledge_base`. Create `handoffs/` if it is missing.

- `<summary>` is at most three words in kebab-case that describe the content.
- `<date>` is the output of `date +%F`.

## What to Write

Prefer references over inlined content:

- Cite a whole file by its path.
- Cite a specific part of a file as `path:line`.
- Cite commits by `git` SHA.
- Cite external sources by URL.

Structure the handoff with these sections (omit any that would be empty):

- **Goal.** A 1-3 sentence summary of purpose. Base it on the current conversation and on the
  argument, if the user provided one.
- **Current State.** What has been done and what is unfinished.
- **Key Locations.** Files, paths, URLs, and symbols the next agent needs. Provide a one-line note
  explaining why the location is relevant to the conversation.
- **Decisions & Constraints.** Choices already made and why they were made. Stating them clarifies
  intent and stops the next agent from relitigating settled decisions.
- **Plans.** The concrete steps identified in context that haven't yet been finished (if any).
  Provide these in order if possible.
- **Gotchas.** Non-obvious traps, failed approaches, and things that look wrong but aren't.

Report the handoff's path back to the user.
