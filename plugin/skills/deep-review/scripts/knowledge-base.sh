#!/usr/bin/env bash
set -euo pipefail

# The bracket ranges in toon_string must match ASCII letters only, whatever the user's locale.
export LC_ALL=C

FOLDERS=(. .claude .agents .codex .cursor .gemini .kiro .goose .windsurf .opencode scratch tmp temp)

usage() {
  echo "usage: bash knowledge-base.sh [init|index]" >&2
  exit 2
}

# Prints $1 with symbolic links resolved, also when $1 is missing. macOS may lack realpath and
# readlink -f.
resolve() {
  local parent
  if [ -d "$1" ]; then
    (cd "$1" && pwd -P)
  else
    parent="$(resolve "$(dirname "$1")")"
    printf '%s/%s' "${parent%/}" "$(basename "$1")"
  fi
}

# Prints the resolved path of the first .skald folder under $1 that is not the skald data
# directory, or fails when none exists.
search() {
  local base="$1" folder candidate
  for folder in "${FOLDERS[@]}"; do
    [ -d "$base/$folder/.skald" ] || continue
    candidate="$(cd "$base/$folder/.skald" && pwd -P)"
    if [ "$candidate" != "$data" ]; then
      printf '%s' "$candidate"
      return
    fi
  done
  return 1
}

# Prints the resolved root of the main checkout, or fails when the repository has none. git lists
# the main checkout first, and marks it bare when there is none.
main_checkout() {
  local records line path="" top
  records="$(git worktree list --porcelain 2>/dev/null)" || return 1
  while IFS= read -r line; do
    case "$line" in
      "") break ;;
      bare) return 1 ;;
      "worktree "*) path="${line#worktree }" ;;
    esac
  done <<<"$records"
  [ -n "$path" ] || return 1
  # In a submodule, git lists the submodule's git directory under .git/modules as the main
  # checkout, so the checkout's root comes from rev-parse.
  top="$(git -C "$path" rev-parse --show-toplevel 2>/dev/null)" || return 1
  (cd "$top" && pwd -P)
}

trim() {
  local value="$1"
  value="${value#"${value%%[![:blank:]]*}"}"
  printf '%s' "${value%"${value##*[![:blank:]]}"}"
}

# Prints the first problem in config file $1, or nothing.
config_error() {
  local line number=0 key value cr=$'\r'
  while IFS= read -r line || [ -n "$line" ]; do
    number=$((number + 1))
    line="$(trim "${line%"$cr"}")"
    [ -n "$line" ] || continue
    key=""
    case "$line" in
      *:*) key="$(trim "${line%%:*}")" ;;
    esac
    if [ "$key" != mode ]; then
      printf 'config.toon line %s is not a valid setting; the only key is mode' "$number"
      return
    fi
    value="$(trim "${line#*:}")"
    case "$value" in
      \"*\")
        value="${value#\"}"
        value="${value%\"}"
        ;;
    esac
    case "$value" in
      markdown) ;;
      skald)
        printf 'config.toon sets mode to skald, but skald mode is not available yet'
        return
        ;;
      *)
        printf 'config.toon sets mode to %s, but the only valid value is markdown' "$value"
        return
        ;;
    esac
  done <"$1"
}

toon_quoted() {
  printf '"%s"' "$(printf '%s' "$1" | sed 's/[\\"]/\\&/g')"
}

toon_string() {
  local value="$1" bare='^[A-Za-z_/][A-Za-z0-9_./-]*$'
  if [[ $value =~ $bare ]] && [ "$value" != true ] && [ "$value" != false ] && [ "$value" != null ]; then
    printf '%s' "$value"
  else
    toon_quoted "$value"
  fi
}

# Sets dir, exists, mode, ignored, and error from the files on disk.
inspect() {
  if dir="$(search "$root")" || { [ "$main" != "$root" ] && dir="$(search "$main")"; }; then
    exists=true
  else
    exists=false
    dir="$(resolve "$main/.skald")"
  fi

  error=""
  mode=markdown
  if [ "$dir" = "$data" ]; then
    error="$dir is the skald data directory, so it cannot hold a knowledge base"
  elif [ -f "$dir/config.toon" ]; then
    error="$(config_error "$dir/config.toon" 2>/dev/null)" || error="cannot read $dir/config.toon"
    [ -z "$error" ] || mode=""
  fi

  # The probe runs from the parent folder, because dir itself may be missing.
  ignored=false
  if [ "$in_git" = true ] && git -C "$(dirname "$dir")" check-ignore -q "$dir/config.toon" 2>/dev/null; then
    ignored=true
  fi
}

