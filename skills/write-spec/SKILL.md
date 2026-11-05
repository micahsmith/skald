---
name: write-spec
description: Create a spec detailing the design decisions of a feature. This spec includes what the feature is, why it is being built, and the high-level technical decisions that will constrain implementation.
disable-model-invocation: true
---

Create a spec file that records key information: **what** will be built, **why** it is being built,
and any architectural decisions that constrain implementation. The spec SHOULD be stable and
readable. Do NOT include file paths, code, task sequencing, or implementation details, because these
items are prone to rot.

Default behavior is to synthesize the spec from the current conversation and exploration of the
codebase. Interview the user to fill in gaps. If there is little to no context, use `/brainstorm`.

## Process

1. **Explore.** Read relevant code, documentation, and recent commits. Read enough to understand the
   scope and implications of technical decisions and to avoid specifying existing behavior. Do NOT
   run an exhaustive exploration.
2. **Draft.** Write a draft of each section (see "Template").
3. **Interview & Iterate.** Examine the draft and identify missing information or weak points. If a
   required section is missing or anemic, interview the user. Ask one question at a time. Be
   comprehensive: fill every gap that would change the spec. Do NOT ask a long series of questions
   about minor details.
4. **Write.** Create the spec file (see "Output").
5. **Review.** Review the file and fix any remaining issues inline (see "Self-Review").
6. **Report.** Report the spec file path and a short summary of its contents.

## Template

1. **Problem & Context.** A thorough description of the problem from the user's perspective and an
   explanation of why it needs resolution.
2. **Scope.** A statement of the goals and non-goals of the changes. You MUST state non-goals
   explicitly ("changes to authentication will NOT be made").
3. **Solution.** The description of how the problem will be solved.
4. **Requirements.** The complete breakdown of what must be achieved by the changes introduced. Use
   an explicit breakdown into **Functional** and **Non-Functional** requirements (non-functional
   requirements include performance, security, accessibility, reliability, and similar). You MAY
   frame requirements as user stories or as a list of system behaviors/properties.
5. **Technical Decisions.** Description of high-level decisions and contracts. This section MAY
   include component boundaries, data flow, and schema shapes. Do NOT include file path references
   or code snippets (unless strictly constrained to spelling out schema shapes).
6. **Acceptance Criteria.** Every criterion MUST be observable and binary. Write each criterion as a
   plain testable statement. You MAY use Given/When/Then only when that structure adds clarity. Each
   criterion MUST reference the requirement it validates.
7. **Open Questions & Risks.** A statement of known unknowns or deferred decisions and possible
   risks.

## Output

### Save Location

Resolve the base directory for the spec as follows:

1. **Specs Directory.** If the repository has a directory conventionally used for specs (`specs/`,
   `prds/`, or similar), use it.
2. **Scratch Directory.** Otherwise, if an existing scratch directory is present at the repository
   root (`tmp/`, `temp/`, `scratch/`, or similar, especially if git-ignored), use it.
3. **Fallback.** Otherwise, use the repository root, or the current working directory if not in a
   repository.

Use `mktemp` or a system temp directory ONLY inside a script bundled with this skill.

### Filename

In the resolved directory, create a new directory whose name is at most four words describing the
spec in kebab-case. Create the file in this directory named `spec-<date>.md`, where `<date>` is the
output of `date +%F`.

### Splitting

The default SHOULD be a single file. If the spec grows very large (>1500 lines), extract
self-contained appendices (contract details, schemas, or acceptance criteria) into sibling files
that the primary spec references. Use `spec-<date>-<content>.md`. Do NOT pre-split into per-task
files.

## Self-Review

Before the final report, scan the draft and fix problems inline:

- Placeholders or TODOs.
- Vague requirements.
- Non-observable or non-binary acceptance criteria.
- Requirements without at least one acceptance criterion.
- Contradictions between sections.

The user will review the file conversationally.
