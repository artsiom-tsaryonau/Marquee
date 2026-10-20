#!/usr/bin/env bash
# Local checks before every push (there is no hosted CI).
set -euo pipefail
cd "$(dirname "$0")/.."

if [ -f Cargo.toml ]; then
  cargo fmt --all --check
  cargo clippy --workspace --exclude marquee-desktop --all-targets --locked -- -D warnings
  cargo test --workspace --exclude marquee-desktop --locked
  if ! pkg-config --exists webkit2gtk-4.1 || ! pkg-config --exists dbus-1; then
    echo "BLOCKED: Tauri native deps (webkit2gtk-4.1 / dbus-1). On Fedora: ./scripts/fedora-deps.sh" >&2
  elif ! cargo check -p marquee-desktop --locked; then
    echo "FAILED: cargo check -p marquee-desktop. On Fedora: ./scripts/fedora-deps.sh" >&2
    exit 1
  fi
fi

cd apps/desktop
pnpm install --frozen-lockfile
pnpm check
pnpm test
pnpm build
