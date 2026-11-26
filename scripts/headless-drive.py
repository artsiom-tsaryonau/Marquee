#!/usr/bin/env python3
"""Drive a game inside a private headless Gamescope for live acceptance runs.

Start Gamescope first, then point Marquee (or any launcher) at its XWayland:

    env -u DISPLAY -u WAYLAND_DISPLAY gamescope --backend headless \\
        -W 1280 -H 720 -w 1280 -h 720 -- sleep infinity
    # its log names the display: "Starting Xwayland on :N"
    env WAYLAND_DISPLAY=marquee-no-wayland SDL_VIDEODRIVER=x11 GDK_BACKEND=x11 \\
        DISPLAY=:N target/debug/marquee --data-dir DIR run web

Do not just unset WAYLAND_DISPLAY: libwayland then falls back to wayland-0, and a
Wayland-first game (sm64coopdx) opens on the physical desktop. Naming a socket
that does not exist and forcing X11 keeps every window inside Gamescope.

Commands (DISPLAY must name that XWayland):

    windows                 list top-level windows
    shot OUT.png [NAME]     save the largest mapped window, or the one whose title contains NAME
    key KEYSYM [MS] [N]     press an X keysym (x, space, Return, Up) via XTest
    click X Y | move X Y    pointer in window coordinates via Gamescope's libei socket
    close [NAME]            ask the largest window (or the one titled NAME) to close

XTest key events reach X clients here; XTest pointer events do not, so pointer
input goes through Gamescope's EIS socket, whose absolute coordinates are those
of the focused window. Needs python-xlib and Pillow (for example in a venv);
pointer input needs libei.so.1. Nothing here touches the physical desktop.
"""
import ctypes
import os
import select
import sys
import time
from pathlib import Path


def x_display():
    from Xlib import display

    if not os.environ.get("DISPLAY"):
        sys.exit("Set DISPLAY to the headless Gamescope XWayland (for example :3)")
    return display.Display()


def windows(disp):
    from Xlib import X

    root = disp.screen().root
    out = []
    for window in root.query_tree().children:
        try:
            geometry = window.get_geometry()
            mapped = window.get_attributes().map_state == X.IsViewable
            out.append((window, window.get_wm_name(), geometry.width, geometry.height, mapped))
        except Exception:
            continue
    return out


def largest(disp, name=None):
    found = [w for w in windows(disp) if w[4] and w[2] >= 160 and w[3] >= 120]
    if name:
        found = [w for w in found if name.lower() in str(w[1] or "").lower()]
    if not found:
        sys.exit("No mapped window")
    return max(found, key=lambda w: w[2] * w[3])


def shot(disp, path, title=None):
    from Xlib import X
    from PIL import Image

    window, name, width, height, _ = largest(disp, title)
    raw = window.get_image(0, 0, width, height, X.ZPixmap, 0xFFFFFFFF)
    Image.frombytes("RGBX", (width, height), raw.data, "raw", "BGRX").convert("RGB").save(path)
    print("saved", path, width, height, name)


def key(disp, keysym, hold_ms=100, repeat=1):
    from Xlib import X, XK
    from Xlib.ext import xtest

    window = largest(disp)[0]
    window.set_input_focus(X.RevertToParent, X.CurrentTime)
    disp.sync()
    code = disp.keysym_to_keycode(XK.string_to_keysym(keysym))
    if not code:
        sys.exit(f"Unknown keysym {keysym}")
    for _ in range(repeat):
        xtest.fake_input(disp, X.KeyPress, code)
        disp.sync()
        time.sleep(hold_ms / 1000)
        xtest.fake_input(disp, X.KeyRelease, code)
        disp.sync()
        time.sleep(0.25)
    print("key", keysym, "x", repeat)


def close(disp, title=None):
    from Xlib import X, protocol

    window, name = largest(disp, title)[:2]
    protocols = disp.intern_atom("WM_PROTOCOLS")
    delete = disp.intern_atom("WM_DELETE_WINDOW")
    window.send_event(
        protocol.event.ClientMessage(
            window=window, client_type=protocols, data=(32, [delete, X.CurrentTime, 0, 0, 0])
        )
    )
    disp.sync()
    print("asked to close", name)


# libei: enough of the sender API for absolute pointer motion and the left button.
CAPS = {"POINTER": 1, "POINTER_ABSOLUTE": 2, "KEYBOARD": 4, "TOUCH": 8, "SCROLL": 16, "BUTTON": 32}
BTN_LEFT = 0x110


