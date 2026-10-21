#!/usr/bin/env bash
# Download the emulators your library needs into Marquee's own folder
# (~/.local/share/marquee/runners). Nothing is installed on the system and
# nothing is added to this repository. Safe to re-run.
#   scripts/install-emulators.sh            what your library folders need
#   scripts/install-emulators.sh <id>...    specific emulators (see: marquee emulators list)
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
run() { cargo run -q --release -p marquee-cli -- "$@"; }
if [ "$#" -eq 0 ]; then
  run emulators install-needed
else
  for id in "$@"; do run emulators install "$id" >/dev/null && echo "installed: $id"; done
fi
