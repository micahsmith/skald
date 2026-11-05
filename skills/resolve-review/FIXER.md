# Fixer Brief

You fix one unit of work from a `deep-review` report. A unit is one issue, or several issues that
share a root cause.

## Rules

- Other fixers share the working tree with you. You MUST NOT commit, stage, stash, reset, or switch
  branches.
- Edit only the files in your footprint. You MAY create new files. If the fix needs another existing
  file, stop and return `blocked` with the name of that file.
- Make the smallest change that fixes the issue. Do not refactor, rename, or reformat other code.
- You MUST NOT edit the report.
- You MUST NOT put code in your return. The coordinator holds no code.

## Writing Your Return

Write the `NOTE` and `TESTS` fields so that they read as a continuation of the issue entry in the
report. The coordinator never reads the code, so the fields MUST name each file you changed, each
test you added, and each test command you ran with its result.

## Method

1. **Read.** Read the entry for each of your issues in the report. Then read the code in your
   footprint. If your prompt says "partial fix may exist", inspect the uncommitted changes in your
   footprint. Finish the partial fix or replace it.
2. **Write the regression test.** Each fix to a bug MUST include a test that fails before the fix
   and passes after it. Write the test first, and confirm that it fails. Skip this step when:
   - the fix changes no behavior (style, comments, naming), or
   - the project has no tests.
3. **Fix.** Apply the fix named in your prompt. Follow the patterns of the surrounding code.
4. **Test.** Run the regression test and the tests that cover your footprint. Run only targeted
   tests, because the coordinator runs the full suite. Run no tests if your prompt forbids them.
5. **Return.**

## When to Return `blocked`

Return `blocked` when:

- the fix needs an existing file outside your footprint,
- the fix named in your prompt does not resolve the issue,
- the fix needs a decision that your prompt does not contain, or
- a targeted test still fails after three attempts at the fix.

Before you return `blocked`, undo your own edits by hand. You MUST NOT use `git` to undo them,
because `git` would also discard changes that belong to the user.

## Return Format

Return one block per issue and nothing else:

```text
ID: <issue ID>
STATUS: fixed | blocked
FILES: <comma-separated paths of the files you changed or created>
TESTS: <command> — passed | failed | not run (<reason>)
NOTE: <two to four sentences: what changed and where, or why the issue is blocked>
```
