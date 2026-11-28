#!/usr/bin/env bash
# Starts the third-party The Order: 1886 port (JulioCacko/The-Order-1886_pc) for
# Marquee. Marquee copies this file beside the downloaded program.
#
#   marquee-run.sh <game folder>/eboot.bin
#
# The program is untrusted: it runs without network or a view of the home folder.
# It sees the game (read-only), its own save folder, the GPU, game controllers and
# the display. Sound goes to a private audio server that has only a silent output
# and refuses to load modules, so the program cannot reach the real server or the
# microphone. Marquee copies that output to the speakers, one way.
#
# Known limit: an X11 display is shared with the other X11 programs on the desktop.
set -euo pipefail

app=$(cd "$(dirname "$0")" && pwd)
eboot=${1:?usage: marquee-run.sh <eboot.bin>}
game=$(cd "$(dirname "$eboot")" && pwd)
config=${XDG_CONFIG_HOME:?Marquee must set XDG_CONFIG_HOME}
state=${config%/*}/user

titles='CUSA00076|CUSA00035|CUSA00070|CUSA00100|CUSA00670|CUSA00785'
if ! grep -a -q -E "$titles" "$game/sce_sys/param.sfo" 2>/dev/null; then
    echo "This runner only starts The Order: 1886" >&2
    exit 2
fi
mkdir -p "$state" "$app/user"

audio=$(mktemp -d)
pids=()
cleanup() {
    [ ${#pids[@]} -eq 0 ] || kill "${pids[@]}" 2>/dev/null || true
    rm -rf "$audio"
}
trap cleanup EXIT
trap 'exit 143' TERM INT

wait_socket() {
    for _ in $(seq 1 100); do [ -S "$1" ] && return 0; sleep 0.1; done
    return 1
}
# The private servers must not inherit anything that points at another audio
# server: PIPEWIRE_REMOTE overrides the runtime folder, and with it the "private"
# server would join the real one, microphone included.
clean=(-u PIPEWIRE_REMOTE -u PIPEWIRE_RUNTIME_DIR -u PIPEWIRE_NODE -u PIPEWIRE_PROPS
    -u PULSE_SERVER -u PULSE_SINK -u PULSE_SOURCE -u PULSE_CLIENTCONFIG -u PULSE_COOKIE)
# exec: each background job must be the server itself so cleanup can stop it.
private() {
    exec env "${clean[@]}" XDG_RUNTIME_DIR="$audio" PIPEWIRE_RUNTIME_DIR="$audio" \
        PULSE_RUNTIME_PATH="$audio/pulse" XDG_CONFIG_HOME="$audio/conf" "$@"
}
ask() { env "${clean[@]}" XDG_RUNTIME_DIR="$audio" PULSE_SERVER="unix:$audio/pulse/native" "$@"; }
# Names of the private server's sinks or sources, one per line.
names() { ask pactl list short "$1" 2>/dev/null | awk '{print $2}'; }
# Fail closed: the sandbox gets this server only if it holds nothing but the
# silent output and its monitor, and refuses to load modules.
isolated() {
    [ "$(names sinks)" = silent ] && [ "$(names sources)" = silent.monitor ] \
        && ! ask pactl load-module module-null-sink sink_name=probe > /dev/null 2>&1
}
start_audio() {
    mkdir -p "$audio/conf/pipewire/pipewire-pulse.conf.d"
    cat > "$audio/conf/pipewire/pipewire-pulse.conf.d/90-marquee.conf" <<'CONF'
pulse.properties = {
    pulse.allow-module-loading = false
}
pulse.cmd = [
    { cmd = "load-module" args = "module-null-sink sink_name=silent" flags = [ ] }
]
CONF
    private pipewire > "$audio/pipewire.log" 2>&1 & pids+=($!)
    wait_socket "$audio/pipewire-0" || return 1
    # The policy profile runs no hardware monitors: no sound cards, no microphone.
    private wireplumber -p policy > "$audio/wireplumber.log" 2>&1 & pids+=($!)
    private pipewire-pulse > "$audio/pulse.log" 2>&1 & pids+=($!)
    wait_socket "$audio/pulse/native" || return 1
    for _ in $(seq 1 50); do
        isolated && return 0
        sleep 0.1
    done
    return 1
}
sound=()
if command -v pipewire wireplumber pipewire-pulse pactl parec pacat > /dev/null && start_audio; then
    format=(--format=s16le --rate=48000 --channels=2 --latency-msec=40 --raw)
    # parec reads the private server; pacat plays on the desktop's own.
    ask parec -d silent.monitor "${format[@]}" 2>/dev/null \
        | pacat --playback --client-name="Marquee game audio" "${format[@]}" 2>/dev/null &
    pids+=($!)
    sound=(--ro-bind "$audio/pulse/native" /tmp/pulse-native
        --setenv PULSE_SERVER unix:/tmp/pulse-native)
else
    echo "No isolated audio server; starting without sound" >&2
fi

devices=()
for node in /dev/dri /dev/input /dev/nvidia*; do
    [ -e "$node" ] && devices+=(--dev-bind "$node" "$node")
done
display=(--setenv DISPLAY "${DISPLAY:?no display}")
if [ -n "${XAUTHORITY:-}" ] && [ -f "$XAUTHORITY" ]; then
    display+=(--ro-bind "$XAUTHORITY" /tmp/Xauthority --setenv XAUTHORITY /tmp/Xauthority)
fi
update=()
for suffix in -UPDATE -patch; do
    if [ -d "$game$suffix" ]; then
        update=(--ro-bind "$game$suffix" /game-UPDATE)
        break
    fi
done

bwrap --unshare-all --die-with-parent --new-session \
    --ro-bind /usr /usr --ro-bind /etc /etc --ro-bind /sys /sys \
    --symlink usr/lib /lib --symlink usr/lib64 /lib64 --symlink usr/bin /bin \
    --dev /dev "${devices[@]}" --proc /proc --tmpfs /tmp --tmpfs /run \
    --ro-bind /tmp/.X11-unix /tmp/.X11-unix \
    --ro-bind "$app" /app --bind "$state" /app/user \
    --ro-bind "$game" /game "${update[@]}" \
    --clearenv --setenv HOME /tmp --setenv PATH /usr/bin \
    --setenv XDG_RUNTIME_DIR /tmp --setenv SDL_VIDEODRIVER x11 \
    "${display[@]}" "${sound[@]}" \
    --chdir /app "/app/Play The Order 1886" --game-dir /game &
child=$!
pids+=($child)
wait "$child"
