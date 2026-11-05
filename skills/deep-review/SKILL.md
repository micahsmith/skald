---
name: deep-review
description: Runs a deep review on a branch with six focuses (security and privacy, performance, logic, compatibility, design, style) and creates a consolidated report. Use when the user asks to deeply review or audit a branch without disturbing the working tree.
argument-hint: "[target] [base] — default behavior compares `HEAD` against `main`/`master`"
disable-model-invocation: true
---

Review the changes on a branch against a base branch, then write one consolidated report. The review
is read-only. It MUST NOT switch branches or touch the user's working tree, so it cannot interfere
with other work in progress.

Six reviewers do the review, and each has one focus: security and privacy, performance, logic,
compatibility, design, or style. Each reviewer explores the changes freely and writes a report. The
six reports are then combined into one final report.

## Arguments

The arguments indicate the branch under review (`TARGET`) and the branch to compare against
(`BASE`). Accept either positional references or plain language ("review this branch against the
`dev` branch").

- No arguments: `TARGET` is the current branch. `BASE` is `main`, `master`, or whichever branch
  serves as the primary branch. If you cannot find an obvious branch candidate, ask the user.
- One argument: the argument is `TARGET`. Resolve `BASE` using the same rules as above.
- Two arguments: the first argument is `TARGET` and the second is `BASE`.

By default, fetch the latest from the remote. Pass `--no-fetch` to the prep script when the user
asks to skip the fetch or is working offline.

## Steps

### 1. Prepare

Run the prep script with the resolved references:

```sh
bash scripts/prepare-review.sh [--no-fetch] [TARGET] [BASE]
```

The script will resolve the references, compute the merge-base, and decide whether the review can be
done in place or in a throwaway worktree. It returns a summary block that delineates the work to be
reviewed. The summary block includes:

- `WORKDIR`: where reviewers undertake the review (the worktree or the repo root if the review is
  "in place").
- `TARGET_SHA`: the commit SHA containing the changes to be reviewed (`TARGET`).
- `MERGE_BASE`: the `BASE` against which `TARGET` is compared.
- `DIFF_CMD`: the diff command reviewers MUST run.
- `CLEANUP`: the command to clean up the worktree (if present) after the review is finished.
- The diffstat and changed-file list.

If the script fails, report the cause to the user.

### 2. First Pass Summary

Spawn one agent to read the diffstat and changed-file list. This agent MUST run on a small model
whenever available. The agent MUST write a 3-6 sentence plain language summary of the changes. Pass
this summary to all reviewers as shared initial context. The summary also opens the final report
(see "Report Structure").

### 3. Parallel Fan-Out

Call the six reviewers to run in parallel (the same message passed to six `Agent` calls). Do NOT
paste the diff into their prompts. Provide each reviewer with the same context and with its
individual focus:

- `WORKDIR`, `MERGE_BASE`, `TARGET_SHA`, `DIFF_CMD`, diffstat, and changed-file list.
- The first pass summary from step 2.
- Instructions on issue report format and individual focus.
- The drop rule and the "Considered, Not Reported" table format, both as written below.

Pass each focus exactly as written below. You MUST NOT add example issues, suspected problems, or
areas to check. The coordinator has not read the code, so its guesses can bias what a reviewer
reports.

Each reviewer MUST run the `DIFF_CMD` itself, explore under `WORKDIR`, and return with a report. A
reviewer MUST NOT leave test files out of the diff it reads. Always launch all six reviewers
regardless of how small the diff is. Reviewers MAY report "No issues found".

The six focuses:

- **Security and Privacy**
  - Leaked secrets
  - Injection threats
  - Authentication/authorization failures
  - Unsafe input handling
  - Unsafe handling of sensitive data
- **Performance**
  - Needless work or complexity
  - N+1 queries
  - Wasteful memory allocations or I/O ops
- **Logic**
  - Errors in logic
  - Mismatch with intent
  - Broken edge cases
  - Race conditions
  - Incorrect or incomplete error handling
  - Tests that pass for the wrong reason, or that leak state into later tests

  The logic reviewer MUST search in these passes, in order:

  1. **Line by line.** Read every changed hunk and the whole function around it. For each line, ask
     what input, state, or timing makes the line wrong. Bugs in unchanged lines of a changed
     function are in scope.
  2. **Removed behavior.** For each deleted or replaced line, name the rule it enforced. Then find
     where the new code enforces that rule. If no code does, report it.
  3. **Swallowed errors.** Find each catch, recover, or fallback that the diff adds or touches.
     Check that it logs or rethrows. Check that it does not also catch cancellation or interrupts.
  4. **Language traps.** Check the diff for the known traps of its language and framework.
  5. **Tests.** Check that each test's assertion holds because of the code under test, not because
     of a mock. Check that each test's teardown undoes its setup.
  6. **Gaps.** Re-read the diff one last time, looking only for issues not yet listed.
- **Compatibility**
  - Unintended breaks in backwards compatibility

  The compatibility reviewer MUST search in these passes, in order:

  1. **Callers.** For each changed function, find every caller. Check each caller against the new
     preconditions, return values, exceptions, and ordering.
  2. **Gaps.** Re-read the diff one last time, looking only for issues not yet listed.
- **Design**
  - The same fact kept in more than one place
  - Values that must agree but travel by separate paths
  - Defaults, parameters, branches, or fields that nothing uses
  - Fixes at the wrong depth

  The design reviewer MUST search in these passes, in order:

  1. **One fact, one place.** For each list, rule, number, order, or default that the diff writes,
     search the codebase for every other place that states the same fact. Report each copy that
     only a comment or a naming convention keeps in step. A test that fails when the copies
     disagree counts as a guard.
  2. **Values that travel together.** For each value that the diff passes between functions, check
     whether the receiver also needs a second value that arrives by another path. Report each pair
     that a caller can pass out of step.
  3. **Unused paths.** For each default, parameter, branch, or field that the diff adds, find a
     caller or an input that uses it. Report each one that nothing uses.
  4. **Altitude.** For each problem that the diff fixes, name its root cause. Then check whether the
     fix changes the mechanism that causes the problem or adds a special case beside it. Report a
     special case when a change to the shared mechanism would remove the problem.
  5. **Gaps.** Re-read the diff one last time, looking only for issues not yet listed.

  A design issue whose scenario needs a later edit is a Suggestion or a Style issue.
- **Style**
  - Non-idiomatic use of language or framework
  - Violation of existing codebase patterns
  - Illegible or hard to decipher code blocks
  - Excessive or trivial comments on well-written code
  - Violation of any rules provided in `AGENTS.md`, `CLAUDE.md`, and the like

On top of their passes, the logic, compatibility, and design reviewers MUST apply the scenario rule:

1. For each candidate problem, write the concrete scenario. For a logic, edge case, or error
   handling problem, write the input or state, then the wrong output or crash. For an intent or
   compatibility problem, write the behavior that a caller or the first pass summary expects, then
   the behavior the code gives. For a design problem, write a plausible later edit that changes
   only one copy or one path, then the wrong output or crash that follows.
2. Drop any candidate that has no concrete scenario.

Rule for reviewers: report ONLY issues pertinent to your focus. However, if an issue MIGHT pertain
to your focus, report it even when it also belongs to another focus. Several reviewers MAY report
the same issue.

Every reviewer MUST apply the drop rule to each candidate in its focus. A reviewer MAY leave such a
candidate out of its issues for only these reasons:

- **False.** The reviewer checked the candidate, and the code does not behave as the candidate
  claims.
- **No scenario.** The candidate has no concrete scenario under the scenario rule. Only the
  reviewers that apply the scenario rule MAY use this reason.
- **Accepted by intent.** A spec, plan, or ticket that the reviewer read accepts this exact
  behavior. The reviewer MUST cite the document and the section.

Any other doubt lowers the severity or the confidence of the issue. Such a doubt MUST NOT remove the
issue. Examples are "no current caller triggers it", "the cost is small", and "an earlier decision
seems related". The reviewer MUST state the doubt in the Confidence Note.

Every reviewer MUST end its report with a "Considered, Not Reported" section. The section holds one
table row per dropped candidate:

```markdown
| Location | Candidate | Reason | Detail |
|---|---|---|---|
| `path/to/file.js:120` | One sentence that states the candidate | False | The evidence that the claim is false |
```

`Reason` is `False`, `No scenario`, or `Accepted by intent`. For `Accepted by intent`, `Detail`
cites the document and the section. A reviewer that dropped no candidate writes "None." under the
heading.

### 4. Consolidated Report

Collate, deduplicate, and organize the six reports. Do NOT overrule a reviewer: every issue a
reviewer flags MUST reach the final report. Merge an issue raised by several reviewers into a single
entry that names every ID which reported it. A merged entry MUST have a single Context block and at
most one Failure Scenario. Rewrite each rather than concatenating versions from each reviewer.

Collect the "Considered, Not Reported" rows of all six reports into one table. Add a `Reviewer`
column that names the focus. Merge rows that describe the same candidate, and name every reviewer
that dropped it. Leave out a row when another reviewer reported the same problem as an issue. In
that case, add one sentence to the issue's Confidence Note. The sentence names the reviewer that
dropped the candidate and states that reviewer's reason. Do NOT leave out a row for any other
reason.

### 5. Final Report and Summary

Write the full report to a file named `review-<target>-<date>.md`, where `<date>` is the output of
`date +%F`. Resolve the base directory for the file as follows:

1. **Reviews Directory.** If the repository has a directory conventionally used for reviews
   (`reviews/` or similar), use it.
2. **Scratch Directory.** Otherwise, if an existing scratch directory is present at the repository
   root (`tmp/`, `temp/`, `scratch/`, or similar, especially if git-ignored), use it.
3. **Fallback.** Otherwise, use the repository root, or the current working directory if not in a
   repository.

Use `mktemp` or a system temp directory ONLY inside a script bundled with this skill.

Report the file location to the user with a short summary of findings: the count of issues by
severity, the top-level blocking issues, and the count of rows in the "Considered, Not Reported"
table for each reason. Do NOT reproduce the entire report unless it contains three or fewer issues.

### 6. Cleanup

If the prep script returned a `CLEANUP` line, run that command to remove the worktree.

## Issue Format

Every reviewer MUST use this standard format for every issue. Write each issue in plain and simple
language. You MAY use technical terms that developers commonly understand, but you MUST avoid
jargon. A reader MUST be able to understand each issue from the report alone.

Use this format:

```markdown
### [SEC-1] <one-line title>
- **Severity:** Blocking | Major | Suggestion | Style
- **Confidence:** High | Medium | Low
- **Location:** `path/to/file.js:120-134`

<code snippet>

**Context:** Why this code exists: what the file or class is responsible for, what job this code
does for the work on the branch, and who consumes it. Do NOT describe the problem here. Write 3-6
sentences, grounded in code you have read. If the purpose is unclear, state plainly that the purpose
is unclear.

**Failure Scenario:** (REQUIRED when any ID in the heading is `LOG`, `COM`, or `DES`, OPTIONAL
otherwise)
The input, state, or expected behavior, then the wrong output, crash, or actual behavior. One or
two sentences.

**Issue Statement:** What is the problem and why is it a problem.

**Proposed Solution:** How the problem could be resolved, and why the proposed solution fixes it.
Propose ONE solution. If other fixes are also valid, name them in a separate sentence that starts
with "Alternatives:", joined by "or". A list of steps means steps that all happen; never use a
list for alternatives.

**Confidence Note:** (REQUIRED when Confidence is Medium or Low) Why confidence was not High and
what ambiguity or intent needs to be resolved in order to confirm status.
```

Use one ID prefix per focus:

- `SEC` for security and privacy
- `PERF` for performance
- `LOG` for logic
- `COM` for compatibility
- `DES` for design
- `STY` for style

Group issues by severity, blocking first. Open each report with a 2-3 sentence summary.

## Report Structure

The final report MUST use this format:

1. **What This Branch Does.** The first pass summary from step 2, so that the report states the
   purpose of the work before it lists problems with the work.
2. **Summary.** A table of issue counts per severity per focus followed by the list of blocking
   issues.
3. **Issues by Severity.** Issues in order: Blocking → Major → Suggestion → Style. Each issue keeps
   its ID and focus tag. A merged issue MUST list the ID from each reviewer and combine their
   content.
4. **Considered, Not Reported.** The merged table from step 4. Sort its rows by reason: `Accepted by
   intent` first, then `No scenario`, then `False`.
