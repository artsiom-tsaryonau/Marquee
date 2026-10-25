"""Reviewed adapter commands only. Manifests never contain executable argv."""
import fcntl
import json
import os
from pathlib import Path
import runpy
import signal
import subprocess


def launch(manifest, root):
    if manifest.get('schema_version') != 1:
        raise RuntimeError('Unsupported launch plan')
    adapter = manifest.get('adapter')
    env = os.environ.copy()
    env.pop('DISPLAY', None); env.pop('XAUTHORITY', None)
    env.update(XDG_CONFIG_HOME=str(root / 'config'), XDG_DATA_HOME=str(root / 'data'),
               XDG_CACHE_HOME=str(root / 'cache'), SDL_AUDIODRIVER='pulseaudio')
    config = root / 'config'
    if adapter == 'xenia':
        template = runpy.run_path('/opt/marquee-wolf/xenia-game.py')
        file = config / 'xenia-canary.config.toml'
        if not file.exists():
            with file.open('x') as out: out.write(template['CONFIG'])
        argv = template['command'](root)
        old = '--target=/games/vanquish.iso'
        if argv.count(old) != 1:
            raise RuntimeError('Pinned Xenia launch contract changed')
        argv[argv.index(old)] = '--target=/games/game.iso'
        env.pop('GDK_BACKEND', None)
        env['SDL_VIDEODRIVER'] = 'wayland'
    elif adapter == 'blastem':
        template = runpy.run_path('/opt/marquee-wolf/game.py')
        config = root / '.config/blastem'; config.mkdir(parents=True, exist_ok=True)
        (root / '.local/share/blastem').mkdir(parents=True, exist_ok=True)
        file = config / 'blastem.cfg'
        if not file.exists():
            text = template['configuration'](Path('/opt/blastem/default.cfg').read_text())
            text = text.replace('/state/game-saves/sonic', str(root / 'data'))
            with file.open('x') as out: out.write(text)
        env.update(HOME=str(root), SDL_VIDEODRIVER='wayland', SDL_RENDER_DRIVER='software')
        argv = ['/opt/blastem/blastem', '-g', '-f', '/games/game.md']
    else:
        raise RuntimeError('No reviewed streaming adapter for this runner')
    return argv, env


def main():
    os.umask(0o077)
    path = Path('/launch.json')
    if path.stat().st_size > 16384:
        raise RuntimeError('Launch plan exceeds the review limit')
    manifest = json.loads(path.read_text())
    root = Path('/profile')
    for part in ['config', 'data/content', 'cache']:
        (root / part).mkdir(parents=True, exist_ok=True, mode=0o700)
    with (root / 'writer.lock').open('a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        runtime = Path(os.environ['XDG_RUNTIME_DIR']); display = os.environ['WAYLAND_DISPLAY']
        if '/' in display or not (runtime / display).is_socket():
            raise RuntimeError('Private Wolf display is unavailable')
        if not os.environ.get('PULSE_SINK') or not os.environ.get('PULSE_SERVER'):
            raise RuntimeError('Private Wolf audio route is unavailable')
        argv, env = launch(manifest, root)
        with (root / 'last-launch.log').open('w') as log:
            child = subprocess.Popen(argv, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
            def stop(_sig, _frame):
                if child.poll() is None: os.killpg(child.pid, signal.SIGTERM)
            signal.signal(signal.SIGTERM, stop); signal.signal(signal.SIGINT, stop)
            code = child.wait()
        (root / 'last-exit.json').write_text(json.dumps({'launch_id': manifest['launch_id'], 'exit_code': code}))
        return code


if __name__ == '__main__':
    raise SystemExit(main())
