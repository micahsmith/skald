#!/usr/bin/env bash
set -euo pipefail

# Lands the uncommitted changes of one git worktree onto another working tree, file by file,
# without touching either index. Commands:
#   list                                  Show every worktree and whether it is clean.
#   plan   <worktree> [<target>]          Show what apply would do. Changes nothing.
#   apply  <worktree> [<target>]          Land the changes. Refuses if any file conflicts.
#   check  <worktree> [<target>]          Confirm that the target holds every change.
#   remove <worktree> [<target>] [--discard] [--delete-branch]
#                                         Remove the worktree if no work would be lost.
# <target> defaults to the working tree of the current directory.

usage() {
  sed -n '4,12p' "$0" | sed 's/^# //' >&2
  exit 2
}

die() {
  echo "ERROR: $*" >&2
  exit 1
}

toplevel() {
  git -C "$1" rev-parse --show-toplevel 2>/dev/null ||
    die "'$1' is not inside a git working tree. Pass the path of a worktree."
}

CMD="${1:-}"
[ -n "$CMD" ] || usage
shift

DISCARD=0
DELETE_BRANCH=0
ARGS=()
for a in "$@"; do
  case "$a" in
    --discard) DISCARD=1 ;;
    --delete-branch) DELETE_BRANCH=1 ;;
    -*) usage ;;
    *) ARGS+=("$a") ;;
  esac
done

TMP="$(mktemp -d "${TMPDIR:-/tmp}/land-worktree-XXXXXX")"
trap 'rm -rf "$TMP"' EXIT

# Ignored paths that build tools and package managers recreate. remove deletes them without asking.
BUILD_OUTPUT='(^|/)(node_modules|build|dist|out|target|bin|\.gradle|\.kotlin|\.next|\.turbo|\.cache|\.npm|\.pnpm-store|coverage|__pycache__|\.venv|venv|\.pytest_cache|\.mypy_cache|\.quarkus|cdk\.out|\.idea)(/|$)|(^|/)(\.DS_Store|next-env\.d\.ts)$|\.(pyc|tsbuildinfo|iml)$'

# Prints the ignored paths in worktree $1 that are not build output. A wholly ignored directory
# prints as one line.
ignored_work() {
  git -C "$1" ls-files -z --others --ignored --exclude-standard --directory |
    tr '\0' '\n' | grep -v -E "$BUILD_OUTPUT" || true
}

list_worktrees() {
  local main="" path="" head="" branch="" key val
  emit() {
    [ -n "$path" ] || return 0
    local state="clean" dirty ignored lost label
    if [ ! -d "$path" ]; then
      printf '%s\t-\tuncommitted=-\tignored=-\tunbranched_commits=-\tmissing\n' "$path"
      return 0
    fi
    dirty="$(git -C "$path" status --porcelain --untracked-files=all | wc -l | tr -d ' ')"
    ignored=-
    [ "$path" = "$main" ] || ignored="$(ignored_work "$path" | wc -l | tr -d ' ')"
    lost=0
    if [ -z "$branch" ]; then
      lost="$(git -C "$path" rev-list --count HEAD --not --branches --remotes --tags)"
    fi
    [ "$dirty" = 0 ] && [ "$ignored" = 0 ] && [ "$lost" = 0 ] || state="dirty"
    label="${branch:-detached@${head:0:9}}"
    if [ "$path" = "$main" ]; then state="main"; fi
    printf '%s\t%s\tuncommitted=%s\tignored=%s\tunbranched_commits=%s\t%s\n' \
      "$path" "$label" "$dirty" "$ignored" "$lost" "$state"
  }
  while IFS=' ' read -r key val; do
    case "$key" in
      worktree) emit; path="$val"; head=""; branch=""; [ -n "$main" ] || main="$val" ;;
      HEAD) head="$val" ;;
      branch) branch="${val#refs/heads/}" ;;
    esac
  done < <(git worktree list --porcelain)
  emit
}

if [ "$CMD" = list ]; then
  git rev-parse --git-dir >/dev/null 2>&1 || die "not inside a git repository."
  list_worktrees
  exit 0
fi

