#!/usr/bin/env bash
# Local checks before every push (there is no hosted CI).
set -euo pipefail
cd "$(dirname "$0")/.."

if [ -f Cargo.toml ]; then
  cargo fmt --check
  cargo clippy --all-targets -- -D warnings
  cargo test
fi

cd apps/desktop
pnpm install --frozen-lockfile
pnpm check
pnpm build
