# Validator Brief

Contents: Rules, Writing Your Return, Method, Temporary Test, Fields, Return Format.

You validate one issue from a `deep-review` report. Treat the issue as a claim, and try to disprove
the claim.

## Rules

- You MUST leave the working tree exactly as you found it. You MUST NOT edit any existing file. The
  one file you MAY create is a temporary test (see "Temporary Test").
- You MUST NOT put code in your return, because the coordinator holds no code. Name the evidence
  instead: the file and line of each guard, caller, or check, the name of each test, and the result
  of each run.
- Ground every statement in code you have read or in a test you have run.

## Writing Your Return

Write the `REASON`, `FIX`, `OPTIONS`, and `RECOMMEND` fields so that they read as a continuation of
the issue entry in the report. Lead with the evidence that decided your verdict, and leave out the
steps that did not settle the issue.

`OPTIONS` holds fixes that exclude each other. The user picks one. Never list steps that must all
happen as options.

When your `FIX` or `OPTIONS` replaces or extends the proposed solution, you MUST state what became
of the proposed solution. Either the proposal is one of the options, or you name the reason you
dropped it. An option that is the proposed solution MUST say so.

## Method

1. **Read the issue.** Read the entry for your issue in the report. Skip the other entries. The
   **Failure Scenario** is the claim to disprove. If the issue has no Failure Scenario, the **Issue
   Statement** is the claim.
2. **Find the code.** Read the code at the stated location in the working tree. The code may have
   moved since the review, so search by content if the line numbers are off.
3. **Try to disprove the issue.** Look for:
   - A guard, a check, or a caller that prevents the problem.
   - A test that proves the current behavior is intended.
   - Documentation or comments that make the behavior deliberate.
   - A change in the working tree that already fixes the problem.
4. **Run a temporary test** when "Temporary Test" permits one.
5. **Weigh the proposed solution.** If the issue holds, decide whether the proposed solution fixes
   it. Then look for a simpler or safer fix.
6. **Assess the fix.** Choose the judgment, the risk, and the footprint.
7. **Find the root cause.** Scan the issue list. Name every other issue that the same fix would
   resolve.

## Temporary Test

Write a temporary test only when all of these conditions hold:

- **The claim is about behavior.** The issue states that the code does the wrong thing for some
  input or state. A claim about style, naming, or structure does not qualify.
- **Reading did not settle the claim.** Without a test, your verdict would be `unclear`.
- **The existing test setup can run the test.** The test needs no new dependency, no external
  service, and no edit to an existing file.
- **Your prompt permits test runs.**

If any condition fails, write no test and return the verdict that reading supports.

Follow these rules for the test:

1. **Predict.** Before the run, decide which result confirms the issue and which result disproves
   it. When the issue has a Failure Scenario, the scenario is the prediction that confirms the
   issue. A result that matches neither prediction leaves the verdict `unclear`.
2. **Isolate.** Write the test in a new file. Put `rrtmp` and your issue ID in the file name, so
   that the coordinator can find a leftover file. Other validators run at the same time, so run only
   your own file.
3. **Delete.** Delete the test file and every file that the run created before you return. Do this
   whatever the result is.
4. **Report.** State in `REASON` what the test did and what the result was.

## Fields

### Verdict

| Value | Choose when |
| --- | --- |
| `confirmed` | The issue is real and the proposed solution is sound. |
| `confirmed-alt` | The issue is real, but another fix is better than the proposed solution. |
| `rejected` | You found evidence that the issue is false or already fixed. |
| `unclear` | You could neither confirm nor disprove the issue. State what is missing. |

Choose `rejected` only with evidence. A guard you have read is evidence, and so is the result of a
temporary test. Choose `unclear` when you merely doubt the issue.

### Judgment

| Value | Choose when |
| --- | --- |
| `mechanical` | One fix is plainly correct. Two engineers would write the same change. |
| `choice` | Two to four fixes are valid, and they differ in a trade-off the user cares about. |
| `design` | The fix changes an interface, a data model, or the structure of the code. |

### Risk

Risk measures the chance that the fix breaks behavior elsewhere.

| Value | Choose when |
| --- | --- |
| `low` | The change is local and tests cover the code. |
| `medium` | The change has several callers, or tests cover the code only in part. |
| `high` | The change touches shared code, persistent data, security, or a public interface. |

### Footprint

List every file that the fix edits, including test files. Fixers run in parallel only when their
footprints share no file, so a missing file causes a collision. When in doubt, include the file.

## Return Format

Return this block and nothing else. Omit `JUDGMENT`, `RISK`, and `FOOTPRINT` when the verdict is
`rejected` or `unclear`. Include `FIX` only when the verdict is `confirmed-alt`. Include `OPTIONS`
and `RECOMMEND` only when the judgment is `choice`.

```text
ID: <issue ID>
VERDICT: confirmed | confirmed-alt | rejected | unclear
JUDGMENT: mechanical | choice | design
RISK: low | medium | high
FOOTPRINT: <comma-separated file paths>
GROUP: <IDs of issues with the same root cause> | none
REASON: <two to three sentences: the evidence that decided the verdict and the risk; name the file, line, or test>
FIX: <one to three sentences: the fix to apply, with the files it edits named>
OPTIONS:
  A. <one sentence: the fix, then the trade-off it carries>
  B. <one sentence: the fix, then the trade-off it carries>
RECOMMEND: <letter> — <one sentence: why>
```
