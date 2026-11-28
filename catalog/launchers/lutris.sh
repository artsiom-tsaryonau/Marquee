#!/bin/bash
# Starts a game that is set up in Lutris, without opening Lutris.
#
#   marquee-run.sh ~/.local/share/lutris/games/<slug>-<time>.yml
#
# Lutris writes the game's launch script (its Wine or Proton runner, prefix and
# environment as configured there); this runs it, so the session lasts as long as
# the game. Lutris pre-launch and post-exit extras that its script omits do not run.
set -eu
unset XDG_CONFIG_HOME XDG_DATA_HOME XDG_CACHE_HOME
config=$(basename "${1:?usage: marquee-run.sh <game.yml>}" .yml)
slug=$(printf '%s' "$config" | sed -E 's/-[0-9]+$//')
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
(cd "$work" && lutris --output-script "$slug" > lutris.log 2>&1) || true
# The file is named after the game's Lutris ID, which can differ from the config
# name (an installer's slug), so take whichever script it wrote.
script=$(find "$work" -maxdepth 1 -name '*.sh' -print -quit)
if [ -z "$script" ] || [ ! -s "$script" ]; then
    echo "Lutris could not write a launch script for $slug:" >&2
    tail -5 "$work/lutris.log" >&2
    exit 1
fi
bash "$script"
