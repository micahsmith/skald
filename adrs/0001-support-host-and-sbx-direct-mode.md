---
date: 2026-11-18
status: accepted
---

# Support agents on the host and in sbx direct mode

A coding agent may run on the host or inside an sbx sandbox, and the host must see its work either
way. skald supports both, with sbx in direct mode, and keeps all knowledge-base data and worktree
slots in the main checkout's gitignored `.skald/` folder. In direct mode, the mounted working tree
is the only location that the host and the sandbox both read and write at one absolute path.

## Consequences

- The home-directory store `~/.skald/skald.db` goes away. In sbx it lives in the sandbox's home
  directory, which the host cannot see.
- A database backend MUST be owned by one process on the host. An agent in the sandbox reaches that
  process over HTTP or vsock. The transport is not yet chosen.
- A SQLite file MUST NOT be shared across the sandbox mount. SQLite locking is unreliable on shared
  filesystems, and WAL mode needs shared memory, which a VM boundary does not provide.
- Per-machine config, such as worktree setup steps, lives in each machine's home directory. See
  ADR-0007.
- sbx clone mode is unsupported. In clone mode, the host sees the sandbox's commits only after
  `git fetch`, and never sees gitignored files such as `.skald/`. Two ways can close the gap:
  - a service on the host that the agent reaches through `host.docker.internal`, which has the same
    shape as the host-owned database above;
  - records stored in git refs, which travel with `git fetch`.

## Considered Options

- **One SQLite file in the working tree, opened from both sides.** Rejected because of the locking
  problems above.
- **Support clone mode now.** Rejected because it puts a host service or git-ref sync into the first
  version.
