#!/usr/bin/env python3
"""Fill a port recipe's pin from one GitHub release. Prints the diff and writes the JSON. Does not commit."""

from __future__ import annotations

import argparse
import difflib
import hashlib
import io
import json
import sys
import tarfile
import urllib.request
import zipfile
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("port_id")
    parser.add_argument("tag")
    parser.add_argument("--catalog", type=Path, default=Path("catalog/ports"))
    parser.add_argument("--api-base", default="https://api.github.com")
    args = parser.parse_args()
    path = args.catalog / f"{args.port_id}.json"
    original = path.read_text()
    recipe = json.loads(original)
    release = get_json(
        f"{args.api_base.rstrip('/')}/repos/{recipe['github']}/releases/tags/{args.tag}"
    )
    asset = pick_asset(release, recipe)
    payload = download(asset["browser_download_url"])
    digest = asset.get("digest")
    actual = hashlib.sha256(payload).hexdigest()
    if digest and digest.lower() != f"sha256:{actual}":
        raise SystemExit(f"asset digest does not match the downloaded file: {digest}")
    binary = extract_binary(payload, asset["name"], recipe.get("binary"))
    if binary[:4] != b"\x7fELF" or int.from_bytes(binary[18:20], "little") != 62:
        raise SystemExit("pinned binary is not an x86-64 ELF")
    recipe["pinned_release"] = {
        "tag": args.tag,
        "asset": asset["name"],
        "sha256": actual,
        "binary_sha256": hashlib.sha256(binary).hexdigest(),
    }
    updated = json.dumps(recipe, indent=2) + "\n"
    path.write_text(updated)
    sys.stdout.writelines(
        difflib.unified_diff(
            original.splitlines(keepends=True),
            updated.splitlines(keepends=True),
            fromfile=str(path),
            tofile=str(path),
        )
    )
    return 0


def pick_asset(release: dict, recipe: dict) -> dict:
    suffix = recipe["asset_suffix"].lower()
    excludes = [item.lower() for item in recipe.get("asset_excludes", [])]
    matches = []
    for asset in release.get("assets", []):
        name = asset.get("name", "").lower()
        if name.endswith(suffix) and not any(item in name for item in excludes):
            matches.append(asset)
    if len(matches) != 1:
        raise SystemExit(f"{release.get('tag_name')} has {len(matches)} matching assets, expected 1")
    return matches[0]


def extract_binary(payload: bytes, asset_name: str, binary_name: str | None) -> bytes:
    if not binary_name:
        return payload
    files = unpack(payload, asset_name)
    nested = dict(files)
    for name, data in files.items():
        if name.lower().endswith((".zip", ".tar.gz", ".tgz")):
            nested.update(unpack(data, name))
    found = [data for name, data in nested.items() if Path(name).name == binary_name]
    if len(found) != 1:
        raise SystemExit(f"archive does not contain exactly one {binary_name}")
    return found[0]


def unpack(payload: bytes, name: str) -> dict[str, bytes]:
    lower = name.lower()
    out: dict[str, bytes] = {}
    if lower.endswith(".zip"):
        with zipfile.ZipFile(io.BytesIO(payload)) as archive:
            for info in archive.infolist():
                if info.is_dir() or ".." in Path(info.filename).parts:
                    continue
                out[info.filename] = archive.read(info)
        return out
    if lower.endswith((".tar.gz", ".tgz")):
        with tarfile.open(fileobj=io.BytesIO(payload), mode="r:gz") as archive:
            for member in archive.getmembers():
                if not member.isfile() or ".." in Path(member.name).parts:
                    continue
                extracted = archive.extractfile(member)
                if extracted is not None:
                    out[member.name] = extracted.read()
        return out
    return {name: payload}


def get_json(url: str) -> dict:
    request = urllib.request.Request(url, headers={"Accept": "application/vnd.github+json", "User-Agent": "marquee-repin"})
    with urllib.request.urlopen(request, timeout=60) as response:
        return json.load(response)


def download(url: str) -> bytes:
    request = urllib.request.Request(url, headers={"User-Agent": "marquee-repin"})
    with urllib.request.urlopen(request, timeout=120) as response:
        return response.read()


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except SystemExit:
        raise
    except Exception as error:
        raise SystemExit(str(error)) from error
