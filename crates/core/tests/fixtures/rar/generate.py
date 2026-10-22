"""Generate tiny stored RAR fixtures containing synthetic bytes, no game data.

Layouts follow bundled UnRAR's headers.hpp/arcread.cpp. Regenerate with Python 3;
no RAR creation tools or proprietary compressor are needed.
"""
from pathlib import Path
import struct
import zlib

ROOT = Path(__file__).parent
PAYLOAD = b"synthetic cartridge bytes\n"


def v(n):
    result = bytearray()
    while n >= 128:
        result.append((n & 127) | 128)
        n >>= 7
    result.append(n)
    return bytes(result)


def block4(kind, flags, body):
    header = struct.pack("<BHH", kind, flags, 7 + len(body)) + body
    return struct.pack("<H", zlib.crc32(header) & 65535) + header


def rar4(name, payload, mode=0o100644):
    name = name.encode()
    body = struct.pack("<IIBIIBBHI", len(payload), len(payload), 3,
                       zlib.crc32(payload), 0, 20, 0x30, len(name), mode) + name
    return (b"Rar!\x1a\x07\0" + block4(0x73, 0, bytes(6)) +
            block4(0x74, 0x8000, body) + payload + block4(0x7b, 0, b""))


def block5(body):
    header = v(len(body)) + body
    return struct.pack("<I", zlib.crc32(header)) + header


def rar5(name, payload, redirect=0, directory=False):
    name = name.encode()
    extra = b""
    if redirect:
        target = b"../../outside.nds"
        record = v(5) + v(redirect) + v(int(directory)) + v(len(target)) + target
        extra = v(len(record)) + record
    body = v(2) + v(2 | bool(extra))
    if extra:
        body += v(len(extra))
    body += v(len(payload)) + v(4 | int(directory)) + v(len(payload))
    body += v(0o40755 if directory else 0o100644)
    body += struct.pack("<I", zlib.crc32(payload))
    body += v(0) + v(1) + v(len(name)) + name + extra
    return (b"Rar!\x1a\x07\x01\0" + block5(v(1) + v(0) + v(0)) +
            block5(body) + payload + block5(v(5) + v(0) + v(0)))


fixtures = {
    "regular4.rar": rar4("Game.nds", PAYLOAD),
    "regular5.rar": rar5("Game.nds", PAYLOAD),
    "symlink4.rar": rar4("Game.nds", b"../../outside.nds", 0o120777),
    "symlink5.rar": rar5("Game.nds", b"", redirect=1),
    "hardlink5.rar": rar5("Game.nds", b"", redirect=4),
    "filecopy5.rar": rar5("Game.nds", b"", redirect=5),
    "directory-link5.rar": rar5("games", b"", redirect=1, directory=True),
    "traversal5.rar": rar5("../outside.nds", PAYLOAD),
}
for name, data in fixtures.items():
    (ROOT / name).write_bytes(data)