[ "${#ARGS[@]}" -ge 1 ] && [ "${#ARGS[@]}" -le 2 ] || usage
W="$(toplevel "${ARGS[0]}")"
T="$(toplevel "${ARGS[1]:-.}")"
[ "$W" != "$T" ] || die "the worktree and the target are the same tree ($W). Pass the target tree as the second argument."
[ "$(git -C "$W" rev-parse --path-format=absolute --git-common-dir)" = \
  "$(git -C "$T" rev-parse --path-format=absolute --git-common-dir)" ] ||
  die "'$W' and '$T' belong to different repositories."

# Prints "<status>\0<path>\0" for each path that differs between the worktree and its HEAD.
changes() {
  git -C "$W" diff --name-status -z --no-renames HEAD
  git -C "$W" ls-files -z --others --exclude-standard |
    while IFS= read -r -d '' p; do printf 'A\0%s\0' "$p"; done
}

# Writes the worktree's HEAD version of $1 to $TMP/base. Fails when HEAD lacks the path.
base_of() {
  git -C "$W" cat-file -e "HEAD:$1" 2>/dev/null || { : > "$TMP/base"; return 1; }
  git -C "$W" show "HEAD:$1" > "$TMP/base"
}

exists() { [ -e "$1" ] || [ -L "$1" ]; }

xbit() { if [ -x "$1" ]; then echo 1; else echo 0; fi; }

# Two files are the same when their content and their executable bit match.
same() { cmp -s "$1" "$2" && [ "$(xbit "$1")" = "$(xbit "$2")" ]; }

# Prints the action for path $1: SKIP, COPY, DELETE, MERGE, or CONFLICT plus a reason.
classify() {
  local p="$1" w="$W/$1" t="$T/$1" has_base=1 rc
  base_of "$p" || has_base=0
  if ! exists "$w"; then
    if ! exists "$t"; then echo SKIP
    elif [ "$has_base" = 1 ] && cmp -s "$t" "$TMP/base"; then echo DELETE
    else echo "CONFLICT the worktree deleted the file, and the target changed it"
    fi
  elif exists "$t" && same "$w" "$t"; then echo SKIP
  elif ! exists "$t"; then
    if [ "$has_base" = 1 ]; then echo "CONFLICT the worktree changed the file, and the target deleted it"
    else echo COPY
    fi
  elif [ "$has_base" = 1 ] && cmp -s "$t" "$TMP/base"; then echo COPY
  else
    # merge-file exits with the conflict count, or 255 when it cannot read or merge a file.
    rc=0
    git merge-file -p -q -L target -L base -L worktree "$t" "$TMP/base" "$w" > "$TMP/merged" 2>/dev/null ||
      rc=$?
    case "$rc" in
      0) echo MERGE ;;
      255) echo "CONFLICT both trees changed the file, and git cannot merge a binary or unreadable file" ;;
      *) echo "CONFLICT both trees changed the same lines" ;;
    esac
  fi
}

# Prints PRESENT or MISSING for path $1: whether the target holds the worktree's change.
presence() {
  local p="$1" w="$W/$1" t="$T/$1"
  base_of "$p" || true
  if ! exists "$w"; then
    exists "$t" && echo MISSING || echo PRESENT
  elif ! exists "$t"; then echo MISSING
  elif same "$w" "$t"; then echo PRESENT
  elif [ "$(xbit "$w")" != "$(xbit "$t")" ]; then echo MISSING
  elif git merge-file -p -q "$t" "$TMP/base" "$w" > "$TMP/merged" 2>/dev/null &&
    cmp -s "$TMP/merged" "$t"; then echo PRESENT
  else echo MISSING
  fi
}

# Reports commits in the worktree's HEAD that the target's HEAD lacks. Apply lands none of them.
unlanded_commits() {
  local w_head label n
  w_head="$(git -C "$W" rev-parse HEAD)"
  label="$(git -C "$W" symbolic-ref -q --short HEAD || echo "detached HEAD")"
  n="$(git -C "$T" rev-list --count "HEAD..$w_head")"
  [ "$n" = 0 ] && return 0
  echo "UNLANDED_COMMITS=$n ($label). Apply lands only uncommitted changes. The newest commits:"
  git -C "$T" log --oneline -10 "HEAD..$w_head"
}

