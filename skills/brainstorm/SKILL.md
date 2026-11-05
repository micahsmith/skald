---
name: brainstorm
description: Interview the user to reach alignment on design and tasks before any work begins. Use for ideation or creative work or when the user wants to brainstorm or solidify requirements.
---

Interview the user relentlessly until you reach **total** alignment. Build a complete and
unambiguous picture of what to build and what the requirements are. Leave no assumption unexamined.
The conversation encodes this shared understanding.

## Pre-Interview

Explore before the interview. Read relevant code, documentation, and recent commits. Try to answer
questions from the codebase before asking the user.

## Interview

- **Ask one question at a time:**
  - If a topic can be decomposed, it MUST be decomposed.
- **State options:**
  - For each topic, provide a list of concrete options available when you can.
  - You MAY ask open-ended questions when the space is genuinely open.
  - You MAY use `AskUserQuestion`.
  - If the topic under discussion implies trade-offs, explicitly state the trade-offs.
  - You MAY recommend an answer.
- **Work through decisions in order:**
  - Address decisions that have broader implications first and keep minor decisions to the end.
  - Only resolve decisions that change the outcome. Be relentless about alignment, but stop short of
    exhaustive interrogation.
- **Let the user end the interview:**
  - You MAY inform the user when you think the interview is done.
  - The user MAY end the interview at any time.
  - Once the interview is over, provide an overview of where things stand. Note anything that
    remains uncertain or unsettled.

## Knowing When You Are Done

You are aligned when you can state four things and the user agrees with all of them:

- purpose
- approach
- scope
- success criteria

Before you conclude, surface any decision that is still open. For each one, propose 2-3 options with
their trade-offs and your recommendation. The user makes the choice; do NOT assume it.

When you believe you are there, play back the full picture in your own words and ask the user to
confirm or correct it. Alignment is their agreement, not your assumption.

## What Happens Next

Nothing is mandated. The shared understanding is the deliverable. That understanding may become a
written spec, a design doc, an implementation plan, or direct work. It may also stay as agreed
context. Ask the user where they want to take it. Do NOT assume a structure, a file location, or a
next skill.
