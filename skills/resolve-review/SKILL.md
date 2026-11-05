---
name: resolve-review
description: Triage and resolve issues from a `deep-review` report. Validates every issue, delegates simple low-risk fixes, walks through more complex review items, and tracks the status of review items. Use when there is a need to resolve, triage, or fix the issues in a `deep-review` report.
argument-hint: "[report-path]"
disable-model-invocation: true
---

Process a `deep-review` report systematically. You are the coordinator. Subagents read the code and
change the code. You only handle verdicts, decisions, and statuses, to keep your context small.

## Rules

- You MUST NOT read source code. Delegate every code read to a subagent. You MAY read the report,
  `git` status output, and test output.
- You alone edit the report. Subagents MUST NOT edit the report.
- All fixes land in the shared working tree. You and the subagents MUST NOT commit, stage, stash, or
  switch branches. The user handles commits.
- Run at most 4 sub-agents (validators or fixers) at once.
- Accept ONLY a `deep-review` report.

## Vocabulary

Validators assign a verdict, a judgment, and a risk to each issue. [VALIDATOR.md](VALIDATOR.md)
holds the full criteria.

| Field | Value | Meaning |
| --- | --- | --- |
| Verdict | `confirmed` | The issue is real and the proposed solution is sound. |
| Verdict | `confirmed-alt` | The issue is real, but another fix is better than the proposed solution. |
| Verdict | `rejected` | The validator disproved the issue. |
| Verdict | `unclear` | The validator could neither confirm nor disprove the issue. |
| Judgment | `mechanical` | One fix is plainly correct. |
| Judgment | `choice` | Several fixes are valid, and the user needs to choose one. |
| Judgment | `design` | The fix depends on a design decision that needs user input. |
| Risk | `low`, `medium`, `high` | The chance that the fix breaks behavior elsewhere. |

Each issue has one status at a time:

| Status | Meaning |
| --- | --- |
| `open` | No validator has assessed the issue. |
| `rejected` | The verdict is `rejected`. No fix is planned. |
| `unclear` | The verdict is `unclear`. No fix is planned. |
| `queued` | The issue is ready for a fixer. |
| `awaiting-decision` | The issue needs an answer from the user. |
| `in-progress` | A fixer is working on the issue. |
| `fixed` | The fix is applied. |
| `blocked` | The fixer could not finish. |
| `skipped` | The user chose to leave the issue unfixed. |

## Steps

### 1. Load the Report

The argument is the path to the report. If the argument is missing, look for the newest
`review-*.md` file. Search in this order:

1. A directory conventionally used for reviews (`reviews/` or similar).
2. An existing scratch directory at the repository root (`tmp/`, `temp/`, `scratch/`, or similar).
3. The repository root, or the current working directory if not in a repository.

Unless the argument exactly matches the file found, confirm the file you found with the user before
you continue.

List the issues without reading the whole report:

```sh
grep -n -E '^### \[|^- \*\*(Severity|Location):\*\*' <report-path>
```

A merged issue carries several IDs in its heading. Treat the merged issue as one issue, and keep
every ID.

Check the working tree before any work starts:

- The report is named `review-<target>-<date>.md`. If the current branch differs from `<target>`,
  ask the user whether to continue.
- If `git status --porcelain` shows uncommitted changes, list them to the user. State that the fixes
  will mix with those changes. Then continue.

### 2. Build or Resume the Triage Table

If the report has no triage table, add one (see "Tracking"). Give every issue the status `open`.

If the report has a triage table, resume from it. Handle each row by its status:

| Status | Action |
| --- | --- |
| `fixed`, `rejected`, `skipped` | Skip the row. |
| `open` | Validate the issue in step 3. |
| `queued` | Fix the issue in step 5. |
| `awaiting-decision`, `unclear`, `blocked` | Include the issue in the decision batch in step 6. |
| `in-progress` | The row is stale, because no fixer survives a session. See below. |

For each stale `in-progress` row, run `git status --porcelain -- <files>` on the files in the row.
Then set the status to `queued`. If any file has changes, add "partial fix may exist" to the note.

### 3. Validate

First, find out how tests run. If the project guidance in your context already states the test
commands, use them. Otherwise, spawn one subagent to report. This subagent SHOULD run on a small
model (such as Haiku) when the harness can choose one. It reports:

- The command that runs the full test suite.
- The command form that runs a single test file or test case.
- Whether several test runs can happen at once in one working tree. Shared databases, fixed ports,
  and shared build directories prevent this.

Then spawn one validator per `open` issue. Give each validator:

- The absolute path to [VALIDATOR.md](VALIDATOR.md), which the validator MUST read first.
- The report path and the ID of its issue.
- The issue list from step 1, so that the validator can name issues that share a root cause.
- The test commands. If test runs cannot happen at once, tell the validator to run no tests.

