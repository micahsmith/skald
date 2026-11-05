# Skill Writing Guidelines

Version 1.

Apply these rules to every file in a skill. An agent reads the skill on every use, so each line
costs tokens and attention every time.

## What Earns a Line

A sentence earns its place only if it gives the agent one of the following:

- knowledge the model lacks;
- a convention specific to the user, project, or organization;
- a fix for a failure the agent shows without the sentence.

Apply the deletion test to each sentence: delete it, then name the behavior that would change. If
you cannot name a behavior, the sentence MUST stay deleted.

Every rule MUST trace to the user, the codebase, or an observed failure. Ask the user for a missing
procedure. Do not invent it from general knowledge.

Leave out content the agent can discover or already knows: file lists, directory layouts, README
content, and explanations of common tools. If the agent needs a specific file, give its path.

## Rules Over Judgment

Write a test wherever the agent would otherwise judge. Judgment phrases include "as needed", "if
appropriate", "where relevant", "be thorough", and "ensure quality". Replace each with one of these
tests:

| Test                              | Example                                                 |
| --------------------------------- | ------------------------------------------------------- |
| A yes/no question                 | "Does each test case state a concrete expected value?"  |
| A command with an expected result | "`cargo test` exits 0."                                 |
| A threshold                       | "Split the file when it exceeds 500 lines."             |

Match rigidity to fragility:

- **Fragile operations** (paths, formats, commands, completion markers): give exact values and
  steps.
- **Context-dependent work**: give decision rules ("If X, do Y. Otherwise, do Z."). Fix the order
  of steps only when a later step depends on an earlier one.
- **Choices**: name one default. Name an alternative only with the condition that selects it: "Use
  `jq`. If `jq` is missing, use Python."

Verify with the strongest check available:

1. A script or command whose output decides pass or fail.
2. A yes/no checklist.
3. Open-ended review ("check your work"). This MUST NOT be the only check.

## Wording

- Write modality with RFC 2119 keywords in capitals: MUST, MUST NOT, SHOULD, SHOULD NOT, MAY.
  Capitalize no other word for emphasis. Write "only", "not", "never", and "always" in lowercase.
- Use MUST only when a violation breaks the result. Use SHOULD for a default with known exceptions.
  Write an ordinary step as a plain imperative.
- Add no intensifiers such as "CRITICAL" or "IMPORTANT". State each rule once. Emphasis makes
  agents over-apply rules.
- Follow each MUST NOT with the action to take instead, when one exists.
- Give a reason only when the agent must apply a rule to cases the skill does not list. Keep the
  reason to one clause: "Write plain text, because a speech engine reads the output aloud."

## Cost

An instruction costs its tokens plus the work it causes. Agents follow instructions closely, so an
unbounded instruction can be the most expensive line in a skill.

- Every instruction that starts open-ended work (explore, search, test, verify, ask) MUST state its
  scope or its stopping condition: "Explore until you can name every file the change touches."
- State constraints that apply to every step, such as scope limits and forbidden actions, before
  the steps.
- Give each skill one job. Split a skill whose use cases share no steps.

## Structure

- Keep SKILL.md under 500 lines.
- Keep content that every run needs in SKILL.md. Move content into a sibling file when some runs do
  not need it. In SKILL.md, state when to read each sibling file.
- Link every sibling file from SKILL.md. Do not link sibling files from each other.
- Start any sibling file over 100 lines with a table of contents.
- When examples show a pattern, vary their surface details so the agent copies only the pattern.

## Scripts

- Use a script for any operation that must be deterministic or repeatable.
- Keep a script of 12 lines or fewer inline. Put a longer script in `scripts/`.
- Write scripts to run on every platform the skill targets. Use forward slashes in paths.
- A script MUST handle its own errors. Its error messages MUST say what failed and how to fix it.
- Tell the agent to run a script, not read it, unless the agent must adapt the script.

## Description

- State what the skill does.
- For a skill the model may invoke, also state when to use it. Name the requests, situations, or
  file types that should trigger it.
- If the frontmatter sets `disable-model-invocation: true`, the "what" sentence is enough.
- Use at most 3 sentences and at most 1024 characters.

## Checklist

Answer each question yes or no. Fix every "no".

1. Does every sentence pass the deletion test?
2. Does every rule trace to the user, the codebase, or an observed failure?
3. Has every judgment phrase become a question, a command, or a threshold?
4. Does every choice name one default?
5. Does every open-ended instruction state its scope or stopping condition?
6. Are RFC 2119 keywords the only words capitalized for emphasis?
7. Would a violation of each MUST break the result?
8. Does each MUST NOT name the action to take instead, where one exists?
9. Does each reason cover cases the skill does not list?
10. Does each verification step use more than open-ended review?
11. Does the description state when to use the skill, unless model invocation is disabled?
12. Is SKILL.md under 500 lines, with every sibling file linked from SKILL.md?
