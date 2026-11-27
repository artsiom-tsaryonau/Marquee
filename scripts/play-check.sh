#!/usr/bin/env bash
# Play check: start a library game through Marquee's own Play path, but inside a
# private headless gamescope display, take screenshots and optionally record a video.
# Nothing appears on the desktop.
#
#   scripts/play-check.sh <edition-id> [--seconds 60] [--shots 20,40,60]
#                         [--size 1280x720] [--record] [--out DIR]
#
# Needs gamescope (with gamescopectl), and for --record GStreamer pipewiresrc and
# openh264enc. Uses target/release/marquee unless MARQUEE_BIN is set.
set -euo pipefail

usage() { sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//'; exit 2; }
[ $# -ge 1 ] || usage
id=$1; shift
seconds=60; shots=""; size=1280x720; record=0; out=""
while [ $# -gt 0 ]; do
    case $1 in
        --seconds) seconds=$2; shift 2 ;;
        --shots) shots=$2; shift 2 ;;
        --size) size=$2; shift 2 ;;
        --record) record=1; shift ;;
        --out) out=$2; shift 2 ;;
        *) usage ;;
    esac
done
[[ $size =~ ^([0-9]+)x([0-9]+)$ ]] || usage
w=${BASH_REMATCH[1]}; h=${BASH_REMATCH[2]}
[ -n "$shots" ] || shots=$(seq -s, 15 15 "$seconds")
root=$(cd "$(dirname "$0")/.." && pwd)
bin=${MARQUEE_BIN:-$root/target/release/marquee}
out=${out:-$root/data/play-checks/$(date +%Y%m%d-%H%M%S)-$id}
mkdir -p "$out"
runtime=${XDG_RUNTIME_DIR:-/run/user/$(id -u)}

# Sound goes to a private null sink, never to the speakers. SDL, PulseAudio and
# PipeWire clients all follow these variables; the sink is removed at the end.
sink="marquee_check_$$"
module=$(pactl load-module module-null-sink sink_name="$sink" \
    sink_properties=device.description="Marquee-play-check") || {
    echo "could not create a silent audio sink; not starting the game" >&2
    exit 1
}
# On any exit, stop the game before removing the sink: a removed sink's streams
# would otherwise move to the default output (the speakers).
cleanup() {
    pkill -KILL -f "gamescope --backend headless -W $w -H $h" 2>/dev/null || true
    sleep 1
    pactl unload-module "$module" 2>/dev/null || true
    rm -f "$marker"
}
trap cleanup EXIT
trap 'exit 130' INT TERM
export PULSE_SINK="$sink" PIPEWIRE_NODE="$sink"
# SDL can talk to PipeWire directly and ignore PULSE_SINK; make it use PulseAudio.
export SDL_AUDIODRIVER=pulseaudio SDL_AUDIO_DRIVER=pulseaudio

marker=$(mktemp)
export MARQUEE_DISPLAY_MODE=window
export MARQUEE_LAUNCH_PREFIX="[\"gamescope\",\"--backend\",\"headless\",\"-W\",\"$w\",\"-H\",\"$h\",\"-w\",\"$w\",\"-h\",\"$h\",\"--\"]"
"$bin" launch "$id" > "$out/marquee.log" 2>&1 &
launcher=$!

display=""
for _ in $(seq 1 100); do
    # The socket this gamescope created (names are reused after a crash).
    display=$(find "$runtime" -maxdepth 1 -name 'gamescope-[0-9]*' ! -name '*-ei' ! -name '*.lock' \
        -newer "$marker" -printf '%f\n' 2>/dev/null | head -1)
    [ -n "$display" ] && break
    sleep 0.1
done
if [ -z "$display" ]; then
    echo "gamescope did not start; see $out/marquee.log" >&2
    wait "$launcher" || true
    exit 1
fi
echo "display $display, output $out"

recorder=""
if [ "$record" = 1 ]; then
    # Attach to this gamescope's PipeWire video node by serial (names are shared).
    serial=""
    for _ in $(seq 1 50); do
        serial=$(pw-dump 2>/dev/null | python3 -c '
import json, sys
nodes = [o for o in json.load(sys.stdin) if o.get("type") == "PipeWire:Interface:Node"
         and o.get("info", {}).get("props", {}).get("node.name") == "gamescope"]
print(nodes[-1]["info"]["props"].get("object.serial", "") if nodes else "")')
        [ -n "$serial" ] && break
        sleep 0.2
    done
    if [ -n "$serial" ]; then
        gst-launch-1.0 -e -q pipewiresrc target-object="$serial" do-timestamp=true \
            ! videoconvert ! video/x-raw,format=I420 ! openh264enc bitrate=8000000 \
            ! h264parse ! mp4mux ! filesink location="$out/recording.mp4" 2>"$out/gst.log" &
        recorder=$!
    else
        echo "no gamescope PipeWire stream; recording skipped" >&2
    fi
fi

start=$(date +%s)
IFS=, read -ra marks <<< "$shots"
for mark in "${marks[@]}"; do
    while [ $(( $(date +%s) - start )) -lt "$mark" ]; do
        kill -0 "$launcher" 2>/dev/null || break 2
        sleep 0.5
    done
    GAMESCOPE_WAYLAND_DISPLAY=$display gamescopectl screenshot "$out/shot-$(printf '%03d' "$mark")s.png" >/dev/null 2>&1 || true
done
while [ $(( $(date +%s) - start )) -lt "$seconds" ] && kill -0 "$launcher" 2>/dev/null; do sleep 0.5; done

# Finish the recording first (EOS writes the MP4 index), then stop the game the
# way Marquee's Stop does: the gamescope group gets SIGTERM.
if [ -n "$recorder" ]; then
    kill -INT "$recorder" 2>/dev/null || true
    wait "$recorder" 2>/dev/null || true
fi
pkill -TERM -f "gamescope --backend headless -W $w -H $h" 2>/dev/null || true
wait "$launcher" 2>/dev/null || true
sleep 1
ls -1 "$out"