Record each verdict, judgment, risk, and footprint in the triage table as the validator returns. The
footprint is the list of files the fix touches. Then add a validation block to the issue (see
"Tracking"). The validation block is the only place in the report that holds the validator's
evidence.

A validator MAY write a temporary test when reading the code cannot settle an issue. Every temporary
test has `rrtmp` in its file name, and the validator deletes it before returning. A validator that
stops early can leave the file behind. So end this step by listing the leftover temporary tests,
even when no issue was `open`. Delete each file listed:

```sh
git ls-files --others --exclude-standard | grep rrtmp
```

### 4. Apply the Dispatch Policy

Set the status of each validated issue from this table:

| Verdict | Judgment | Risk | Action | Status |
| --- | --- | --- | --- | --- |
| `confirmed`, `confirmed-alt` | `mechanical` | `low`, `medium` | Fix without asking. | `queued` |
| `confirmed`, `confirmed-alt` | `mechanical` | `high` | The user approves in bulk. | `awaiting-decision` |
| `confirmed`, `confirmed-alt` | `choice` | any | The user picks an option. | `awaiting-decision` |
| `confirmed`, `confirmed-alt` | `design` | any | Discuss with the user, last. | `awaiting-decision` |
| `rejected` | — | — | Surface the issue. Do NOT fix it. | `rejected` |
| `unclear` | — | — | Surface the issue. Do NOT fix it. | `unclear` |

For a `confirmed-alt` issue, the fixer applies the validator's fix instead of the proposed solution.
Record the validator's fix in the note.

The user MAY override a `rejected` or `unclear` verdict. An overridden issue becomes `queued`.

### 5. Fix in Waves

Start the first wave as soon as validation ends. Do NOT wait for the user.

Plan each wave as follows:

1. **Group.** Issues that share a root cause form one unit of work, and one fixer takes the whole
   unit. Every other issue is its own unit. The footprint of a unit is the union of its footprints.
2. **Separate.** Two units conflict when their footprints share a file. Units that conflict MUST run
   in different waves.
3. **Fill.** Put at most 4 units that do not conflict into the wave. The other units wait.

Set each issue to `in-progress` before its fixer starts. Each fixer SHOULD run on a mid-size model
(such as Sonnet) when the harness can choose one. Give each fixer:

- The absolute path to [FIXER.md](FIXER.md), which the fixer MUST read first.
- The report path and the IDs in its unit.
- The fix to apply to each issue: the proposed solution, the validator's fix, or the option the user
  chose.
- The footprint of the unit, plus the "partial fix may exist" note if the row has one.
- The test commands. If test runs cannot happen at once, tell the fixer to run no tests.

Update the triage table as each fixer returns, and add a resolution block to each issue that becomes
`fixed` or `blocked` (see "Tracking"). Handle a `blocked` return by its cause:

- **The fix needs a file outside the footprint.** Add the file to the footprint. Set the issue to
  `queued` for a later wave. The next fixer is a fresh attempt, not a retry, so it runs on the
  mid-size model.
- **Any other cause.** Keep the status `in-progress`, and retry the unit once on the session default
  model. Include the first fixer's `NOTE` in the retry prompt. Write the resolution block only after
  the retry returns. If the retry also returns `blocked`, set the status to `blocked`, and include
  the issue in the decision batch.

Run waves until no `queued` issue remains.

### 6. Ask the User in One Batch

Present every open decision in one message while the first wave runs. If your subagents block the
conversation, present the batch right after the first wave returns.

The batch has three parts. Omit any empty part.

1. **Approvals.** Every `mechanical` issue with `high` risk. State the fix and the risk for each.
   The user MAY approve them all with one reply.
2. **Choices.** Every `choice` issue. Present each choice in this shape, so that the user can see
   that the options exclude each other:

   ```markdown
   **LOG-2** Cache never expires. Pick one:
   - **A.** Expire each entry after a TTL. Simple, but stale entries live until the TTL ends.
   - **B.** Cap the map with an LRU. Bounded memory, but hot entries can be evicted under load.

   Recommend A, because every caller already passes a timestamp.
   ```

   Letter ONLY options that exclude each other. Never letter steps that must all happen. Each option
   states the fix and its trade-off.
3. **Not Fixed.** Every `rejected`, `unclear`, and `blocked` issue, each with its reason. The user
   MAY override any of them.

Apply the answers. An approved or decided issue becomes `queued`, with the decision in its note. A
declined issue becomes `skipped`. Fix the new `queued` issues in further waves.

Discuss the `design` issues last, one at a time. Each discussion ends with a decision (`queued`) or
with `skipped`.

### 7. Verify

Fixers run targeted tests during the waves. After the last wave, run the full test suite yourself.

