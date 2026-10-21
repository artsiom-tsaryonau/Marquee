#!/usr/bin/env bash
# One way to start Marquee:
#   scripts/run.sh desktop   production app, rebuilt when sources change; no Vite/web server
#   scripts/run.sh build     build the production app without opening a window
#   scripts/run.sh web       the web UI only, no window (builds the UI first)
#   scripts/run.sh dev       web API + Vite with hot reload, at http://localhost:1420
#   scripts/run.sh both      the app window, also served to a browser
# Extra arguments go to the underlying command.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
mode="${1:-}"
shift || true

# Native startup selects a compatible accelerated transport. The software
# fallback is opt-in: MARQUEE_RENDERER=software. See docs/DESKTOP_RENDERING.md.

release_binary() {
  (cd "$root" && cargo metadata --no-deps --format-version=1 | node -e '
    let input = ""; process.stdin.setEncoding("utf8"); process.stdin.on("data", chunk => input += chunk);
    process.stdin.on("end", () => process.stdout.write(JSON.parse(input).target_directory + "/release/marquee-desktop"));
  ')
}
build_release() {
  (cd "$root/apps/desktop" && pnpm tauri build --no-bundle "$@")
}
ensure_release() {
  binary="$(release_binary)"
  local rebuild=0 changed
  if [[ ! -x "$binary" || ${MARQUEE_REBUILD:-0} == 1 ]]; then rebuild=1; fi
  if [[ $rebuild == 0 ]]; then
    # Directory mtimes also catch removed inputs; prune build outputs so merely
    # running Vite/pnpm does not invalidate the production executable.
    changed="$(cd "$root" && find apps/desktop crates/core catalog Cargo.toml Cargo.lock \
      -type d \( -name node_modules -o -name dist -o -name target -o -name gen \) -prune \
      -o -newer "$binary" -print -quit)"
    if [[ -n $changed ]]; then rebuild=1; fi
  fi
  if [[ $rebuild == 1 ]]; then build_release; fi
}

case "$mode" in
  desktop|both)
    ensure_release
    if [[ $mode == both ]]; then export MARQUEE_WEB=1; fi
    exec "$binary" "$@"
    ;;
  build)
    build_release "$@"
    ;;
  web)
    (cd "$root/apps/desktop" && pnpm build)
    cd "$root" && exec cargo run -q -p marquee-cli -- run web "$@"
    ;;
  dev)
    if (exec 3<>/dev/tcp/127.0.0.1/1420) 2>/dev/null; then
      echo "Port 1420 is already in use (a stale Vite server?). Stop it first so the UI cannot be an old build." >&2
      exit 1
    fi
    cd "$root"
    cargo run -q -p marquee-cli -- run web "$@" &
    server=$!
    trap 'kill "$server" 2>/dev/null || true' EXIT
    cd apps/desktop && pnpm dev
    ;;
  *)
    echo "usage: scripts/run.sh desktop|web|dev|both|build [args...]" >&2
    exit 2
    ;;
esac
