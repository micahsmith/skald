---
name: write-plan
description: Create an implementation plan that an agent loop can execute on its own. The plan includes instructions for running it and a completion sentinel that the loop detects. Use it to break a spec or design into concrete chunks of work.
disable-model-invocation: true
---

Create an implementation plan from a spec, or from a design given in the conversation. Write the
plan as an ordered set of tracer-bullet slices. A tracer-bullet slice cuts through every layer the
feature touches. Each slice is small enough to build and verify on its own, and complete enough to
demonstrate working behavior end to end.

The plan MUST be self-contained, because an agent loop will execute it. An agent loop runs an agent
repeatedly until a stated goal is judged complete. Assume no other context, skills, or conversation
is available while the plan runs. The loop reads only the conversation transcript. So every plan
MUST end with a literal completion sentinel that the loop can detect.

Every plan therefore encodes **decisions**, **slices**, **tests**, and an **end condition**.

Plans MAY include code and implementation details, but ONLY when the plan cannot otherwise be
executed successfully. Leave most implementation to the executing agent. The executor writes the
code. The plan constrains only the choices that are costly to get wrong or that would drift from the
agreed design.

A full spec file is the preferred input, but it is NOT REQUIRED. If you lack the context to create a
plan, use `/brainstorm` and `/write-spec` first.

## Process

1. **Explore.** Read any relevant code, tests, and documentation. Explore deeply. You MUST
   understand the layers of code the work crosses, the standards and patterns to follow, and how
   tests are written and run. Explore until you can define slices, ordering and dependencies,
   decision anchors, and concrete test cases.
2. **Draft Slices.** Decompose the work into tracer-bullet slices (see "Slices").
3. **Interview & Iterate.** Present the draft. Confirm with the user before proceeding further. To
   resolve gaps, ask one question at a time.
4. **Write.** Create the plan file (see "Output").
5. **Review.** Scan the draft and fix problems inline (see "Self-Review").
6. **Report.** Report the plan file path and a short summary of its contents.

## Plan Structure

A plan has four parts:

1. **Header.** The header includes the feature name, a one-sentence statement of the goal, a link to
   the spec (if it exists), and the slice index with ordering.
2. **Execution Protocol.** The standardized block (see "Execution Protocol").
3. **Slices.** The work broken down into tracer-bullet style chunks.
4. **Completion Sentinel.** The last part of the plan is the completion report template (see
   "Completion Sentinel").

### Execution Protocol

Write this block into every plan verbatim. The block tells the executing agent how to run the plan
inside a loop.

```markdown
## How To Execute This Plan

You are to implement the plan presented here. Do your work in a worktree. Dispatch each slice of
work to a fresh subagent if possible. Prove each slice with surfaced test output. Stop when you
emit the completion report.

1. **Worktree.** Do all work in a git worktree (if worktrees are available) named after this plan
   file.
2. **Sequential Slices in Order.** Follow the ordering for slices. Start a slice ONLY after every
   slice that blocks it has passed verification. Independent slices MAY run in parallel ONLY if
   each runs in its own worktree. Independent worktrees MUST be merged back into the main worktree
   once complete.
3. **Delegate Slices to Subagents.** Each subagent MUST receive ONLY this execution protocol and
   details on the slice it is implementing (the plan file path and the slice title). Keep context
   windows small.
4. **TDD.** Use red/green/refactor. Demonstrate failure of the test first, write the minimum
   implementation to make it pass, and then demonstrate success. Lastly, refactor and iterate.
5. **Surface Proof.** Each subagent MUST report test results and the checkpoint status for the
   slice. You MUST include this proof in the main conversation so that completion is evaluable.
6. **Terminate.** When every slice checkpoint is verified, run the full test suite, exhibit that
   all tests pass, and print the completion report verbatim.
```

### Slices

Write slices in order so that each one builds on its predecessors. Once its blockers are complete,
each slice MUST be executable on its own.

If the current code would make the feature slices hard to build, add a **prefactoring** slice first.
A prefactoring slice restructures code without changing its behavior. Include this slice ONLY when
its value is high.

Each slice MUST contain:

- **Title.** The title MUST describe the end-to-end behavior the slice delivers.
- **Blocked By.** Slices that MUST be implemented before work can proceed, else "None".
- **What to Build.** Behavior across the layers the slice intersects (e.g. schema, API, UI). The
  constraints and invariants that MUST be followed. ONLY include code if it is the simplest way to
  express the constraints and invariants.
- **Decision Anchors.** Interfaces, signatures, and contracts that the implementation MUST satisfy.
- **Test Cases.** One test case for each behavior. Tests MUST cover happy paths, edge cases, and
  error cases. Each test case states a **name**, **setup/preconditions**, **action** with concrete
  inputs, and the **expected observable result** with concrete values. Do NOT write test code. See
  "Tests" for additional instructions.
- **Checkpoint.** The observable state when the implementation is done, stated so that the executor
  can surface it as proof of completion (which tests have passed and which behavior is demoable).

### Completion Sentinel

End the plan with the report the executor prints only when all slices are verified. This sentinel
allows loops to detect completion:

```markdown
## Completion Report

Print this ONLY when all slice checkpoints are verified:

- One line per slice: its title, ✅, and the checkpoint proof.
- The full test suite command and its output.
- Final line verbatim: `PLAN COMPLETE: plan-<date>.md`
```

## Output

### Save Location

Resolve the base directory for the plan as follows:

1. **Spec Directory.** If a spec for this work exists, write the plan into the SAME directory as the
   spec.
2. **Conventional Spec Directory.** If a directory conventionally used for plans or specs already
   exists (`plans/`, `specs/`, `prds/`, or similar), use it.
3. **Scratch Directory.** Otherwise, if an existing scratch directory is present at the repository
   root (`tmp/`, `temp/`, `scratch/`, or similar, especially if git-ignored), use it.
4. **Fallback.** Otherwise, use the repository root, or the current working directory if not in a
   repository.

If no spec exists, create a new directory inside the resolved base directory. Name the new directory
in kebab-case, using at most four words that describe the work. Write the plan there.

Use `mktemp` or a system temp directory ONLY inside a script bundled with this skill.

### Filename

Create the file named `plan-<date>.md`, where `<date>` is the output of `date +%F`.

### Splitting

The default SHOULD be a single file. If the plan grows very large (>1500 lines), split the plan into
a series of sibling files named `plan-<date>-<slice>.md` and keep the `plan-<date>.md` as the
**index**. The index holds the header, the execution protocol, the slice list with ordering, and the
completion report. It also points to every other plan file. Do NOT pre-split small plans.

## Self-Review

Before the final report, scan the draft and fix problems inline:

- Placeholders or TODOs.
- Missing test cases.
- Test cases without concrete and observable results.
- Missing edge/error cases.
- Cyclic or contradictory slice ordering.
- Decision anchors that contradict the spec.
- Missing execution protocol or completion sentinel.

The user will review the file conversationally.

## Tests

Tests MAY be either unit or integration tests. Tests MAY verify either behavior or properties. Write
tests to guide the implementation. Excessive fixtures and complicated mocking often indicate that
the unit under test needs refactoring.

Write every test to verify current state and behavior. Delete any test that only confirms behavior
changed from an earlier state of the codebase. Remove such tests before you report completion.
