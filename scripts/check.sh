#!/usr/bin/env bash
# Local checks before every push (there is no hosted CI).
set -euo pipefail
cd "$(dirname "$0")/.."

if [ -f Cargo.toml ]; then
  cargo fmt --all --check
  cargo clippy --workspace --exclude marquee-desktop --all-targets --locked -- -D warnings
  cargo test --workspace --exclude marquee-desktop --locked
  if ! cargo check -p marquee-desktop --locked; then
    echo "BLOCKED: cargo check -p marquee-desktop (install webkit2gtk4.1-devel, dbus-devel, and other Tauri Fedora deps to close this gate)." >&2
  fi
fi

cd apps/desktop
pnpm install --frozen-lockfile
pnpm check
pnpm test
pnpm build