Attribute each failure to an issue by footprint: match the failing file or test to the files each
fixer touched. Send the failure back to one fixer for that issue, on the session default model. If
the second attempt also fails, set the issue to `blocked`. A failure that matches no footprint may
predate the fixes. Report it to the user and leave it alone.

If the project has no tests, state that in the final summary.

### 8. Report

Give the user a short summary:

- The count of issues per status.
- Every issue that is still `blocked`, `unclear`, or `awaiting-decision`, with its reason.
- The result of the full test suite.
- The report path, and a reminder that no change is committed.

## Tracking

The report is the only record of progress. A fresh session MUST be able to resume from the report
alone. Update the report at every status change. Do NOT batch the updates.

### Triage Table

Add the table as its own section directly after the Summary section of the report:

```markdown
## Triage

| ID | Title | Verdict | Judgment | Risk | Status | Files Touched | Note |
| --- | --- | --- | --- | --- | --- | --- | --- |
| SEC-1 | Query built from raw input | confirmed | mechanical | low | fixed | `db/users.js`, `test/users.test.js` | Regression test added. |
| LOG-2, PERF-1 | Cache never expires | confirmed | choice | medium | awaiting-decision | `cache/store.js` | Options: TTL or LRU. |
| STY-3 | Unused import | — | — | — | open | — | — |
```

- **Files Touched** holds the footprint until the fixer returns. Then the column holds the files
  that the fixer changed.
- **Note** holds what a fresh session needs to continue: the root-cause group, the validator's fix,
  the chosen option, or the reason for a block. Keep each note to one or two sentences. The
  validation block holds the decisive evidence, so the note MUST NOT repeat it.

### Validation Block

Add a validation block to an issue as soon as its validator returns. Write the block after the last
part of the issue: after the last `- **Also reported:**` bullet when present (`merge-review` adds
these), otherwise after **Confidence Note** when present, otherwise after **Proposed Solution**. The
reader then meets the context and the problem before the verdict.

The top bullet holds only the verdict. Sub-bullets hold the rest, each with a fixed label, in this
order:

| Sub-bullet | Present when | Content |
| --- | --- | --- |
| `Evidence` | Always | The validator's `REASON`, cut to one or two sentences that name the guard, caller, or test that decided the verdict. |
| `Fix` | Verdict is `confirmed-alt` | The validator's `FIX`: the fix to apply, and the fate of the proposed solution. |
| `Options` | Judgment is `choice` | The validator's `OPTIONS`: lettered fixes that exclude each other, each with its trade-off. An option that is the proposed solution says so. |
| `Recommend` | Judgment is `choice` | The validator's `RECOMMEND`: the letter and one reason. |

A `rejected` or `unclear` verdict has only `Evidence`. A plain `confirmed` verdict has no `Fix`,
because the Proposed Solution already holds the fix. Drop the `FIX` field that the validator returns
in that case.

```markdown
- **Validation:** rejected
  - Evidence: the guard at `api/users.js:42` returns early when `id` is empty, and a temporary test
    with an empty ID returned 400.
```

```markdown
- **Validation:** confirmed-alt
  - Evidence: `cache/store.js:30` adds entries and nothing removes them.
  - Fix: expire entries in `cache/store.js`. The proposed solution (clear on restart) is dropped,
    because the process runs for weeks.
  - Options: (A) TTL per entry; simple, but stale entries live until the TTL ends. (B) LRU cap;
    bounded memory, but hot entries can be evicted under load.
  - Recommend: A, because every caller already passes a timestamp.
```

When the validator replaces or extends the proposed solution, the `Fix` or `Options` sub-bullet MUST
state what became of the proposed solution. Either the proposal is one of the options, or the
sub-bullet names the reason it was dropped. You MUST NOT edit or remove the **Proposed Solution**
field itself.

Cut the validator's narrative. Keep the names of the files, lines, and tests that the summary cites,
so that a reader can check the verdict.

### Resolution Block

Add a resolution block to an issue when its status becomes `fixed`, `rejected`, `unclear`,
`blocked`, or `skipped`. Write the block directly after the validation block of the issue, or in its
place if the issue has none. The top bullet holds only the status. Sub-bullets hold the detail for
that status:

| Status | Sub-bullets |
| --- | --- |
| `fixed` | `Changed` (the fixer's `FILES`), `Tests` (`TESTS`), `Note` (`NOTE`). |
| `blocked` | `Reason` (the fixer's `NOTE`), then `Tests` if any ran. |
| `skipped` | `Reason`: the reason the user gave. |
| `rejected`, `unclear` | None. The validation block holds the evidence. |

```markdown
- **Resolution:** fixed
  - Changed: `cache/store.js`, `test/store.test.js`.
  - Tests: `npm test -- test/store.test.js` passed.
  - Note: entries now expire after a TTL; regression test added.
```

You MAY rewrite the fixer's text for clarity. You MUST keep every file, test command, and result.
Replace the block if the status changes later.
