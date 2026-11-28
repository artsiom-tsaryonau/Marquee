#!/usr/bin/env python3
"""Run a Windows game for Marquee with umu-launcher and Proton.

    marquee-run.py <game>.mqwin                 start the game
    marquee-run.py <game>.mqwin --run PROGRAM…  run something else in its prefix
                                                (an installer, winecfg, winetricks)

Marquee owns umu (<data>/windows/umu) and its Proton copies (<data>/windows/proton).
The game keeps its own prefix. Games are not sandboxed.
"""
import json
import os
import shutil
import signal
import subprocess
import sys
from pathlib import Path


def main() -> int:
    if len(sys.argv) < 2:
        print(__doc__, file=sys.stderr)
        return 2
    config = os.environ.get("XDG_CONFIG_HOME")
    if not config:
        print("Marquee must set XDG_CONFIG_HOME", file=sys.stderr)
        return 2
    windows = Path(config).parent
    game = json.loads(Path(sys.argv[1]).read_text())
    umu = windows / "umu" / "umu-run"
    proton = windows / "proton" / game["proton"]
    for needed, what in ((umu, "umu-launcher"), (proton / "proton", game["proton"])):
        if not needed.is_file():
            print(f"{what} is missing; run: marquee windows setup", file=sys.stderr)
            return 1

    env = dict(os.environ)
    # Marquee's profile folders would move umu's own runtime and the user's files.
    for name in ("XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME"):
        env.pop(name, None)
    env.update(game.get("env", {}))
    env.update(WINEPREFIX=game["prefix"], PROTONPATH=str(proton))
    env.setdefault("GAMEID", "umu-default")
    env.setdefault("DXVK_ENABLE_NVAPI", "1")

    command = [sys.executable, str(umu)]
    if len(sys.argv) > 2 and sys.argv[2] == "--run":
        command += sys.argv[3:]
        cwd = None
    else:
        exe = Path(game["exe"])
        command += [str(exe), *game.get("args", [])]
        cwd = exe.parent
        if shutil.which("gamemoderun"):
            command.insert(0, "gamemoderun")

    child = subprocess.Popen(command, cwd=cwd, env=env)
    # Stop from Marquee signals the group; pass it on and still run post-exit.
    stopped = []

    def stop(number, _frame):
        stopped.append(number)
        child.send_signal(number)

    for number in (signal.SIGTERM, signal.SIGINT):
        signal.signal(number, stop)
    code = child.wait()
    after = game.get("post_exit")
    if after and len(sys.argv) == 2 and os.access(after, os.X_OK):
        subprocess.run([after], env=env, check=False)
    if stopped:
        return 0  # Being asked to close is not a crash, whatever Proton returns.
    return code if code >= 0 else 128 - code


if __name__ == "__main__":
    sys.exit(main())
