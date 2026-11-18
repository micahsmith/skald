#!/usr/bin/env bash
set -euo pipefail

INPUT="$(cat)"

# Degrade gracefully if jq is missing
if ! command -v jq >/dev/null 2>&1; then
  printf 'Unknown | Unknown | None\n'
  exit 0
fi

IFS=$'\t' read -r MODEL SIZE PCT USED DIR <<EOF
$(printf '%s' "${INPUT}" | jq -r '
  [ (.model.display_name // "Claude"),
    (.context_window.context_window_size // 0),
    (.context_window.used_percentage // 0),
    (.context_window.total_input_tokens // 0),
    (.workspace.current_dir // ".")
  ] | @tsv')
EOF

# Strip prefix
MODEL="${MODEL#Claude }"

# Find maximum context window
MAX="$(awk -v s="${SIZE}" 'BEGIN{
  if (s>=1000000){ v=s/1000000; if (v==int(v)) printf "%dM", v; else printf "%.1fM", v }
  else if (s>0)  { printf "%dK", s/1000 }
  else           { printf "?" }
}')"

# Find percent used and total tokens in context
PCT="$(awk -v p="${PCT}" 'BEGIN{ printf "%d", p+0.5 }')"
USED_K="$(awk -v t="${USED}" 'BEGIN{ printf "%d", (t+500)/1000 }')"

# Find git branch name
BRANCH="$(git -C "${DIR}" rev-parse --abbrev-ref HEAD 2>/dev/null || true)"
[ -z "${BRANCH}" ] && BRANCH="None"

printf '%s (%s) | %s%% (%sK) | %s\n' "${MODEL}" "${MAX}" "${PCT}" "${USED_K}" "${BRANCH}"
