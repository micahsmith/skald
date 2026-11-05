---
name: write-skill
description: Create a new skill, including its SKILL.md, reference files, and scripts. Use when the user asks to write a skill, or to turn a workflow, procedure, or set of instructions into one.
---

## Process

1. **Gather Requirements.** Ask the user one question at a time. Skip any question the
   conversation already answers.
   - Intention: what task does the skill solve?
   - Scope: what are the core use cases?
   - Practice: how does the user do the task today? What goes wrong? How does the user check the
     result?
   - Tests: 2–3 realistic requests the skill must handle. Give each request a pass check: a fact
     about the result that you can verify. If the user has no tests, propose them and confirm them
     with the user.

   Ask whether the user wants the tests run. Steps 2 and 5 run them in subagents. If the user
   declines, skip those steps.

2. **Baseline.** Run each test without the skill (see "Test Runs"). Record each failed pass check
   and what the subagent did wrong. If every check passes, tell the user that the skill may add
   nothing, and ask whether to continue.

3. **Write.** Read [GUIDELINES.md](GUIDELINES.md). Draft SKILL.md and any supporting files (see
   "Structure"). The draft MUST address every failure recorded in step 2.

4. **Self-Review.** Work through the GUIDELINES.md checklist, which starts with
   `scripts/check-skill.sh`.

5. **Test.** Run each test with the draft (see "Test Runs"). Then:
   - Fix the draft for each failed pass check, and for each instruction the subagent skipped or
     found unclear.
   - For content that no test used, add a test that needs it or delete the content.

   Repeat until every pass check holds, for at most 3 rounds.

6. **Review.** Give the user the draft and the test results, including any check that still fails.
   Iterate until the user approves. If the draft changed after step 5, rerun the tests before final
   approval.

## Test Runs

- Run each test in a fresh subagent. Give the subagent only the test's request. In step 5, also
  tell it to read the draft SKILL.md first, by absolute path. Do not reveal the pass check.
- Run each subagent in its own git worktree. Outside a git repository, tell the subagent to stop
  before any action with side effects and to report what it would have done.
- Tell the subagent to end its report with the steps it took and the files it read. In step 5, also
  ask for the skill sections it followed and any instruction it found unclear.
- Check each pass check yourself against the result. Do not rely on the subagent's own verdict.
- If the skill ships in a plugin, `claude plugin eval` MAY run the tests as a suite. A suite can
  also check whether the description triggers the skill.

## Structure

Name each sibling file in capitals, such as `EXAMPLES.md`.

### Save Location

Resolve the base directory for the skill as follows:

1. **Skills Directory.** If the repository has a directory conventionally used for skills
   (`skills/`, `.claude/skills/`, `plugin/skills/`, or similar), use it.
2. **Scratch Directory.** Otherwise, if an existing scratch directory is present at the repository
   root (`tmp/`, `temp/`, `scratch/`, or similar, especially if git-ignored), use it.
3. **Fallback.** Otherwise, use the repository root, or the current working directory if not in a
   repository.

Create the skill directory inside the base directory, named after the skill. You MUST NOT use
`mktemp` or a system temp directory.
