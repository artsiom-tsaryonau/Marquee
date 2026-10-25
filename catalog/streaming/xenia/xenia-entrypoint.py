"""Extend only Marquee's reviewed pilot host; preserve its pairing and Sonic app."""
import json
import os
from pathlib import Path
import runpy
import tomllib

APP = {
    'title': 'Marquee Vanquish pilot', 'start_virtual_compositor': True,
    'start_audio_server': True,
    'runner': {'type': 'process', 'run_cmd': '/usr/bin/python3 /opt/marquee-wolf/xenia-game.py'},
}


def configure(path, writer=None):
    # Same lossless writer as the existing pilot; injected only by unit tests.
    inline = writer or runpy.run_path('/opt/marquee-wolf/game-entrypoint.py')['inline']
    text = path.read_text()
    if len(text.encode()) > 1024 * 1024:
        raise RuntimeError('Host configuration exceeds the review limit')
    data = tomllib.loads(text)
    profiles = data.get('profiles', [])
    if len(profiles) != 1 or not isinstance(data.get('gstreamer'), dict):
        raise RuntimeError('Expected the reviewed single-profile pilot host')
    apps = profiles[0].get('apps', [])
    if any(a.get('title') not in ['Marquee streaming test', 'Marquee Sonic pilot', APP['title']] for a in apps):
        raise RuntimeError('Unrelated host catalogue; no changes made')
    found = [a for a in apps if a.get('title') == APP['title']]
    if found:
        if found != [APP]:
            raise RuntimeError('Incompatible existing Xenia app')
        return
    apps.append(APP)
    replacement = '\n'.join(json.dumps(k) + ' = ' + inline(v) for k, v in data.items()) + '\n'
    if tomllib.loads(replacement) != data:
        raise RuntimeError('Host configuration did not round-trip')
    temp = path.with_suffix('.xenia-new')
    with temp.open('x') as out:
        os.chmod(temp, 0o600)
        out.write(replacement); out.flush(); os.fsync(out.fileno())
    temp.replace(path)


if __name__ == '__main__':
    os.umask(0o077)
    if not Path('/games/vanquish.iso').is_file():
        raise RuntimeError('Mount the reviewed Vanquish ISO read-only')
    configure(Path('/state/config.toml'))
    runpy.run_path('/opt/marquee-wolf/entrypoint.py', run_name='__main__')
