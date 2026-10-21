#!/usr/bin/env bash
# One way to start Marquee:
#   scripts/run.sh desktop   the app window only (no web port)
#   scripts/run.sh web       the web UI only, no window (builds the UI first)
#   scripts/run.sh dev       web API + Vite with hot reload, at http://localhost:1420
#   scripts/run.sh both      the app window, also served to a browser
# Extra arguments go to the underlying command.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
mode="${1:-}"
shift || true

# WebKitGTK on NVIDIA Wayland dies with "Error 71 (Protocol error)" unless DMA-BUF is disabled.
export WEBKIT_DISABLE_DMABUF_RENDERER="${WEBKIT_DISABLE_DMABUF_RENDERER:-1}"

case "$mode" in
  desktop)
    cd "$root/apps/desktop" && exec pnpm tauri dev "$@"
    ;;
  both)
    cd "$root/apps/desktop" && MARQUEE_WEB=1 exec pnpm tauri dev "$@"
    ;;
  web)
    (cd "$root/apps/desktop" && pnpm build)
    cd "$root" && exec cargo run -q -p marquee-cli -- run web "$@"
    ;;
  dev)
    cd "$root"
    cargo run -q -p marquee-cli -- run web "$@" &
    server=$!
    trap 'kill "$server" 2>/dev/null || true' EXIT
    cd apps/desktop && pnpm dev
    ;;
  *)
    echo "usage: scripts/run.sh desktop|web|dev|both [args...]" >&2
    exit 2
    ;;
esac
