#!/usr/bin/env bash
# Fedora packages needed to compile the Tauri window (webkit, dbus, …).
# Idempotent: skips sudo when everything is already installed.
# Official Tauri Fedora list plus dbus-devel (pkg-config dbus-1) which this
# workspace actually failed without. Does not install PCSX2, BIOS, or games.
set -euo pipefail

PACKAGES=(
  webkit2gtk4.1-devel
  dbus-devel
  openssl-devel
  curl
  file
  libappindicator-gtk3-devel
  librsvg2-devel
  libxdo-devel
  pkgconf-pkg-config
  gcc
  gcc-c++
  make
)

usage() {
  cat <<'EOF'
Usage: scripts/fedora-deps.sh [--dry-run]

Installs Fedora devel packages required by `cargo check -p marquee-desktop`
and `pnpm tauri dev`.
EOF
}

dry_run=0
case "${1:-}" in
  -h | --help)
    usage
    exit 0
    ;;
  --dry-run)
    dry_run=1
    ;;
  "")
    ;;
  *)
    usage >&2
    exit 2
    ;;
esac

if [ ! -r /etc/os-release ]; then
  echo "Cannot read /etc/os-release; this script is Fedora-only." >&2
  exit 1
fi
# shellcheck disable=SC1091
. /etc/os-release
if [ "${ID:-}" != "fedora" ]; then
  echo "Fedora required (found ID=${ID:-unknown}). Package names: ${PACKAGES[*]}" >&2
  exit 1
fi

missing=()
for pkg in "${PACKAGES[@]}"; do
  if ! rpm -q "$pkg" >/dev/null 2>&1; then
    missing+=("$pkg")
  fi
done

if [ "${#missing[@]}" -eq 0 ]; then
  echo "Fedora Tauri packages already installed."
  exit 0
fi

echo "Missing: ${missing[*]}"
if [ "$dry_run" -eq 1 ]; then
  echo "Would run: sudo dnf install -y ${missing[*]}"
  exit 0
fi

sudo dnf install -y "${missing[@]}"
echo "Installed ${#missing[@]} package(s). Next: cargo check -p marquee-desktop"
