---
date: 2026-11-18
status: accepted
---

# Build the inspector as a Tauri app that links `skald-core`

The host needs a visual view of the knowledge base, and the view and the CLI must agree on records,
rendering, and write rules. The repository becomes a Cargo workspace with a `skald-core` library,
the `skald` CLI, and a Tauri inspector app that links `skald-core` in process. Reading, writing,
and rendering then exist once, and the app becomes the natural host process to own a SQLite
database later.

## Consequences

- The inspector watches the filesystem backend and updates live. It shows workstreams with
  status, slot, and branch, and it renders every record.
- The inspector writes only new records: abandonments, annotations, and answers to review
  decisions. It never creates a version of a record that an agent wrote, so it never collides with
  an agent.
- resolve-review waits for answers with a bounded wait command, such as `skald kb wait`, run as a
  background task. Answers given in chat are also recorded as answer records.
- An annotation attaches to an element by its id. Every skill that writes a new version of a record
  MUST first read the record's open annotations, then record which ones it addressed.
- The inspector replaces `skald design open` and `skald serve`, which cannot reach the host's
  browser from inside sbx. `skald design render` stays.
- The app and the CLI can run different versions. A rule for kinds that the app does not know is
  still open.

## Considered Options

- **The app runs `skald --json` for each view.** Rejected because every view would launch a
  process, and live updates would need polling.
- **A headless skald daemon, with the app as one client.** Rejected for now because the filesystem
  backend needs no daemon. It returns as an option when the database backend arrives.
