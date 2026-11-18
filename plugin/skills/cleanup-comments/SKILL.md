---
name: cleanup-comments
description: Delete unnecessary code comments and tighten comments that remain.
disable-model-invocation: true
---

Examine the code comments in scope. Only retain the comments that state what the code cannot and
delete the rest.

The rules here MUST be treated as the definitive style guide for code comments. If they conflict
with other guidance, these rules govern.

## Scope

Presume that the scope of work to clean up is all uncommitted changes in the working tree. In other
words, the default scope is `git diff HEAD` and any untracked files. If the user specifies a
different scope, use the stated scope. For example, the user may request to examine all comments
compared to the base branch. Branches, directories, single files, or the whole repository might be
requested.

Do NOT edit comments outside of the provided scope. Report them to the user and ONLY edit if the
user first confirms the edit is allowed.

## Scope Lock

Edit comments only. All executable code MUST be byte-identical upon completion. Do NOT rename,
extract, change logic, or reformat. You MAY remove whitespace orphaned by a deleted comment.

Comments that exist to clarify or warn about poorly written code are doing important work and MUST
NOT be changed. Report these comments to the user. Do NOT fix the code until the user first confirms
the edit is allowed.

## Code Comments

### Phase 1: Delete or Keep

Run every comment in scope through these steps in order. Stop at the first step that applies. For
comments that document classes or functions (doc comments), use the "Doc Comments" section below
instead.

1. **Restatement.** Read the comment as though it started with "Because...", "Requires...", or
   "Careful:...". If the restated comment is not grammatical or coherent, the comment restates the
   code. **Delete it.** For example, these comments merely restate code: `// increment the counter`,
   `// loop over the users`, `// parse the request body`, and `} // end if`.
2. **Change Narration.** Read the comment as someone who knows nothing about the code's history. If
   the comment only records that history, **delete it.** For example, these comments only record
   history: `// now also handles null`, `// updated to the v2 client`, `// as requested`,
   `// removed the old fallback`, and `// to satisfy acceptance criterion AC7`. The commit history
   already holds that content.
3. **Staleness.** Determine whether the comment describes code or behavior that no longer exists. If
   the comment is stale, **delete it** OR correct it so the comment is made accurate. A mistaken or
   false comment is worse than a missing comment.
4. **Protected Comment.** The comment matches a category under the "Protected Comments" section.
   **Keep it.**
5. **Everything Else.** The comment matches no step above. **Keep it.**

### Phase 2: Tighten

All comments that are kept MUST be subjected to the deletion test one sentence at a time: delete the
sentence and name the fact that has been lost with its deletion. If you cannot name a fact that is
now absent from the file, that sentence MUST be removed from the comment. Keep the sentences that
survive the deletion test.

Embedding real information and embedding it efficiently are independent judgments. A comment can
state a genuine constraint and remain far too long or verbose. Phase 2 MUST be applied to all
comments that survive Phase 1. A comment of five or more lines almost always needs tightening.

Do NOT reword comments that are already clear and concise.

See [EXAMPLES.md](EXAMPLES.md).

## Doc Comments

Doc comments document declarations for their callers. Use these steps instead of "Code Comments".

1. **Audience.** Does the declaration belong to a library or utility API such that callers use it
   without reading it? A declaration that is merely public, exported, or capitalized does not
   qualify. If it does not qualify, **delete the doc comment**.
2. **Density.** Do sibling declarations carry doc comments? Check the file and its surrounding
   package or module. If most siblings do not carry doc comments, **delete it**. If there is nothing
   to sample, **delete it**.
3. **Signature.** Apply the deletion test per sentence, as in Phase 2 above. A sentence that merely
   restates what is already present in the signature MUST be deleted.

Doc comments that survive SHOULD state the contract: guarantees, return values, errors raised,
caller obligations, and surprising behavior. The doc comments MUST NOT state implementation details.
A caller needs to know whether a method is thread-safe, for example, but not which executor it uses.

## Protected Comments

These read like restatement but aren't. Keep them.

- **Cross-File Sync Pointers.** For example: `// keep in sync with the router's target groups`. The
  link is the reason: it stops two copies from drifting apart. Deleting it deletes the only warning
  that the next editor gets.
- **Data-Literal Semantics.** For example: `# pence, not pounds`, `# (width, height), portrait`. A
  literal cannot state its own units, order, or convention.
- **Format Contracts.** For example: `// thousands separators, matching the dashboard`. These pin
  output to an expectation held outside the file.
- **Section Banners.** For example: `# --- Parsing ---`. These are navigation, not narration. Treat
  them as house style. Conform to codebase conventions and keep them unless instructed otherwise.
- **TODO and FIXME Markers.** Keep them. Delete one only when the work it names is visibly done.

## When Uncertain

When uncertain about a reason, a constraint, or a warning, keep it. When uncertain about a
restatement, delete it.

Wrongly deleting a warning that does real work costs more than leaving a mediocre comment behind.

## Report

Re-read the diff and confirm that only comments changed. Then report, briefly, with clear section
demarcation:

- Counts: comments deleted, comments tightened, comments kept.
- Comments kept but flagged for a separate refactor, cited as `path:line`.
- Comments outside the scope that need attention, cited as `path:line`.
