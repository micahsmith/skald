#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CONFIG_DIR="${CLAUDE_CONFIG_DIR:-$HOME/.claude}"
SETTINGS="${CONFIG_DIR}/settings.json"
DEST="${CONFIG_DIR}/bootstrap-status-line.sh"

# Preconditions
command -v jq >/dev/null 2>&1 || { echo "error: jq is required; install it and re-run." >&2; exit 1; }

mkdir -p "${CONFIG_DIR}"

# Install the status line script
cp "${SCRIPT_DIR}/status-line.sh" "${DEST}"
chmod +x "${DEST}"

# Backup settings.json (will not create backup if it already exists)
[ -f "${SETTINGS}" ] || printf '{}\n' > "${SETTINGS}"
[ -f "${SETTINGS}.bak" ] || cp "${SETTINGS}" "${SETTINGS}.bak"

# Merge changes into settings.json
JQ_FILTER="$(cat <<'EOF'
  .statusLine = { type: "command", command: $cmd }
  | .permissions = (.permissions // {})
  | .permissions.deny = ((.permissions.deny // []) as $d
      | if ($d | index("AskUserQuestion")) then $d else $d + ["AskUserQuestion"] end)
EOF
)"

TMP="$(mktemp)"
jq --arg cmd "${DEST}" "${JQ_FILTER}" "${SETTINGS}" > "${TMP}"
mv "${TMP}" "${SETTINGS}"

echo "Bootstrapped Claude config at ${CONFIG_DIR}"
echo "  - statusLine        -> ${DEST}"
echo "  - permissions.deny  += AskUserQuestion"
echo "  - backup            -> ${SETTINGS}.bak"
