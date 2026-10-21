#!/usr/bin/env bash
# Make this Fedora machine able to build Marquee: Tauri devel packages, then a
# toolchain check. Idempotent; asks for sudo only when a package is missing.
# Usage: scripts/setup.sh [--dry-run]
set -euo pipefail

PACKAGES=(
  webkit2gtk4.1-devel dbus-devel openssl-devel curl file
  libappindicator-gtk3-devel librsvg2-devel libxdo-devel
  pkgconf-pkg-config gcc gcc-c++ make
)

case "${1:-}" in
  "" | --dry-run) ;;
  *) echo "Usage: scripts/setup.sh [--dry-run]" >&2; exit 2 ;;
esac

# shellcheck disable=SC1091
if [ ! -r /etc/os-release ] || ! . /etc/os-release || [ "${ID:-}" != "fedora" ]; then
  echo "Fedora only. Packages needed: ${PACKAGES[*]}" >&2
  exit 1
fi

missing=()
for pkg in "${PACKAGES[@]}"; do rpm -q "$pkg" >/dev/null 2>&1 || missing+=("$pkg"); done
if [ "${#missing[@]}" -gt 0 ]; then
  echo "Missing: ${missing[*]}"
  if [ "${1:-}" = "--dry-run" ]; then echo "Would run: sudo dnf install -y ${missing[*]}"; exit 0; fi
  sudo dnf install -y "${missing[@]}"
fi

tools=()
for cmd in rustc cargo node pnpm; do command -v "$cmd" >/dev/null 2>&1 || tools+=("$cmd"); done
if [ "${#tools[@]}" -gt 0 ]; then
  echo "Missing: ${tools[*]}. Install Rust with rustup (https://rustup.rs), Node 22+, and pnpm (corepack enable pnpm)." >&2
  exit 1
fi
if [ "$(node -p "process.versions.node.split('.')[0]")" -lt 22 ]; then
  echo "Node 22+ required (found $(node -v))." >&2
  exit 1
fi
echo "Ready: $(rustc --version), node $(node -v), pnpm $(pnpm -v). Next: pnpm install in apps/desktop, then scripts/run.sh desktop"
