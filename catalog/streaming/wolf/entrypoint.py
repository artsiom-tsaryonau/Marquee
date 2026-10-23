"""Own only this container's PulseAudio and Wolf processes; no host services."""
import os
from pathlib import Path
import signal
import subprocess
import sys
import time
import uuid


def main():
    os.umask(0o077)
    state = Path("/state")
    state.mkdir(parents=True, exist_ok=True, mode=0o700)
    for folder in ["apps", "config", "pulse-state"]:
        (state / folder).mkdir(exist_ok=True, mode=0o700)
    if not (state / "config.toml").exists():
        defaults = Path("/opt/marquee-wolf/defaults.toml").read_text()
        if defaults.count("[gstreamer]\n") != 1:
            raise RuntimeError("Unexpected pinned GStreamer configuration")
        # Keep the reviewed encoding defaults, replace the upstream catalogue
        # with one synthetic process app. Never pull/start the default Wolf UI.
        profile = '''hostname = "Marquee local pilot"
config_version = 7
uuid = "''' + str(uuid.uuid4()) + '''"
paired_clients = []
[[profiles]]
id = "moonlight-profile-id"
[[profiles.apps]]
title = "Marquee streaming test"
start_virtual_compositor = false
start_audio_server = false
[profiles.apps.runner]
type = "process"
run_cmd = "/bin/sleep 2147483647"
[profiles.apps.video]
source = "videotestsrc pattern=ball flip=true is-live=true ! video/x-raw, framerate={fps}/1"
[profiles.apps.audio]
source = "audiotestsrc wave=ticks is-live=true"
'''
        with (state / "config.toml").open("x") as config:
            config.write(profile + "\n[gstreamer]\n" + defaults.split("[gstreamer]\n", 1)[1])
    runtime = state / "runtime"
    (runtime / "pulse").mkdir(parents=True, exist_ok=True, mode=0o700)
    os.environ.update({
        "XDG_RUNTIME_DIR": str(runtime),
        "PULSE_SERVER": str(runtime / "pulse/native"),
        "PULSE_RUNTIME_PATH": str(runtime / "pulse"),
        "PULSE_STATE_PATH": str(state / "pulse-state"),
        "XDG_CONFIG_HOME": str(state / "config"),
        "WOLF_CFG_FILE": str(state / "config.toml"),
        "WOLF_PRIVATE_KEY_FILE": str(state / "key.pem"),
        "WOLF_PRIVATE_CERT_FILE": str(state / "cert.pem"),
        "WOLF_SOCKET_PATH": str(runtime / "wolf.sock"),
        "HOST_APPS_STATE_FOLDER": str(state / "apps"),
        "WOLF_DOCKER_SOCKET": "/nonexistent/marquee-no-container-api.sock",
        "WOLF_INTERNAL_IP": "127.0.0.1",
        "WOLF_LOG_LEVEL": "INFO",
    })
    processes = []
    stopping = False

    def stop(_signum, _frame):
        nonlocal stopping
        stopping = True

    signal.signal(signal.SIGINT, stop)
    signal.signal(signal.SIGTERM, stop)
    try:
        pulse = subprocess.Popen([
            "pulseaudio", "-n", "--daemonize=false", "--fail=true",
            "--use-pid-file=false", "--exit-idle-time=-1", "--log-target=stderr",
            "--load=module-native-protocol-unix auth-anonymous=1 socket=" + os.environ["PULSE_SERVER"],
            "--load=module-always-sink",
        ])
        processes.append(pulse)
        end = time.monotonic() + 10
        while not Path(os.environ["PULSE_SERVER"]).is_socket():
            if stopping or pulse.poll() is not None or time.monotonic() >= end:
                raise RuntimeError("Private streaming audio server did not start")
            time.sleep(0.05)
        wolf = subprocess.Popen(["/opt/marquee-wolf/wolf"])
        processes.append(wolf)
        while not stopping:
            if wolf.poll() is not None:
                return wolf.returncode or 1
            if pulse.poll() is not None:
                raise RuntimeError("Private streaming audio server exited")
            time.sleep(0.1)
        return 0
    finally:
        for process in reversed(processes):
            if process.poll() is None:
                process.send_signal(signal.SIGINT)
                try:
                    process.wait(timeout=30)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()


if __name__ == "__main__":
    sys.exit(main())
