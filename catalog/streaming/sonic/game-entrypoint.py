"""Explicit Sonic pilot; retain the reviewed host identity and synthetic app."""
import os
import json
import math
from pathlib import Path
import runpy
import tomllib

APP = '''
[[profiles.apps]]
title = "Marquee Sonic pilot"
start_virtual_compositor = true
start_audio_server = true
[profiles.apps.runner]
type = "process"
run_cmd = "/usr/bin/python3 /opt/marquee-wolf/game.py"
'''


def configure(path):
    text = path.read_text()
    if len(text.encode()) > 1024 * 1024:
        raise RuntimeError("Pilot configuration exceeds the review limit")
    data = tomllib.loads(text)
    profiles = data.get("profiles", [])
    if len(profiles) != 1 or not isinstance(data.get("gstreamer"), dict):
        raise RuntimeError("Game pilot requires the reviewed single-profile host")
    apps = profiles[0].get("apps", [])
    existing = [a for a in apps if a.get("title") == "Marquee Sonic pilot"]
    expected = tomllib.loads("[[profiles]]\n" + APP)["profiles"][0]["apps"][0]
    if existing:
        if len(existing) != 1 or existing[0] != expected:
            raise RuntimeError("An incompatible Sonic pilot app was preserved")
        return
    if len(apps) != 1 or apps[0].get("title") != "Marquee streaming test":
        raise RuntimeError("Refusing to modify an unrelated Wolf catalogue")
    apps.append(expected)
    # Wolf rewrites its config using inline arrays/tables after pairing. Emit
    # only the pinned contract's primitive types and require a lossless parse
    # before replacement; unsupported TOML values leave the original untouched.
    replacement = "\n".join(json.dumps(key) + " = " + inline(value)
                            for key, value in data.items()) + "\n"
    if tomllib.loads(replacement) != data:
        raise RuntimeError("Pilot configuration did not round-trip losslessly")
    temporary = path.with_suffix(".game-new")
    with temporary.open("x") as out:
        os.chmod(temporary, 0o600)
        out.write(replacement)
        out.flush()
        os.fsync(out.fileno())
    temporary.replace(path)


def inline(value):
    if isinstance(value, (str, bool)) or type(value) is int:
        return json.dumps(value, ensure_ascii=True)
    if type(value) is float and math.isfinite(value):
        return json.dumps(value)
    if isinstance(value, list):
        return "[" + ", ".join(inline(item) for item in value) + "]"
    if isinstance(value, dict):
        return "{" + ", ".join(json.dumps(key) + " = " + inline(item)
                                for key, item in value.items()) + "}"
    raise RuntimeError("Unsupported value in the reviewed pilot configuration")


if __name__ == "__main__":
    os.umask(0o077)
    rom = Path("/games/sonic.md")
    if not rom.is_file() or not 512 <= rom.stat().st_size <= 16 * 1024 * 1024:
        raise RuntimeError("Mount the reviewed Sonic input read-only at /games/sonic.md")
    configure(Path("/state/config.toml"))
    runpy.run_path("/opt/marquee-wolf/entrypoint.py", run_name="__main__")
