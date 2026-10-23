"""Report plugin availability separately from a measured encoding check."""
import json
from pathlib import Path
import subprocess


def succeeds(args):
    try:
        return subprocess.run(args, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=20).returncode == 0
    except (OSError, subprocess.TimeoutExpired):
        return False


def main():
    factories = {name: succeeds(["gst-inspect-1.0", name]) for name in [
        "waylanddisplaysrc", "x264enc", "nvh264enc", "nvh264device1enc", "vah264enc", "opusenc",
    ]}
    software = succeeds([
        "gst-launch-1.0", "-q", "videotestsrc", "num-buffers=8", "!", "videoconvert", "!",
        "x264enc", "tune=zerolatency", "speed-preset=ultrafast", "!", "fakesink",
    ])
    nvidia = None
    if factories["nvh264enc"]:
        nvidia = succeeds([
            "gst-launch-1.0", "-q", "videotestsrc", "num-buffers=8", "!", "videoconvert", "!",
            "video/x-raw,format=NV12", "!", "nvh264enc", "!", "fakesink",
        ])
    print(json.dumps({
        "factories": factories,
        "software_h264_encode_test": software,
        "nvidia_h264_encode_test": nvidia,
        "render_nodes": [str(p) for p in Path("/dev/dri").glob("renderD*")],
        "nvidia_devices": [str(p) for p in Path("/dev").glob("nvidia*")],
        "uinput_access": succeeds(["test", "-w", "/dev/uinput"]),
        "uhid_access": succeeds(["test", "-w", "/dev/uhid"]),
        "streaming_acceptance": False,
    }, sort_keys=True))
    return 0 if factories["waylanddisplaysrc"] and factories["opusenc"] and software else 1


if __name__ == "__main__":
    raise SystemExit(main())
