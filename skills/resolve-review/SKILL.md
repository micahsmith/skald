---
name: resolve-review
description: Triage and resolve issues from a `deep-review` report. Validates every issue, delegates simple low-risk fixes, walks through more complex review items, and tracks the status of review items.
argument-hint: "[report-path]"
disable-model-invocation: true
---

You are the coordinator. Subagents read and change the code. You handle verdicts, decisions, and
statuses.

## Rules

- You MUST NOT read source code, so that your context stays small. You MAY read the report, `git`
  status output, and test output.
- You alone edit the report, and only with `scripts/triage.sh`. Run the script; do not read it.
- Record each change in the report as soon as it happens, so that a fresh session can resume from
  the report alone.
- All fixes land in the shared working tree. You MUST NOT commit, stage, stash, or switch branches.
  The user handles commits.
- Run at most 4 subagents (validators or fixers) at once.
- Accept only a `deep-review` report. For findings from another review, tell the user to run
  `merge-review` first.

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

If you found the report by search, confirm it with the user before you continue.

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

Run the script:

```sh
bash scripts/triage.sh ingest <report-path>
bash scripts/triage.sh load <report-path>
```

`ingest` records any return file that an earlier session left. `load` adds the triage table if the
report has none, with every issue `open`. It prints the returns directory as `RETURNS=<path>`, then
each row that is not `fixed`, `rejected`, or `skipped`. Handle each row by its status:

| Status | Action |
| --- | --- |
| `open` | Validate the issue in step 3. |
| `queued` | Fix the issue in step 4. |
| `awaiting-decision`, `unclear`, `blocked` | Include the issue in the decision batch in step 5. |
| `in-progress` | The row is stale, because no fixer survives a session. See below. |

For each stale `in-progress` row, run `git status --porcelain -- <files>` on the files in the row.
Then set the status to `queued`. If any file has changes, add "partial fix may exist" to the note,
after the existing note:

```sh
bash scripts/triage.sh set <report-path> <ID> status=queued note="<note>; partial fix may exist"
```

### 3. Validate

First, find out how tests run. If the project guidance in your context already states the test
commands, use them. Otherwise, spawn one subagent to report. This subagent SHOULD run on a small
model (such as Haiku) when the harness can choose one. It reports:

- The command that runs the full test suite.
- The command form that runs a single test file or test case.
- Whether several test runs can happen at once in one working tree. Shared databases, fixed ports,
  and shared build directories prevent this.

Then validate the `open` issues in batches of at most 4. Start the validators of a batch together,
and wait until every one has returned. Give each validator:

- The absolute path to [VALIDATOR.md](VALIDATOR.md), which the validator MUST read first.
- The report path and the ID of its issue.
- Its return path, `<RETURNS>/<ID>.txt`.
- The issue list from step 1, so that the validator can name issues that share a root cause.
- The test commands. If test runs cannot happen at once, tell the validator to run no tests.

After each batch, run `bash scripts/triage.sh ingest <report-path>`. It records each verdict,
judgment, risk, footprint, and status, and it writes each validation block. It prints one line per
issue. If it prints an `ERROR` for a file, it keeps that file. Run that validator once more, then
run `ingest` again. If the second return also fails, report the error to the user.

A validator MAY write a temporary test when reading the code cannot settle an issue. Every temporary
test has `rrtmp` in its file name, and the validator deletes it before returning. A validator that
stops early can leave the file behind. So end this step by listing the leftover temporary tests,
even when no issue was `open`. Delete each file listed:

```sh
git ls-files --others --exclude-standard | grep rrtmp
```

### 4. Fix in Waves

Start the first wave as soon as validation ends. Do not wait for the user.

Plan each wave with `bash scripts/triage.sh wave <report-path>`. It groups the queued issues that
share a root cause into units. It picks at most 4 units whose footprints share no file, and it sets
their issues to `in-progress`. For each unit, it prints the IDs, the footprint, the notes, and the
return path.

Start the fixers of a wave together, one per unit, and wait until every one has returned. Each
fixer SHOULD run on a mid-size model (such as Sonnet) when the harness can choose one. Give each
fixer:

- The absolute path to [FIXER.md](FIXER.md), which the fixer MUST read first.
- The report path, and the unit's IDs, footprint, notes, and return path, as `wave` printed them.
- The fix to apply to each issue: the validator's fix when the note says so, the option the user
  chose when the note names one, and the proposed solution otherwise.
- The test commands. If test runs cannot happen at once, tell the fixer to run no tests.

After each wave, run `ingest`. It sets each issue to `fixed`, `queued`, or `blocked`, and it writes
each resolution block. Handle each issue that is not `fixed` as follows:

- **`queued`.** The fix needs a file outside the footprint, and `ingest` added the file to the
  footprint. A later wave gives the unit to a fresh fixer on the mid-size model.
- **`blocked`.** Retry the unit once on the session default model. Set each issue in the unit to
  `in-progress` with `set`. Include the note that `ingest` printed in the retry prompt, and give the
  retry the same return path. Wait for the retries, then run `ingest` before the next wave. If a
  retry also returns `blocked`, include the issue in the decision batch.

Run waves until `wave` prints `No queued issues.`

### 5. Ask the User in One Batch

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

   Letter only options that exclude each other. Never letter steps that must all happen. Each option
   states the fix and its trade-off.
3. **Not Fixed.** Every `rejected`, `unclear`, and `blocked` issue, each with its reason. The user
   MAY override any of them. An overridden issue becomes `queued`.

Apply each answer with `set`. An approved or decided issue becomes `queued`, with the decision in
its note. A declined issue becomes `skipped`, with the user's reason. Keep each note to one or two
sentences, and do not repeat the validation block:

```sh
bash scripts/triage.sh set <report-path> LOG-2 status=queued note="User chose A."
bash scripts/triage.sh set <report-path> SEC-1 status=skipped reason="The user accepts the risk."
```

Fix the new `queued` issues in further waves.

Discuss the `design` issues last, one at a time. Each discussion ends with a decision (`queued`) or
with `skipped`.

### 6. Verify

After the last wave, run the full test suite yourself.

Attribute each failure to an issue by footprint: match the failing file or test to the files each
fixer touched, which the triage table lists. Send the failure back to one fixer for that issue, on
the session default model. Set the issue to `in-progress` first, give the fixer the issue's return
path, and run `ingest` when it returns. If the suite fails again, set the issue to `blocked`, with
the failing test as the reason. A failure that matches no footprint may predate the fixes. Report it
to the user and leave it alone.

If the project has no tests, state that in the final summary.

### 7. Report

Run `bash scripts/triage.sh load <report-path>` for the final rows. Give the user a short summary:

- The count of issues per status.
- Every issue that is still `blocked`, `unclear`, or `awaiting-decision`, with its reason.
- The result of the full test suite.
- The report path, and a reminder that no change is committed.

Then delete the returns directory if it is empty: `rmdir <RETURNS>`.
