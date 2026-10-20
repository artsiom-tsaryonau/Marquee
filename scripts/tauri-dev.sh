#!/usr/bin/env bash
# Open the desktop window. WebKitGTK on NVIDIA Wayland dies with
# "Error 71 (Protocol error)" unless DMA-BUF is disabled.
set -euo pipefail
cd "$(dirname "$0")/../apps/desktop"
export WEBKIT_DISABLE_DMABUF_RENDERER="${WEBKIT_DISABLE_DMABUF_RENDERER:-1}"
exec pnpm tauri dev "$@"
