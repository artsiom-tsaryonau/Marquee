#!/usr/bin/env bash
# Play check: start a library game through Marquee's own Play path, but inside a
# private headless gamescope display, take screenshots and optionally record a video.
# Nothing appears on the desktop.
#
#   scripts/play-check.sh <edition-id> [--seconds 60] [--shots 20,40,60]
#                         [--size 1280x720] [--record] [--out DIR] [--drive] [--keep-saves]
#
# --drive keeps the game running until --seconds pass or OUT/stop appears, so
# scripts/headless-drive.py can play it: `headless-drive.py --session OUT seq ...`.
# Emulator saves are snapshotted first and restored afterwards (where they are kept
# is recorded in OUT/saves.json); --keep-saves leaves them. See scripts/play_check_saves.py.
#
# Needs gamescope (with gamescopectl), and for --record GStreamer pipewiresrc and
# openh264enc. Uses target/release/marquee unless MARQUEE_BIN is set. Games get a
# private silent audio server; scripts/play_check_audio_guard.py kills the game if a
# stream still reaches an output.
set -euo pipefail

usage() { sed -n '2,15p' "$0" | sed 's/^# \{0,1\}//'; exit 2; }
[ $# -ge 1 ] || usage
id=$1; shift
seconds=60; shots=""; size=1280x720; record=0; out=""; drive=0; keep_saves=0
while [ $# -gt 0 ]; do
    case $1 in
        --seconds) seconds=$2; shift 2 ;;
        --shots) shots=$2; shift 2 ;;
        --size) size=$2; shift 2 ;;
        --record) record=1; shift ;;
        --drive) drive=1; shift ;;
        --keep-saves) keep_saves=1; shift ;;
        --out) out=$2; shift 2 ;;
        *) usage ;;
    esac
done
[[ $size =~ ^([0-9]+)x([0-9]+)$ ]] || usage
w=${BASH_REMATCH[1]}; h=${BASH_REMATCH[2]}
[ -n "$shots" ] || [ "$drive" = 1 ] || shots=$(seq -s, 15 15 "$seconds")
root=$(cd "$(dirname "$0")/.." && pwd)
bin=${MARQUEE_BIN:-$root/target/release/marquee}
out=${out:-$root/data/play-checks/$(date +%Y%m%d-%H%M%S)-$id}
mkdir -p "$out"
out=$(cd "$out" && pwd)
rm -f "$out/stop" "$out/xdisplay" "$out/wayland" "$out/exited"
runtime=${XDG_RUNTIME_DIR:-/run/user/$(id -u)}