plan() {
  local status p action conflicts=0
  : > "$TMP/plan"
  while IFS= read -r -d '' status && IFS= read -r -d '' p; do
    action="$(classify "$p")"
    printf '%s\t%s\n' "${action%% *}" "$p" >> "$TMP/plan"
    case "$action" in
      CONFLICT*)
        conflicts=$((conflicts + 1))
        printf 'CONFLICT\t%s\t(%s)\n' "$p" "${action#CONFLICT }"
        case "$action" in
          *"same lines") awk '/^<<<<<<< /{p=1} p{print "    " $0} /^>>>>>>> /{p=0}' "$TMP/merged" ;;
        esac
        ;;
      *) printf '%s\t%s\n' "$action" "$p" ;;
    esac
  done < <(changes)
  echo "WORKTREE=$W"
  echo "TARGET=$T"
  for a in COPY MERGE DELETE SKIP CONFLICT; do
    echo "$a=$(grep -c "^$a	" "$TMP/plan" || true)"
  done
  unlanded_commits
  [ "$conflicts" = 0 ]
}

check() {
  local status p result missing=0
  while IFS= read -r -d '' status && IFS= read -r -d '' p; do
    result="$(presence "$p")"
    [ "$result" = PRESENT ] || missing=$((missing + 1))
    printf '%s\t%s\n' "$result" "$p"
  done < <(changes)
  echo "MISSING=$missing"
  [ "$missing" = 0 ]
}

apply() {
  local action p
  while IFS=$'\t' read -r action p; do
    case "$action" in
      COPY) mkdir -p "$(dirname "$T/$p")"; rm -f "$T/$p"; cp -p "$W/$p" "$T/$p" ;;
      DELETE) rm -f "$T/$p" ;;
      MERGE)
        base_of "$p" || true
        git merge-file -p -q "$T/$p" "$TMP/base" "$W/$p" > "$TMP/merged" ||
          die "merging $p failed after the plan found no conflict. $T/$p is unchanged."
        cat "$TMP/merged" > "$T/$p"
        if [ -x "$W/$p" ]; then chmod +x "$T/$p"; else chmod -x "$T/$p"; fi
        ;;
    esac
  done < "$TMP/plan"
}

case "$CMD" in
  plan)
    plan || { echo "Conflicts found. Nothing was changed."; exit 3; } ;;
  apply)
    plan > "$TMP/plan-output" || {
      cat "$TMP/plan-output"
      echo "Conflicts found. Nothing was changed."
      exit 3
    }
    cat "$TMP/plan-output"
    apply
    echo "=== check ==="
    check || die "the target lacks changes after apply. Run check to see which."
    ;;
  check)
    check || exit 4 ;;
  remove)
    branch="$(git -C "$W" symbolic-ref -q --short HEAD || true)"
    if [ "$DISCARD" = 0 ]; then
      lose=0
      if [ -z "$branch" ]; then
        lost="$(git -C "$W" rev-list --count HEAD --not --branches --remotes --tags)"
        if [ "$lost" != 0 ]; then
          echo "UNBRANCHED_COMMITS=$lost: no branch holds these commits of the detached HEAD." >&2
          lose=1
        fi
      fi
      if ! check > "$TMP/check"; then
        echo "Changes that the target lacks:" >&2
        grep '^MISSING	' "$TMP/check" >&2
        lose=1
      fi
      ignored_work "$W" > "$TMP/ignored"
      if [ -s "$TMP/ignored" ]; then
        echo "Ignored files that are not build output:" >&2
        sed 's/^/IGNORED	/' "$TMP/ignored" >&2
        lose=1
      fi
      [ "$lose" = 0 ] ||
        die "removing the worktree would lose the items above. Land or move them first, or pass --discard if the user confirms."
    fi
    git -C "$T" worktree remove --force "$W"
    echo "REMOVED=$W"
    if [ "$DELETE_BRANCH" = 1 ] && [ -n "$branch" ]; then
      if git -C "$T" branch -d "$branch" >/dev/null 2>&1; then
        echo "DELETED_BRANCH=$branch"
      else
        echo "KEPT_BRANCH=$branch: the target's branch lacks some of its commits. Delete it with 'git branch -D $branch' only if the user confirms."
      fi
    fi
    ;;
  *) usage ;;
esac
