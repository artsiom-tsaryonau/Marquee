#!/bin/sh
# Starts a Linux Ren'Py game the way you would yourself: through its own script.
#
#   marquee-run.sh <game folder>/<Name>.sh
#
# Marquee's emulator profile folders are dropped so the game keeps using your own
# ~/.renpy saves. The game is not sandboxed.
unset XDG_CONFIG_HOME XDG_DATA_HOME XDG_CACHE_HOME
script=${1:?usage: marquee-run.sh <Name.sh>}
cd "$(dirname "$script")" || exit 1
exec /bin/sh "$script"