# Silence: the game talks to a private PipeWire with one null sink and no sound
# cards ("-p policy" runs WirePlumber without hardware monitors), so nothing can
# reach the speakers whatever device a game asks for. A server is needed because
# some emulators refuse to start without audio: PPSSPP's AppImage forces
# SDL_AUDIODRIVER=pulseaudio.
silent_dir=$(mktemp -d)
audio_pids=()
# exec: the background job must be the server itself, so cleanup can kill it.
private() { exec env -u PIPEWIRE_REMOTE -u PULSE_SERVER XDG_RUNTIME_DIR="$silent_dir" PIPEWIRE_RUNTIME_DIR="$silent_dir" PULSE_RUNTIME_PATH="$silent_dir/pulse" "$@"; }
wait_socket() { for _ in $(seq 1 100); do [ -S "$1" ] && return 0; sleep 0.1; done; echo "private audio server did not start ($1)" >&2; return 1; }
start_silent_audio() {
    private pipewire > "$silent_dir/pipewire.log" 2>&1 & audio_pids+=($!)
    wait_socket "$silent_dir/pipewire-0"
    private wireplumber -p policy > "$silent_dir/wireplumber.log" 2>&1 & audio_pids+=($!)
    private pipewire-pulse > "$silent_dir/pulse.log" 2>&1 & audio_pids+=($!)
    wait_socket "$silent_dir/pulse/native"
    PULSE_SERVER="unix:$silent_dir/pulse/native" pactl load-module module-null-sink sink_name=silent > /dev/null
}
# Only the game gets these (after gamescope's --). Its runtime folder is private so
# containers mount the silent sockets; gamescope's own sockets are linked in, or
# its Vulkan layer cannot connect and some games abort. gamescope and the recorder still
# need the real PipeWire for the video stream. The shell step also tells the driver
# which XWayland display the game got, and keeps the emulator's own output.
silence="\"sh\",\"-c\",\"printf %s \\\"\$DISPLAY\\\" > '$out/xdisplay'; for s in \\\"\$XDG_RUNTIME_DIR\\\"/gamescope-* \\\"\$XDG_RUNTIME_DIR\\\"/bus; do [ -e \\\"\$s\\\" ] && ln -sf \\\"\$s\\\" '$silent_dir'/; done; exec \\\"\$0\\\" \\\"\$@\\\" > '$out/game.log' 2>&1\",\"env\",\"MARQUEE_PLAY_CHECK=$silent_dir\",\"PULSE_SERVER=unix:$silent_dir/pulse/native\",\"PIPEWIRE_REMOTE=$silent_dir/pipewire-0\",\"XDG_RUNTIME_DIR=$silent_dir\""
# Every process the game starts inherits this marker, so they can all be found even
# after gamescope exits: xemu ignores SIGTERM and would otherwise keep running.
game_pids() { grep -lsz "^MARQUEE_PLAY_CHECK=$silent_dir\$" /proc/[0-9]*/environ 2>/dev/null | cut -d/ -f3 || true; }
stop_game() {
    pkill -TERM -f "gamescope --backend headless -W $w -H $h" 2>/dev/null || true
    for pid in $(game_pids); do kill -TERM "$pid" 2>/dev/null || true; done
    for _ in $(seq 1 50); do [ -n "$(game_pids)" ] || return 0; sleep 0.2; done
    for pid in $(game_pids); do kill -KILL "$pid" 2>/dev/null || true; done
    for _ in $(seq 1 50); do [ -n "$(game_pids)" ] || return 0; sleep 0.2; done
}
# On any exit, stop the game first.
cleanup() {
    stop_game
    pkill -KILL -f "gamescope --backend headless -W $w -H $h" 2>/dev/null || true
    kill "${guard:-}" 2>/dev/null || true
    [ ${#audio_pids[@]} -eq 0 ] || kill "${audio_pids[@]}" 2>/dev/null || true
    if [ "${snapshot:-0}" = 1 ]; then
        sleep 1
        python3 "$root/scripts/play_check_saves.py" restore "$out" || echo "SAVE RESTORE FAILED (see the message above for where the originals are)" | tee "$out/restore-failed" >&2
    fi
    rm -rf ${marker:+"$marker"} "$silent_dir"
}
trap cleanup EXIT
trap 'exit 130' INT TERM

if [ "$keep_saves" = 0 ]; then
    # Refuses to run while any emulator is open, so a real session is never touched.
    python3 "$root/scripts/play_check_saves.py" snapshot "$out"
    snapshot=1
fi
start_silent_audio
marker=$(mktemp)
export MARQUEE_DISPLAY_MODE=window
export MARQUEE_LAUNCH_PREFIX="[\"gamescope\",\"--backend\",\"headless\",\"-W\",\"$w\",\"-H\",\"$h\",\"-w\",\"$w\",\"-h\",\"$h\",\"--\",$silence]"
"$bin" launch "$id" > "$out/marquee.log" 2>&1 &
launcher=$!
# Safety net: any stream that still reaches a real output kills the game at once.
python3 "$root/scripts/play_check_audio_guard.py" "$((seconds + 30))" > "$out/audio.log" 2>&1 &
guard=$!

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
printf %s "$display" > "$out/wayland"
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
while [ $(( $(date +%s) - start )) -lt "$seconds" ] && kill -0 "$launcher" 2>/dev/null && [ ! -e "$out/stop" ]; do sleep 0.5; done
if ! kill -0 "$launcher" 2>/dev/null; then
    echo "game exited by itself after $(( $(date +%s) - start ))s" | tee "$out/exited"
fi

# Finish the recording first (EOS writes the MP4 index), then stop the game the
# way Marquee's Stop does: the gamescope group gets SIGTERM.
if [ -n "$recorder" ]; then
    kill -INT "$recorder" 2>/dev/null || true
    wait "$recorder" 2>/dev/null || true
fi
stop_game
wait "$launcher" 2>/dev/null || true
sleep 1
ls -1 "$out"
