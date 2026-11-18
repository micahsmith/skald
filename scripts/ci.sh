#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

# The offline test needs `unshare`, which exists only on Linux.
if [ "$(uname -s)" = Linux ]; then
  export SKALD_REQUIRE_OFFLINE_TEST="${SKALD_REQUIRE_OFFLINE_TEST:-1}"
fi

cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
bash plugin/hooks/emit-guides.sh session | grep -q "Code Comment Style Guide"
