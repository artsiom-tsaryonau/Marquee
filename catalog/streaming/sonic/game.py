"""Run BlastEm directly in Wolf's session; never invoke its Podman wrapper."""
import fcntl
import json
import os
from pathlib import Path
import subprocess


def configuration(text):
    replacements = {
        "save_path $USERDATA/blastem/$ROMNAME": "save_path /state/game-saves/sonic",
        "screenshot_path $HOME": "screenshot_path /state/game-saves/sonic",
        "vgm_path $HOME": "vgm_path /state/game-saves/sonic",
        "\t\tl ui.load_state": "\t\tl ui.load_state\n\t\tf6 ui.save_state\n\t\tf9 ui.load_state",
    }
    for old, new in replacements.items():
        if text.count(old) != 1:
            raise RuntimeError("Unexpected pinned BlastEm configuration")
        text = text.replace(old, new)
    return text


def main():
    os.umask(0o077)
    saves = Path("/state/game-saves/sonic")
    saves.mkdir(parents=True, exist_ok=True, mode=0o700)
    # A second client/session must not write the same test saves concurrently.
    with (saves / "writer.lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        runtime = Path(os.environ["XDG_RUNTIME_DIR"])
        display = os.environ["WAYLAND_DISPLAY"]
        if "/" in display or not (runtime / display).is_socket():
            raise RuntimeError("Wolf's private Wayland display is unavailable")
        if not os.environ.get("PULSE_SINK") or not os.environ.get("PULSE_SERVER"):
            raise RuntimeError("Wolf's session audio route is unavailable")
        # Only the container's ephemeral home/config is used. The actual host
        # home and personal saves are never mounted or changed.
        config_dir = Path.home() / ".config/blastem"
        config_dir.mkdir(parents=True, exist_ok=True, mode=0o700)
        text = configuration(Path("/opt/blastem/default.cfg").read_text())
        (config_dir / "blastem.cfg").write_text(text)
        env = os.environ.copy()
        env.update(SDL_VIDEODRIVER="wayland", SDL_AUDIODRIVER="pulseaudio",
                   SDL_RENDER_DRIVER="software", XDG_CONFIG_HOME=str(config_dir.parent))
        cache = Path("/state/game-cache")
        cache.mkdir(exist_ok=True, mode=0o700)
        env["XDG_CACHE_HOME"] = str(cache)
        env.pop("DISPLAY", None)
        # A private log survives a remote stop/receiver disconnect.
        with (saves / "last-launch.log").open("w") as log:
            result = subprocess.run(["/opt/blastem/blastem", "-g", "-f", "/games/sonic.md"],
                                    env=env, stdout=log, stderr=subprocess.STDOUT, check=False)
        (saves / "last-exit.json").write_text(json.dumps({"exit_code": result.returncode}))
        return result.returncode


if __name__ == "__main__":
    raise SystemExit(main())
