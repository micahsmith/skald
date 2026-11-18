#!/usr/bin/env bash
# Usage: check-skill.sh <skill-dir>...
# Checks the rules in GUIDELINES.md that a command can decide. Prints one line
# per violation. Exits 0 when it finds none, 1 when it finds any, and 2 on a
# usage error.
set -uo pipefail

[ "$#" -gt 0 ] || {
  echo "ERROR: no skill directory given. Usage: check-skill.sh <skill-dir>..." >&2; exit 2; }

# Prose rules exclude code and quoted text, so the scan blanks them out first.
# A blank line ends any open quote, so a stray quote mark hides one paragraph
# at most.
prose='
/^[ \t]*```/ { fence = !fence; next }
fence { next }
/^[ \t]*$/ { q = 0; tick = 0; next }
{
  s = ""
  for (i = 1; i <= length($0); i++) {
    c = substr($0, i, 1)
    if (c == "`" && !q) tick = !tick
    else if (c == "\"" && !tick) q = !q
    else if (!q && !tick) { s = s c; continue }
    s = s " "
  }
  t = " " s " "
  gsub(/(MUST|SHOULD) NOT/, "", t)
  if (t ~ /[^A-Za-z0-9_](ONLY|NOT|NEVER|ALWAYS|CRITICAL|IMPORTANT|ONE|MIGHT|ALL|EVERY|NO)[^A-Za-z0-9_]/) {
    print f ":" FNR ": capitalized emphasis: " $0; bad = 1
  }
  if (tolower(s) ~ /as needed|if appropriate|where relevant|be thorough|ensure quality|if needed|best attempt|whenever available/) {
    print f ":" FNR ": judgment phrase: " $0; bad = 1
  }
}
END { exit bad }
'

fail=0
v() { echo "$1: $2"; fail=1; }

for dir in "$@"; do
  dir="${dir%/}"
  [ -d "$dir" ] || {
    echo "ERROR: '$dir' is not a directory. Pass a skill directory, such as skills/write-skill." >&2
    exit 2; }
  skill="$dir/SKILL.md"
  [ -f "$skill" ] || { v "$dir" "no SKILL.md"; continue; }

  n=$(awk 'END { print NR }' "$skill")
  [ "$n" -lt 500 ] || v "$skill" "$n lines (limit 499)"

  desc=$(awk '/^---$/ { n++; next } n == 1 && sub(/^description:[ ]*/, "")' "$skill")
  [ -n "$desc" ] || v "$skill" "no description"
  [ "${#desc}" -le 1024 ] || v "$skill" "description has ${#desc} characters (limit 1024)"
  sentences=$(printf '%s\n' "$desc" | awk '{ s = $0 " "; print gsub(/[.!?] /, "", s) }')
  [ "$sentences" -le 3 ] || v "$skill" "description has $sentences sentences (limit 3)"
  if ! grep -q '^disable-model-invocation: true' "$skill"; then
    printf '%s\n' "$desc" | grep -q -i -E '(^|[.!?] )use ' ||
      v "$skill" "description has no sentence that starts with \"Use\" to say when to use the skill"
  fi

  while IFS= read -r f; do
    grep -q -F "${f#"$dir"/}" "$skill" || v "$f" "not referenced from SKILL.md"
    case "$f" in
      *.md)
        grep -n -E '[]][(][^):#]*[.]md[)#]' "$f" | sed "s|^|$f: links a sibling file: |" && fail=1
        lines=$(awk 'END { print NR }' "$f")
        if [ "$lines" -gt 100 ] && ! head -n 6 "$f" | grep -q -E '^(Contents:|## Contents)'; then
          v "$f" "$lines lines and no table of contents in the first 6 lines"
        fi ;;
    esac
  done < <(find "$dir" -type f ! -name SKILL.md | sort)

  while IFS= read -r f; do
    awk -v f="$f" "$prose" "$f" || fail=1
  done < <(find "$dir" -type f -name '*.md' | sort)
done
exit "$fail"
