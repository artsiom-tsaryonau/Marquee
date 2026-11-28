"""Keep play checks away from the owner's saves.

snapshot OUT  refuse if an emulator is already running, then copy every save
              location that exists into OUT/saves-before.
restore OUT   move what the run left behind into OUT/saves-after and put the
              originals back.

MARQUEE_DATA_DIR overrides the data folder.
"""
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

DATA = Path(os.environ.get("MARQUEE_DATA_DIR", Path.home() / ".local/share/marquee"))

# Globs below the data folder: folders, or single files (xemu keeps saves inside
# its virtual hard disk).
SAVES = [
    "azahar/xdg-data/azahar-emu/nand",
    "azahar/xdg-data/azahar-emu/sdmc",
    "blastem/xdg-data/blastem",
    "cemu/xdg-data/Cemu/mlc01/usr/save",
    "dolphin/xdg-data/dolphin-emu/GC",
    "dolphin/xdg-data/dolphin-emu/Wii",
    "duckstation/xdg-config/duckstation/memcards",
    "eden/xdg-data/eden/nand/user/save",
    "eden/xdg-data/eden/nand/system/save",
    "melonds/xdg-config/melonDS",
    "pcsx2/xdg-config/PCSX2/memcards",
    "ppsspp/xdg-config/ppsspp/PSP/SAVEDATA",
    "rpcs3/xdg-config/rpcs3/dev_hdd0/home/*/savedata",
    "rpcs3/xdg-config/rpcs3/dev_hdd0/savedata",
    "shadps4/xdg-data/shadPS4/home/*/savedata",
    "shadps4/xdg-data/shadPS4/home/*/trophy",
    "xemu/xdg-data/xemu/xemu/xbox_hdd.qcow2",
    "xemu/xdg-data/xemu/xemu/eeprom.bin",
    "xenia/content",
    "ymir/xdg-data/StrikerX3/Ymir/backup",
]


def running_emulators():
    runners = str(DATA / "runners")
    found = []
    for proc in Path("/proc").iterdir():
        if not proc.name.isdigit():
            continue
        try:
            exe = os.readlink(proc / "exe")
            argv0 = (proc / "cmdline").read_bytes().split(b"\0")[0].decode(errors="replace")
        except OSError:
            continue
        if exe.startswith(runners) or argv0.startswith(runners):
            found.append(f"{proc.name} {argv0}")
    return found


def copy(source: Path, target: Path):
    target.parent.mkdir(parents=True, exist_ok=True)
    # reflink where the filesystem offers it, so large disk images are cheap.
    subprocess.run(["cp", "-a", "--reflink=auto", str(source), str(target)], check=True)


def remove(path: Path):
    if path.is_dir() and not path.is_symlink():
        shutil.rmtree(path)
    else:
        path.unlink(missing_ok=True)


def matches():
    return sorted({str(p.relative_to(DATA)) for pattern in SAVES for p in DATA.glob(pattern)})


def snapshot(out: Path):
    busy = running_emulators()
    if busy:
        sys.exit("An emulator is already running; not touching its saves:\n" + "\n".join(busy))
    before = out / "saves-before"
    if before.exists():
        sys.exit(f"{before} exists; use a new --out folder")
    before.mkdir(parents=True)
    kept = matches()
    for item in kept:
        copy(DATA / item, before / item)
    (out / "saves.json").write_text(json.dumps(kept, indent=1))


def restore(out: Path):
    kept = json.loads((out / "saves.json").read_text())
    busy = running_emulators()
    if busy:
        sys.exit("An emulator is still running:\n" + "\n".join(busy))
    for item in matches():
        copy(DATA / item, out / "saves-after" / item)
        remove(DATA / item)
    for item in kept:
        copy(out / "saves-before" / item, DATA / item)


if __name__ == "__main__":
    if len(sys.argv) != 3 or sys.argv[1] not in ("snapshot", "restore"):
        sys.exit(__doc__)
    {"snapshot": snapshot, "restore": restore}[sys.argv[1]](Path(sys.argv[2]))