report() {
  printf 'knowledge_base:\n'
  printf '  dir: %s\n' "$(toon_string "$dir")"
  printf '  exists: %s\n' "$exists"
  if [ -n "$mode" ]; then
    printf '  mode: %s\n' "$mode"
  fi
  printf '  ignored: %s\n' "$ignored"
  if [ -f "$dir/work/index.toon" ]; then
    printf '  work_index: %s\n' "$(toon_string "$dir/work/index.toon")"
  fi
  if [ -n "$error" ]; then
    printf '  error: %s\n' "$(toon_quoted "$error")"
  fi
}

cannot_create() {
  # An earlier step may have created dir, so the report reads the disk again.
  inspect
  error="cannot create $1"
  report
  exit 1
}

create_folder() {
  mkdir "$1" 2>/dev/null || cannot_create "$1"
}

create_file() {
  { printf '%s\n' "$2" >"$1"; } 2>/dev/null || cannot_create "$1"
}

init() {
  if [ -n "$error" ]; then
    report
    exit 1
  fi
  # An existing folder without .gitignore is the user's choice to commit it.
  if [ "$exists" = false ]; then
    create_folder "$dir"
    create_file "$dir/.gitignore" "*"
  fi
  [ -e "$dir/config.toon" ] || create_file "$dir/config.toon" "mode: markdown"
  [ -d "$dir/work" ] || create_folder "$dir/work"
  inspect
}

# Prints "<seconds><tab><name>" for each folder in work folder $1, newest first.
folder_times() {
  local work="$1" format
  if stat -c %Y / >/dev/null 2>&1; then
    format=(-c '%Y %n')
  else
    format=(-f '%m %N')
  fi
  {
    find "$work" -mindepth 1 -maxdepth 1 -type d -exec stat "${format[@]}" {} + 2>/dev/null || true
    find "$work" -mindepth 2 -type f -exec stat "${format[@]}" {} + 2>/dev/null || true
  } | awk -v skip="$((${#work} + 1))" '
    {
      seconds = $1 + 0
      rest = substr($0, length($1) + 2 + skip)
      slash = index(rest, "/")
      if (slash == 0) {
        own[rest] = seconds
        next
      }
      name = substr(rest, 1, slash - 1)
      if (!(name in newest) || seconds > newest[name]) newest[name] = seconds
    }
    END {
      for (name in own) print ((name in newest) ? newest[name] : own[name]) "\t" name
    }
  ' | sort -t "$tab" -k1,1nr -k2
}

index() {
  local work="$dir/work" rows row total=0 listed=0 body="" header date_form updated temporary
  if [ -n "$error" ]; then
    report
    exit 1
  fi
  if [ "$exists" = false ]; then
    error="$dir is missing; run init first"
    report
    exit 1
  fi
  [ -d "$work" ] || create_folder "$work"

  date_form=bsd
  if date -d @0 +%F >/dev/null 2>&1; then
    date_form=gnu
  fi
  rows="$(folder_times "$work")"
  while IFS= read -r row; do
    [ -n "$row" ] || continue
    total=$((total + 1))
    [ "$listed" -lt 40 ] || continue
    listed=$((listed + 1))
    if [ "$date_form" = gnu ]; then
      updated="$(date -d "@${row%%"$tab"*}" +%F)"
    else
      updated="$(date -r "${row%%"$tab"*}" +%F)"
    fi
    body="$body  $(toon_string "${row#*"$tab"}"),$updated"$'\n'
  done <<<"$rows"
  header="work[$listed]{name,updated}:"
  [ "$listed" -gt 0 ] || header="work: []"

  # Readers never see a partial file, because the rename replaces index.toon in one step.
  temporary="$work/.index.toon.$$"
  if ! {
    printf 'total: %s\n%s\n%s' "$total" "$header" "$body" >"$temporary" &&
      mv -f "$temporary" "$work/index.toon"
  } 2>/dev/null; then
    rm -f "$temporary"
    cannot_create "$work/index.toon"
  fi
  inspect
}

command=""
case "$#:${1:-}" in
  0:) ;;
  1:init | 1:index) command="$1" ;;
  *) usage ;;
esac

tab=$'\t'

data=""
if [ -n "${SKALD_HOME:-}" ]; then
  data="$(resolve "$SKALD_HOME")"
elif [ -n "${HOME:-}" ]; then
  data="$(resolve "$HOME/.skald")"
fi

if top="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  in_git=true
  root="$(cd "$top" && pwd -P)"
else
  in_git=false
  root="$(pwd -P)"
fi

main="$root"
if [ "$in_git" = true ] && checkout="$(main_checkout)"; then
  main="$checkout"
fi

inspect
case "$command" in
  init) init ;;
  index) index ;;
esac
report
