#!/usr/bin/env python3
"""Offline, same-filesystem collection relocation with durable compatibility paths.

Does not rewrite immutable manager receipts or library identities. Run plan first,
then apply; rerunning apply recovers an interrupted rename/link operation.
"""
import argparse
import ctypes
import fcntl
import hashlib
import json
import os
from pathlib import Path
import stat
import tempfile


def sync_dir(path):
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def save(path, value):
    fd, temporary = tempfile.mkstemp(prefix=".migration-", dir=path.parent)
    try:
        with os.fdopen(fd, "w") as stream:
            json.dump(value, stream, indent=2)
            stream.write("\n")
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, path)
        sync_dir(path.parent)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def inventory(root):
    """Hash regular files, record links verbatim, never traverse linked trees."""
    if root.is_symlink() or not root.is_dir():
        raise ValueError("Inventory root must be a regular directory")
    result = []
    def fail(error):
        raise error

    for directory, dirs, files in os.walk(root, followlinks=False, onerror=fail):
        for name in sorted(dirs + files):
            path = Path(directory) / name
            before = path.lstat()
            row = {"path": str(path.relative_to(root)), "mode": stat.S_IMODE(before.st_mode)}
            if stat.S_ISLNK(before.st_mode):
                row.update(kind="link", target=os.readlink(path))
            elif stat.S_ISDIR(before.st_mode):
                row.update(kind="directory")
            elif stat.S_ISREG(before.st_mode):
                digest = hashlib.sha256()
                with path.open("rb") as stream:
                    if os.fstat(stream.fileno()).st_ino != before.st_ino:
                        raise ValueError("Input changed during inventory")
                    for block in iter(lambda: stream.read(4 * 1024 * 1024), b""):
                        digest.update(block)
                after = path.lstat()
                if (before.st_ino, before.st_size, before.st_mtime_ns) != (
                    after.st_ino, after.st_size, after.st_mtime_ns
                ):
                    raise ValueError("Input changed during inventory")
                row.update(kind="file", size=before.st_size, sha256=digest.hexdigest())
            else:
                raise ValueError("Special files cannot be relocated")
            result.append(row)
    return sorted(result, key=lambda row: row["path"])


def present(path):
    return os.path.lexists(path)


def rename_exclusive(source, destination):
    # Linux renameat2 prevents a concurrent creator from being overwritten.
    libc = ctypes.CDLL(None, use_errno=True)
    if libc.renameat2(-100, os.fsencode(source), -100, os.fsencode(destination), 1):
        number = ctypes.get_errno()
        raise OSError(number, os.strerror(number))


def alias_matches(source, destination):
    return source.is_symlink() and os.readlink(source) == str(destination)


def nearest_existing(path):
    while not path.exists():
        path = path.parent
    return path


def validate_locations(entries):
    paths = [Path(entry[key]) for entry in entries for key in ("source", "destination")]
    for i, left in enumerate(paths):
        for right in paths[i + 1:]:
            if left == right or left in right.parents or right in left.parents:
                raise ValueError("Migration locations must be distinct, non-nested trees")


def plan(root, mappings):
    root = root.expanduser().resolve()
    root.mkdir(parents=True, exist_ok=True)
    state = root / ".storage"
    state.mkdir(mode=0o700, exist_ok=True)
    path = state / "migration.json"
    if present(path):
        raise ValueError("A migration already exists; use apply or rollback")
    entries = []
    for mapping in mappings:
        original, relative = mapping.split("=", 1)
        source = Path(original).expanduser().absolute()
        if source.is_symlink() or source.resolve() != source or not source.is_dir():
            raise ValueError("Source must be a canonical, regular directory")
        destination = (root / relative).resolve()
        if destination == root or root not in destination.parents or present(destination):
            raise ValueError("Destination must be an unused path within the collection")
        if source.stat().st_dev != nearest_existing(destination.parent).stat().st_dev:
            raise ValueError("Cross-filesystem moves require a separate copy migration")
        entries.append({"source": str(source), "destination": str(destination), "stage": "planned",
                        "device": source.stat().st_dev, "inode": source.stat().st_ino})
    validate_locations(entries)
    for entry in entries:
        print("Hashing", entry["source"], flush=True)
        entry["inventory"] = inventory(Path(entry["source"]))
    save(path, {"schema": 1, "strategy": "same-filesystem-rename-with-alias", "entries": entries})
    print("Plan:", path)
    return path


