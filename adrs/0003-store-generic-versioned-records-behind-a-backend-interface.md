---
date: 2026-11-18
status: accepted
---

# Store generic versioned records behind one backend interface

The knowledge base needs several storage backends over time: a filesystem first, then a SQLite
database owned by the host, then a remote database. A backend stores only workstreams and generic
versioned records, and every rule specific to a kind lives once in `skald-core`, above the
interface. A new backend then implements a small interface, and a new kind needs no backend change.

## Consequences

- A record belongs to a workstream or to the repository. It has a kind, a name, metadata,
  versions, and content in a stated format.
- One filesystem backend serves every kind. It stores records as `.toon` files and freeform designs
  as `.html` files, and it stores no markdown (ADR-0004).
- Every write names the version it was based on. A write based on a stale version fails with a
  structured error, and the writer re-reads and retries. This rule covers agents in different
  slots, agents on both sides of the sandbox mount, and the inspector.
- Design validation, decision immutability, merge stamping, and the folding of answers into
  reviews live in `skald-core`. Backends never implement them.
- A query specific to a kind, such as "every entity named User", needs a scan or an index that
  `skald-core` maintains.
- A conformance test suite defines the interface. Every backend MUST pass it.
- How each environment picks its backend is open. In sbx, the host would use SQLite while the
  sandbox uses the remote client, so one config file in the mounted tree cannot hold both.

## Considered Options

- **Typed operations in each backend,** such as `submit_design`. Rejected because every new kind
  would touch every backend.
- **Generic records plus optional fast paths per backend.** Rejected because `skald-core` would
  need two code paths for each fast path.
- **Separate markdown and TOON filesystem backends.** Rejected because both would store the same
  data in different shapes.
