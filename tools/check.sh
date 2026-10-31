#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
: "${HIBANA_RESOURCE_GUARD:?set HIBANA_RESOURCE_GUARD to the shared descendant-memory guard}"
exec "$HIBANA_RESOURCE_GUARD" bash -euo pipefail -c '
  export CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0
  mkdir -p target
  rustc +stable --edition=2024 -D warnings tools/check.rs -o target/check-runtime
  exec target/check-runtime
'
