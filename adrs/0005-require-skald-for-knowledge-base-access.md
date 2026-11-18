---
date: 2026-11-18
status: accepted
---

# Require skald for every knowledge-base read and write

A backend can sit across a network, as a database owned by the host does, so skills cannot reach
the knowledge base with file tools. Skills that use the knowledge base require the skald binary,
and they read and write only through its CLI. Skills then have one code path for every backend,
and the backend interface exists once, in Rust.

## Consequences

- This reverses the earlier rule that the plugin works without skald.
- A skill writes a record to a local file, then submits it with a command such as `skald kb put`.
  Reads use a command such as `skald kb get`.
- `knowledge-base.sh` and its seven copies are deleted. The shared knowledge-base block shrinks to
  the calls into skald.
- Skills that never touch the knowledge base stay standalone, such as cleanup-comments, write-adr,
  and write-skill.
- In sbx, a Linux build of skald must be installed inside the sandbox.
- Agents other than Claude Code need the binary to use the knowledge-base skills.

## Considered Options

- **Two paths in each skill:** markdown files without skald, and the CLI with it. Rejected because
  every knowledge-base skill would branch.
- **One protocol served by skald or by a bundled script.** Rejected because two implementations of
  the protocol would have to agree.
