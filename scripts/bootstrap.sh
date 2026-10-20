#!/usr/bin/env bash
# Make this Fedora machine able to build Marquee (native deps + toolchain check).
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

"$root/scripts/fedora-deps.sh" "$@"

missing=()
for cmd in rustc cargo pkg-config node pnpm; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    missing+=("$cmd")
  fi
done

if [ "${#missing[@]}" -gt 0 ]; then
  echo "Missing commands: ${missing[*]}" >&2
  echo "Install Rust with rustup (https://rustup.rs), Node 22+, and pnpm (fnm or corepack enable pnpm)." >&2
  exit 1
fi

node_major="$(node -p "process.versions.node.split('.')[0]")"
if [ "$node_major" -lt 22 ]; then
  echo "Node 22+ required (found $(node -v))." >&2
  exit 1
fi

echo "Toolchain ready: $(rustc --version), $(node -v), pnpm $(pnpm -v)"
echo "Next: cd apps/desktop && pnpm install && pnpm tauri dev"
