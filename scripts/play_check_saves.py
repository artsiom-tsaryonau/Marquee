"""Keep play checks away from the owner's saves.

snapshot OUT  refuse if an emulator is already running, then copy every save
              location that exists into a work folder named in OUT/saves.json.
restore OUT   move what the run left behind into WORK/after, put the originals
              back, then drop the copies.

Work folders sit beside the data folder, on the same disk: reflinks make the
copies nearly free and the restore can rename. /tmp is the wrong place (it is
in memory and filled up). MARQUEE_DATA_DIR overrides the data folder.
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

DATA = Path(os.environ.get("MARQUEE_DATA_DIR", Path.home() / ".local/share/marquee"))
WORK = DATA.parent / "marquee-play-checks"
KEEP_RUNS = 3

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
    "order1886/user/order1886/savedata",
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


def prune(keep: int):
    """Drop old finished runs. A run that still has `before` was not restored, so
    it holds originals and is never removed."""
    finished = sorted(
        (p for p in WORK.iterdir() if p.is_dir() and not (p / "before").exists()),
        key=lambda p: p.stat().st_mtime,
        reverse=True,
    )
    for old in finished[keep:]:
        shutil.rmtree(old, ignore_errors=True)


def snapshot(out: Path):
    busy = running_emulators()
    if busy:
        sys.exit("An emulator is already running; not touching its saves:\n" + "\n".join(busy))
    record = out / "saves.json"
    if record.exists():
        sys.exit(f"{record} exists; use a new --out folder")
    WORK.mkdir(parents=True, exist_ok=True)
    work = Path(tempfile.mkdtemp(prefix=f"{out.name}-", dir=WORK))
    kept = matches()
    try:
        for item in kept:
            copy(DATA / item, work / "before" / item)
    except BaseException:
        shutil.rmtree(work, ignore_errors=True)  # Nothing was touched yet.
        raise
    record.write_text(json.dumps({"work": str(work), "items": kept}, indent=1))


def restore(out: Path):
    record = json.loads((out / "saves.json").read_text())
    work, kept = Path(record["work"]), record["items"]
    busy = running_emulators()
    if busy:
        sys.exit("An emulator is still running:\n" + "\n".join(busy) + f"\nOriginals: {work}/before")
    try:
        # A rename needs no free space, so a full disk cannot stop this half way.
        for item in matches():
            target = work / "after" / item
            target.parent.mkdir(parents=True, exist_ok=True)
            os.rename(DATA / item, target)
        for item in kept:
            (DATA / item).parent.mkdir(parents=True, exist_ok=True)
            copy(work / "before" / item, DATA / item)
    except (OSError, subprocess.CalledProcessError) as error:
        sys.exit(f"{error}\nOriginals: {work}/before")
    shutil.rmtree(work / "before", ignore_errors=True)
    prune(keep=KEEP_RUNS)


if __name__ == "__main__":
    if len(sys.argv) != 3 or sys.argv[1] not in ("snapshot", "restore"):
        sys.exit(__doc__)
    {"snapshot": snapshot, "restore": restore}[sys.argv[1]](Path(sys.argv[2]))