def migrate(path, rollback=False, only_source=None):
    path = path.expanduser().resolve()
    with (path.parent / "migration.lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        journal = json.loads(path.read_text())
        if journal["schema"] != 1 or journal["strategy"] != "same-filesystem-rename-with-alias":
            raise ValueError("Unsupported migration journal")
        validate_locations(journal["entries"])
        if only_source is not None and not any(
            entry["source"] == str(only_source.expanduser().absolute()) for entry in journal["entries"]
        ):
            raise ValueError("Requested source is not in this migration")
        entries = reversed(journal["entries"]) if rollback else journal["entries"]
        for entry in entries:
            if only_source is not None and entry["source"] != str(only_source.expanduser().absolute()):
                continue
            source, destination = Path(entry["source"]), Path(entry["destination"])
            actual = destination if present(destination) else source
            if actual.is_symlink() or (actual.stat().st_dev, actual.stat().st_ino) != (entry["device"], entry["inode"]):
                raise ValueError("Tree identity changed; migration refused")
            if rollback:
                if source.is_dir() and not source.is_symlink() and not present(destination):
                    entry["stage"] = "rolled_back"
                    save(path, journal)
                    continue
                if present(source) and not alias_matches(source, destination):
                    raise ValueError("Source is occupied; rollback refused")
                if inventory(destination) != entry["inventory"]:
                    raise ValueError("Destination changed; rollback refused")
                if source.is_symlink():
                    source.unlink()
                    sync_dir(source.parent)
                rename_exclusive(destination, source)
                sync_dir(source.parent)
                sync_dir(destination.parent)
                entry["stage"] = "rolled_back"
                save(path, journal)
                print("Restored", source, flush=True)
                continue
            if not present(destination):
                if source.is_symlink() or inventory(source) != entry["inventory"]:
                    raise ValueError("Source changed after plan; create a fresh plan")
                destination.parent.mkdir(parents=True, exist_ok=True)
                if source.stat().st_dev != destination.parent.stat().st_dev:
                    raise ValueError("Cross-filesystem move refused")
                rename_exclusive(source, destination)
                sync_dir(destination.parent)
                sync_dir(source.parent)
                entry["stage"] = "moved"
                save(path, journal)
            if present(source) and not alias_matches(source, destination):
                raise ValueError("Source occupied; existing files will not be replaced")
            if not present(source):
                source.symlink_to(destination, target_is_directory=True)
                sync_dir(source.parent)
            entry["stage"] = "linked"
            save(path, journal)
            if inventory(destination) != entry["inventory"]:
                raise ValueError("Destination verification failed; files retained for inspection")
            entry["stage"] = "verified"
            save(path, journal)
            print("Verified", destination, flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    create = commands.add_parser("plan")
    create.add_argument("--root", required=True, type=Path)
    create.add_argument("--mapping", action="append", required=True, help="ABSOLUTE_SOURCE=RELATIVE_DESTINATION")
    for command in ("apply", "rollback"):
        sub = commands.add_parser(command)
        sub.add_argument("journal", type=Path)
        sub.add_argument("--only-source", type=Path, help="Relocate or restore one cohort first")
    args = parser.parse_args()
    os.umask(0o077)
    if args.command == "plan":
        plan(args.root, args.mapping)
    else:
        migrate(args.journal, rollback=args.command == "rollback", only_source=args.only_source)


if __name__ == "__main__":
    main()
