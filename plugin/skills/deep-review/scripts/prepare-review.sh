#!/usr/bin/env bash
set -euo pipefail

# Usage: prepare-review.sh [--no-fetch] [TARGET] [BASE]
#   TARGET     Branch/ref under review. Defaults to the current branch (HEAD).
#   BASE       Branch/ref to compare against. Defaults to the first existing
#              of: main, master (local, then origin/*).
#   --no-fetch Skip update of remote-tracking refs. The script by default will
#              fetch first to ensure freshness.
#
# Output is a key=value summary of the results.

FETCH=1
positional=()
for a in "$@"; do
  case "$a" in
    --no-fetch) FETCH=0 ;;
    --fetch) FETCH=1 ;;
    *) positional+=("$a") ;;
  esac
done

TARGET="${positional[0]:-HEAD}"
BASE_ARG="${positional[1]:-}"

git rev-parse --is-inside-work-tree >/dev/null 2>&1 || {
  echo "ERROR: not inside a git repository. Run the script from the repository under review." >&2; exit 1; }

REPO_ROOT="$(git rev-parse --show-toplevel)"

if [ "$FETCH" -eq 1 ]; then
  git fetch --quiet --all --prune ||
    echo "WARNING: fetch failed, so local refs may be stale. Pass --no-fetch to skip the fetch." >&2
fi

# Resolve BASE from arg if present; otherwise `main`; otherwise `master`
resolve_base() {
  if [ -n "$BASE_ARG" ]; then echo "$BASE_ARG"; return; fi
  for b in main master origin/main origin/master; do
    if git rev-parse --verify --quiet "$b" >/dev/null; then echo "$b"; return; fi
  done
  echo ""
}

BASE="$(resolve_base)"
[ -n "$BASE" ] || {
  echo "ERROR: no main or master branch found. Pass BASE as the second argument." >&2; exit 1; }
git rev-parse --verify --quiet "$BASE" >/dev/null || {
  echo "ERROR: base ref '$BASE' not found. Check the name, or fetch the branch." >&2; exit 1; }
git rev-parse --verify --quiet "$TARGET" >/dev/null || {
  echo "ERROR: target ref '$TARGET' not found. Check the name, or fetch the branch." >&2; exit 1; }

MERGE_BASE="$(git merge-base "$BASE" "$TARGET")" || {
  echo "ERROR: '$BASE' and '$TARGET' share no common ancestor. Pass a BASE that shares history with TARGET." >&2; exit 1; }

TARGET_SHA="$(git rev-parse "$TARGET")"
HEAD_SHA="$(git rev-parse HEAD)"
CURRENT_BRANCH="$(git rev-parse --abbrev-ref HEAD)"

# Review in place only when the checked-out commit is the target and the
# working tree is clean. Otherwise isolate in a detached worktree so the
# current branch and tree are not affected.
CLEANUP=""
if [ "$TARGET_SHA" = "$HEAD_SHA" ] && [ -z "$(git status --porcelain)" ]; then
  MODE="inplace"
  WORKDIR="$REPO_ROOT"
else
  MODE="worktree"
  WORKDIR="$(mktemp -d "${TMPDIR:-/tmp}/deep-review-XXXXXX")"
  git worktree add --quiet --detach "$WORKDIR" "$TARGET"
  CLEANUP="git -C \"$REPO_ROOT\" worktree remove --force \"$WORKDIR\""
fi

echo "=== deep-review prep ==="
echo "MODE=$MODE"
echo "WORKDIR=$WORKDIR"
echo "BASE=$BASE"
echo "TARGET=$TARGET"
echo "MERGE_BASE=$MERGE_BASE"
echo "TARGET_SHA=$TARGET_SHA"
echo "CURRENT_BRANCH=$CURRENT_BRANCH"
[ -n "$CLEANUP" ] && echo "CLEANUP=$CLEANUP"
echo "DIFF_CMD=git -C \"$WORKDIR\" diff $MERGE_BASE $TARGET_SHA"
echo "=== diffstat ==="
git diff --stat "$MERGE_BASE" "$TARGET"
echo "=== changed files (name-status) ==="
git diff --name-status "$MERGE_BASE" "$TARGET"
echo "=== end prep ==="
