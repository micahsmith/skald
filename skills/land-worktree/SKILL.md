---
name: land-worktree
description: Move the uncommitted changes of a git worktree onto another working tree without staging or committing, confirm that the target holds every change, and remove the worktree on request. Also lists worktrees and removes the clean ones. Use when the user asks to merge, land, or move a worktree's changes onto a branch, to remove or drop a worktree, or to list or prune worktrees.
---

Land a worktree's changes with `scripts/land-worktree.sh`. The script merges one file at a time and
never touches the index. In the commands below, `<skill-dir>` is the directory of this file. Run
each command from inside the target tree.

## Rules

- Change files in either tree only with the script. The one exception is step 2, where you write
  the content that the user chose.
- You MUST NOT write conflict markers into any file. Resolve a conflict as step 2 says.
- Remove a worktree only with the script's `remove` command. It refuses when removal would lose
  work: changes that the target lacks, commits that no branch holds, or ignored files that are not
  build output. It lists each such item. Pass `--discard` only after the user has seen the items
  and has said to drop them.
- Do not list ignored files yourself, because build output can hold thousands of them.

## Target and Worktree

The target is the working tree that the session started in. "My branch" and "the current branch"
mean this tree. Pass its top-level path as the `<target>` argument, because the shell's current
directory may have moved. If the user names another branch, run `list`. The target is the worktree
that `list` shows on that branch.

The worktree is the one whose path, folder name, or branch the user names. If the user names none,
run `list`. Use the only worktree besides the target. If there are several, ask the user which one.

## Land and Remove

1. Run `bash <skill-dir>/scripts/land-worktree.sh apply <worktree> <target>`.
   - Exit 0: the target holds every change. If the output has an `UNLANDED_COMMITS` line, tell the
     user that `apply` did not land those commits. `remove` keeps them. Go to step 3.
   - Exit 3: go to step 2.
   - Any other exit: report the script's error message and stop.
2. `apply` changed nothing, because the paths marked `CONFLICT` hold changes that clash. For each
   such path, show the user the clashing lines that `apply` printed under it. Ask which to keep, or
   what text to write instead. Write the user's choice into both copies of the file. Then run step
   1 again.
3. If the user asked to remove the worktree, run
   `bash <skill-dir>/scripts/land-worktree.sh remove <worktree> <target>`. Add `--delete-branch`
   only when the user asks to delete the worktree's branch too.

## List and Prune

1. Run `bash <skill-dir>/scripts/land-worktree.sh list`. It prints one line per worktree: the
   path, the branch, the count of uncommitted files, the count of ignored files that are not build
   output, the count of commits that no branch holds, and a state. The state is `main` (the main
   worktree, on any branch), `clean`, `dirty`, or `missing`.
2. To prune, run `bash <skill-dir>/scripts/land-worktree.sh remove <worktree> <target>` for each
   worktree whose state is `clean`, except the target. Keep each branch unless the user asks to
   delete it. If any state is `missing`, run `git worktree prune`, which clears the records of
   worktrees whose folders are gone.
3. Report each `dirty` worktree with its three counts.
