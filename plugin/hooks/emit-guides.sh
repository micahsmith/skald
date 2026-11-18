#!/usr/bin/env bash
set -euo pipefail

# SessionStart adds plain stdout to the session. SubagentStart ignores plain stdout
# and reads context only from hookSpecificOutput.additionalContext, so subagent
# mode wraps the guides in JSON.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
MODE="${1:-session}"

GUIDES=(
  "how-to-write-comments.md"
  "how-to-write-prose.md"
)

TEXT=""
for guide in "${GUIDES[@]}"; do
  GUIDE_PATH="${SCRIPT_DIR}/${guide}"
  [ -f "${GUIDE_PATH}" ] || continue
  TEXT="${TEXT}$(cat "${GUIDE_PATH}")"$'\n\n'
done

[ -n "${TEXT}" ] || exit 0

if [ "${MODE}" != "subagent" ]; then
  printf '%s' "${TEXT}"
  exit 0
fi

PAYLOAD="These are the writing guides that apply to every agent."$'\n\n'"${TEXT}"

if command -v jq >/dev/null 2>&1; then
  jq -n --arg ctx "${PAYLOAD}" \
    '{hookSpecificOutput: {hookEventName: "SubagentStart", additionalContext: $ctx}}'
elif command -v python3 >/dev/null 2>&1; then
  printf '%s' "${PAYLOAD}" | python3 -c 'import json, sys; print(json.dumps({"hookSpecificOutput": {"hookEventName": "SubagentStart", "additionalContext": sys.stdin.read()}}))'
else
  exit 0
fi

exit 0
