"""Xenia runs behind nested Gamescope/XWayland, never the physical desktop."""
import fcntl
import json
import os
from pathlib import Path
import subprocess

CONFIG = '''[GPU]
gpu = "vulkan"
[General]
discord = false
[HID]
keyboard_mode = 1
[HID.Key]
keybind_start = "0x0D"
keybind_back = "Z"
keybind_a = "J"
keybind_b = "K"
keybind_x = "U"
keybind_y = "I"
keybind_right_thumb = "H"
keybind_left_trigger = "Q"
keybind_right_trigger = "E"
'''


def command(root):
    return ['gamescope', '--backend', 'sdl', '-W', '1280', '-H', '720', '-w', '1280', '-h', '720',
            '-r', '60', '--', '/usr/bin/env', 'GDK_BACKEND=x11', '/opt/xenia/usr/bin/xenia_canary', '--target=/games/vanquish.iso',
            '--fullscreen=true', '--storage_root=' + str(root / 'data'),
            '--content_root=' + str(root / 'data/content'), '--cache_root=' + str(root / 'cache'),
            '--config=' + str(root / 'config/xenia-canary.config.toml'),
            '--log_file=' + str(root / 'cache/xenia.log')]


def main():
    os.umask(0o077)
    root = Path('/state/game-saves/xenia')
    for part in ['data/content', 'cache', 'config']:
        (root / part).mkdir(parents=True, exist_ok=True, mode=0o700)
    with (root / 'writer.lock').open('a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        runtime = Path(os.environ['XDG_RUNTIME_DIR'])
        display = os.environ['WAYLAND_DISPLAY']
        if '/' in display or not (runtime / display).is_socket():
            raise RuntimeError('Private Wolf display unavailable')
        if not os.environ.get('PULSE_SINK') or not os.environ.get('PULSE_SERVER'):
            raise RuntimeError('Private Wolf audio route unavailable')
        config = root / 'config/xenia-canary.config.toml'
        if not config.exists():
            with config.open('x') as out: out.write(CONFIG)
        env = os.environ.copy()
        env.pop('DISPLAY', None)
        env.pop('XAUTHORITY', None)
        env.pop('GDK_BACKEND', None)
        env.update(SDL_VIDEODRIVER='wayland', SDL_AUDIODRIVER='pulseaudio',
                   XDG_CONFIG_HOME=str(root / 'config'), XDG_DATA_HOME=str(root / 'data'),
                   XDG_CACHE_HOME=str(root / 'cache'))
        with (root / 'last-launch.log').open('w') as log:
            result = subprocess.run(command(root), env=env, stdout=log, stderr=subprocess.STDOUT, check=False)
        (root / 'last-exit.json').write_text(json.dumps({'exit_code': result.returncode}))
        return result.returncode


if __name__ == '__main__':
    raise SystemExit(main())
