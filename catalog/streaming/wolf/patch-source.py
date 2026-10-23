"""Strict local adaptations for the pinned Wolf source; fail on drift."""
import json
from pathlib import Path
import re
import sys


def replace(path, old, new, count=1):
    text = path.read_text()
    if text.count(old) != count:
        raise RuntimeError(f"Unexpected pinned source at {path}: expected {count} matching contexts")
    path.write_text(text.replace(old, new))


def patch(root, lock):
    files = list(root.rglob("CMakeLists.txt"))
    for old, revision in lock["cmake_tags"].items():
        count = 0
        for path in files:
            text = path.read_text()
            text, hits = re.subn(r"GIT_TAG " + re.escape(old) + r"(?=[\s)])", "GIT_TAG " + revision, text)
            if hits:
                path.write_text(text)
                count += hits
        if not count:
            raise RuntimeError(f"Missing pinned CMake dependency: {old}")
    replace(root / "src/moonlight-server/CMakeLists.txt",
            "GIT_REPOSITORY https://github.com/games-on-whales/mdns_cpp.git)",
            "GIT_REPOSITORY https://github.com/games-on-whales/mdns_cpp.git\n        GIT_TAG " + lock["mdns_cpp"] + ")")

    # Request/error bodies can contain PINs, certificates and encryption keys.
    # Keep levels and static context while omitting all dynamic API log fields.
    for relative in ["api/endpoints.cpp", "api/unix_socket_server.cpp", "rest/helpers.hpp"]:
        path = root / "src/moonlight-server" / relative
        text = path.read_text()
        def redact(match):
            level = re.search(r"logs::(trace|debug|info|warning|error)", match[0])
            if not level:
                raise RuntimeError("Unexpected logging expression")
            return 'logs::log(logs::' + level[1] + ', "[API] Event details omitted by Marquee");'
        text, hits = re.subn(r"logs::log\(.*?\);", redact, text, flags=re.S)
        if not hits:
            raise RuntimeError(f"Missing API log context: {relative}")
        path.write_text(text)
    servers = root / "src/moonlight-server/rest/servers.cpp"
    replace(servers, 'logs::log(logs::debug, "Received POST /pin/ pin:{} secret:{}", pin, secret);',
            'logs::log(logs::debug, "Received pairing PIN; details omitted");')
    replace(servers, 'logs::log(logs::info, "Insert pin at http://{}:{}/pin/#{}", pair_sig->host_ip, http_port, secret);',
            'logs::log(logs::info, "Pairing request pending; accept through Marquee");')
    # An unavailable audio server otherwise dereferences an empty optional.
    replace(root / "src/moonlight-server/wolf.cpp",
            "  // PulseAudio sink-input router (hostname -> session_id -> virtual_sink_<session>)",
            '  if (!audio_server) { throw std::runtime_error("Marquee streaming audio server is unavailable"); }\n'
            "  // PulseAudio sink-input router (hostname -> session_id -> virtual_sink_<session>)")


if __name__ == "__main__":
    patch(Path(sys.argv[1]), json.loads(Path(sys.argv[2]).read_text()))