def libei():
    lib = ctypes.CDLL("libei.so.1")
    p = ctypes.c_void_p
    for name, result, args in [
        ("ei_new_sender", p, [p]),
        ("ei_configure_name", None, [p, ctypes.c_char_p]),
        ("ei_setup_backend_socket", ctypes.c_int, [p, ctypes.c_char_p]),
        ("ei_get_fd", ctypes.c_int, [p]),
        ("ei_dispatch", None, [p]),
        ("ei_get_event", p, [p]),
        ("ei_event_get_type", ctypes.c_int, [p]),
        ("ei_event_type_to_string", ctypes.c_char_p, [ctypes.c_int]),
        ("ei_event_get_seat", p, [p]),
        ("ei_event_get_device", p, [p]),
        ("ei_event_unref", p, [p]),
        ("ei_device_has_capability", ctypes.c_bool, [p, ctypes.c_int]),
        ("ei_device_start_emulating", None, [p, ctypes.c_uint32]),
        ("ei_device_stop_emulating", None, [p]),
        ("ei_device_pointer_motion_absolute", None, [p, ctypes.c_double, ctypes.c_double]),
        ("ei_device_button_button", None, [p, ctypes.c_uint32, ctypes.c_bool]),
        ("ei_device_frame", None, [p, ctypes.c_uint64]),
        ("ei_now", ctypes.c_uint64, [p]),
    ]:
        function = getattr(lib, name)
        function.restype = result
        function.argtypes = args
    lib.ei_seat_bind_capabilities.restype = None
    return lib


def ei_socket():
    runtime = Path(os.environ["XDG_RUNTIME_DIR"])
    if os.environ.get("EI_SOCKET"):
        return runtime / os.environ["EI_SOCKET"]
    found = sorted(runtime.glob("gamescope-*-ei"))
    if len(found) != 1:
        sys.exit(f"Set EI_SOCKET; found {[f.name for f in found]} in {runtime}")
    return found[0]


def pointer(x, y, press):
    lib = libei()
    names = {}
    for number in range(120):
        text = lib.ei_event_type_to_string(number)
        if text:
            names[text.decode()] = number
    ei = lib.ei_new_sender(None)
    lib.ei_configure_name(ei, b"marquee-acceptance")
    if lib.ei_setup_backend_socket(ei, str(ei_socket()).encode()) != 0:
        sys.exit("Could not connect to Gamescope's EIS socket")
    fd = lib.ei_get_fd(ei)
    capabilities, resumed, device = {}, set(), None
    deadline = time.time() + 3
    while device is None and time.time() < deadline:
        select.select([fd], [], [], 0.1)
        lib.ei_dispatch(ei)
        while event := lib.ei_get_event(ei):
            kind = lib.ei_event_get_type(event)
            if kind == names["EI_EVENT_SEAT_ADDED"]:
                caps = [ctypes.c_int(value) for value in CAPS.values()]
                lib.ei_seat_bind_capabilities(
                    ctypes.c_void_p(lib.ei_event_get_seat(event)), *caps, ctypes.c_void_p(None)
                )
            elif kind == names["EI_EVENT_DEVICE_ADDED"]:
                found = lib.ei_event_get_device(event)
                capabilities[found] = [k for k, v in CAPS.items() if lib.ei_device_has_capability(found, v)]
            elif kind == names["EI_EVENT_DEVICE_RESUMED"]:
                resumed.add(lib.ei_event_get_device(event))
            lib.ei_event_unref(event)
        for candidate in resumed:
            if {"POINTER_ABSOLUTE", "BUTTON"} <= set(capabilities.get(candidate, [])):
                device = candidate
    if device is None:
        sys.exit("Gamescope offered no absolute pointer")
    sequence = [1]

    def emit(action):
        lib.ei_device_start_emulating(device, sequence[0])
        sequence[0] += 1
        action()
        lib.ei_device_frame(device, lib.ei_now(ei))
        lib.ei_device_stop_emulating(device)
        lib.ei_dispatch(ei)
        time.sleep(0.05)

    # Two motions so the client sees the pointer arrive before the press.
    emit(lambda: lib.ei_device_pointer_motion_absolute(device, x, y))
    time.sleep(0.2)
    emit(lambda: lib.ei_device_pointer_motion_absolute(device, x + 1, y))
    if press:
        time.sleep(0.2)
        emit(lambda: lib.ei_device_button_button(device, BTN_LEFT, True))
        time.sleep(0.1)
        emit(lambda: lib.ei_device_button_button(device, BTN_LEFT, False))
    time.sleep(0.3)
    print("click" if press else "move", x, y)


def main(argv):
    if not argv or argv[0] in ("-h", "--help"):
        print(__doc__)
        return
    command, args = argv[0], argv[1:]
    if command in ("click", "move"):
        pointer(float(args[0]), float(args[1]), command == "click")
        return
    disp = x_display()
    if command == "windows":
        for window, name, width, height, mapped in windows(disp):
            print(hex(window.id), repr(name), width, height, "mapped" if mapped else "")
    elif command == "shot":
        shot(disp, args[0], args[1] if len(args) > 1 else None)
    elif command == "key":
        key(disp, args[0], *(int(value) for value in args[1:3]))
    elif command == "close":
        close(disp, args[0] if args else None)
    else:
        sys.exit(f"Unknown command {command}")


if __name__ == "__main__":
    main(sys.argv[1:])
