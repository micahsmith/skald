---
name: write-skill
description: Create a new skill. Provides guidance on progressive disclosure and bundled resources.
---

## Process

1. **Gather Requirements.**
   - Intention: what task is the user solving with the skill?
   - Scope: what are the core use cases?
   - You MAY skip this step if the conversation already contains requirements.

2. **Write Skill.**
   - Draft SKILL.md and any needed additional files.
   - Use progressive disclosure. Keep SKILL.md short and move detail into files that the agent reads
     only when it needs them.
   - If the SKILL.md exceeds 500 lines, extract content by domain into reference files. For
     instance, if a large part of the SKILL.md covers code standards, move that content into a
     STANDARDS.md file.
   - Keep simple scripts (<= 12 lines) inline. Write longer scripts to the `scripts` directory.
   - Use a script when you need deterministic behavior.
   - Use simple language. Avoid jargon unless it is needed for clarity.
   - Write more ONLY when extra words add clarity.

3. **Review.**
   - Provide the user with your draft and get feedback.
   - Iterate between drafting and feedback until the user gives final approval.

## Structure

### Directory

Use the following for the directory structure of the SKILL.md and any supporting files:

```text
skill-name/
├── EXAMPLES.md     # Optional
├── <REFERENCE>.md  # Optional
├── SKILL.md        # Required
└── scripts/        # Optional
    └── <SCRIPT>.sh
```

### Description

The description MUST make plain what the skill does and when to use it. Note any special triggers.

You MUST conform to these criteria:

- Use at most 1024 characters.
- Use at most 3 sentences.
- Use simple language.
- Be concise.

### Scripts

Scripts MUST be used when deterministic and repeatable operations are needed in the course of
executing the skill. Assume scripts will be run across multiple platforms.
