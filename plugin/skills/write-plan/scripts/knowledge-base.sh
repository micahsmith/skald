#!/usr/bin/env bash
set -euo pipefail

# The bracket ranges in toon_string must match ASCII letters only, whatever the user's locale.
export LC_ALL=C

FOLDERS=(. .claude .agents .codex .cursor .gemini .kiro .goose .windsurf .opencode scratch tmp temp)
MAX_LISTED=40

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
    # A folder that cannot be entered is reported unresolved, so inspect can name it.
    candidate="$(cd "$base/$folder/.skald" 2>/dev/null && pwd -P)" || {
      printf '%s' "$base/$folder/.skald"
      return
    }
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

# Succeeds when $1 holds a character that a shell expands or that ends a quoted string: $, a
# backtick, ", \, or a control character. The agent pastes printed paths into shell commands.
has_unsafe() {
  case "$1" in
    *[[:cntrl:]]* | *[\$\`\"\\]*) return 0 ;;
  esac
  return 1
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
    if has_unsafe "$value"; then
      printf 'config.toon sets mode to a value with characters that are unsafe in a shell command'
      return
    fi
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

# toon_quoted_to and toon_string_to set the variable named by $1, so a loop can quote names
# without a subshell.
toon_quoted_to() {
  local escaped="$2" nl=$'\n' cr=$'\r' tab=$'\t'
  escaped="${escaped//\\/\\\\}"
  escaped="${escaped//\"/\\\"}"
  escaped="${escaped//$nl/\\n}"
  escaped="${escaped//$cr/\\r}"
  escaped="${escaped//$tab/\\t}"
  printf -v "$1" '"%s"' "$escaped"
}

toon_string_to() {
  local plain="$2" bare='^[A-Za-z_/][A-Za-z0-9_./-]*$'
  if [[ $plain =~ $bare ]] && [ "$plain" != true ] && [ "$plain" != false ] && [ "$plain" != null ]; then
    printf -v "$1" '%s' "$plain"
  else
    toon_quoted_to "$1" "$plain"
  fi
}

toon_quoted() {
  local quoted
  toon_quoted_to quoted "$1"
  printf '%s' "$quoted"
}

toon_string() {
  local text
  toon_string_to text "$1"
  printf '%s' "$text"
}

# Succeeds when resolved path $1 is $2 or lies under it, compared on whole path components.
within() {
  [ "$1" = "$2" ] && return 0
  case "$1" in
    "${2%/}"/*) return 0 ;;
  esac
  return 1
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
  unsafe=false
  if has_unsafe "$dir"; then
    # Nothing derived from the path may be printed, so dir is blanked.
    unsafe=true
    dir=""
    mode=""
    error="the knowledge base path holds characters that are unsafe in a shell command"
  elif [ "$dir" = "$data" ]; then
    error="the .skald folder here is the skald data directory, so it cannot hold a knowledge base; create a .claude/.skald folder in the folder that holds it, or run from a project folder"
  elif [ "$exists" = true ] && ! within "$dir" "$root" && ! within "$dir" "$main"; then
    # The path lies outside the repository and is not printed, as above.
    unsafe=true
    dir=""
    mode=""
    error="the knowledge base folder is outside the repository"
  elif [ "$exists" = true ] && [ ! -x "$dir" ]; then
    error="cannot open $dir"
  elif [ "$exists" = true ] && [ -n "$(find "$dir" -type l -print 2>/dev/null || true)" ]; then
    # A link inside the folder can send writes anywhere, also when it dangles.
    error="the knowledge base folder holds a symbolic link"
  elif [ -f "$dir/config.toon" ]; then
    error="$(config_error "$dir/config.toon" 2>/dev/null)" || error="cannot read $dir/config.toon"
    [ -z "$error" ] || mode=""
  fi

  # The probe runs from the parent folder, because dir itself may be missing.
  ignored=false
  if [ "$unsafe" = false ] && [ "$in_git" = true ] && git -C "$(dirname "$dir")" check-ignore -q "$dir/config.toon" 2>/dev/null; then
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
  if [ "$unsafe" = false ] && [ -f "$dir/work/index.toon" ]; then
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

# Prints "<seconds><tab><date><tab><name>" for each folder in work folder $1, newest first. The
# date is the local date of <seconds>. GNU stat prints three words for %y (date, time, zone), so
# the name follows four words there and two words on BSD.
folder_times() {
  local work="$1" format words nl=$'\n'
  if stat -c %Y / >/dev/null 2>&1; then
    format=(-c '%Y %y %n')
    words=4
  else
    format=(-t %F -f '%m %Sm %N')
    words=2
  fi
  {
    find "$work" -mindepth 1 -maxdepth 1 -type d ! -path "*$nl*" -exec stat "${format[@]}" {} + 2>/dev/null || true
    find "$work" -mindepth 2 -type f ! -path "*$nl*" -exec stat "${format[@]}" {} + 2>/dev/null || true
  } | awk -v skip="$((${#work} + 1))" -v words="$words" '
    {
      seconds = $1 + 0
      day = $2
      rest = $0
      for (i = 0; i < words; i++) sub(/^[^ ]+ /, "", rest)
      rest = substr(rest, skip + 1)
      slash = index(rest, "/")
      if (slash == 0) {
        own[rest] = seconds
        ownday[rest] = day
        next
      }
      name = substr(rest, 1, slash - 1)
      if (!(name in newest) || seconds > newest[name]) {
        newest[name] = seconds
        newday[name] = day
      }
    }
    END {
      for (name in own) {
        if (name in newest) print newest[name] "\t" newday[name] "\t" name
        else print own[name] "\t" ownday[name] "\t" name
      }
    }
  ' | sort -t "$tab" -k1,1nr -k3
}

index() {
  local work="$dir/work" rows row total=0 listed=0 body="" header updated name temporary
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

  rows="$(folder_times "$work")"
  while IFS= read -r row; do
    [ -n "$row" ] || continue
    has_unsafe "${row#*"$tab"*"$tab"}" && continue
    total=$((total + 1))
    [ "$listed" -lt "$MAX_LISTED" ] || continue
    listed=$((listed + 1))
    row="${row#*"$tab"}"
    updated="${row%%"$tab"*}"
    toon_string_to name "${row#*"$tab"}"
    body="$body  $name,$updated"$'\n'
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
