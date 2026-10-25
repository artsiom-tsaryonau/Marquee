"""Publish one reviewed library launch slot in the owned, paired Wolf host."""
import fcntl
import json
import os
from pathlib import Path
import runpy
import tomllib

APP = {'title': 'Marquee library game', 'start_virtual_compositor': True,
       'start_audio_server': True,
       'runner': {'type': 'process', 'run_cmd': '/usr/bin/python3 /opt/marquee-wolf/library-game.py'}}
KNOWN = {
    'Marquee streaming test': '/bin/sleep 2147483647',
    'Marquee Sonic pilot': '/usr/bin/python3 /opt/marquee-wolf/game.py',
    'Marquee Vanquish pilot': '/usr/bin/python3 /opt/marquee-wolf/xenia-game.py',
    APP['title']: APP['runner']['run_cmd'],
}


def configure(path, inline):
    text = path.read_text()
    if len(text.encode()) > 1024 * 1024:
        raise RuntimeError('Host configuration exceeds the review limit')
    data = tomllib.loads(text)
    profiles = data.get('profiles', [])
    if len(profiles) != 1 or not isinstance(data.get('gstreamer'), dict):
        raise RuntimeError('Expected the owned single-profile Wolf host')
    apps = profiles[0].get('apps', [])
    if not apps or len({a.get('title') for a in apps}) != len(apps):
        raise RuntimeError('Invalid owned host catalogue')
    for app in apps:
        if app.get('runner') != {'type': 'process', 'run_cmd': KNOWN.get(app.get('title'))}:
            raise RuntimeError('Refusing to replace an unrelated host app')
    # Retire fixed game slots: their old inputs are not mounted in this host.
    profiles[0]['apps'] = [a for a in apps if a['title'] == 'Marquee streaming test'] + [APP]
    result = '\n'.join(json.dumps(k) + ' = ' + inline(v) for k, v in data.items()) + '\n'
    if tomllib.loads(result) != data:
        raise RuntimeError('Host configuration did not round-trip')
    if not path.with_suffix('.before-library.toml').exists():
        with path.with_suffix('.before-library.toml').open('x') as out: out.write(text)
    temporary = path.with_suffix('.library-new')
    with temporary.open('x') as out:
        out.write(result); out.flush(); os.fsync(out.fileno())
    temporary.replace(path)


if __name__ == '__main__':
    os.umask(0o077)
    with Path('/state/library-host.lock').open('a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        inline = runpy.run_path('/opt/marquee-wolf/game-entrypoint.py')['inline']
        configure(Path('/state/config.toml'), inline)
        runpy.run_path('/opt/marquee-wolf/entrypoint.py', run_name='__main__')
