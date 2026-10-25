#!/usr/bin/env python3
"""Explicit Vanquish private-display pilot. Does not change library launch defaults."""
import argparse
import hashlib
import json
import os
import re
from pathlib import Path
import shutil
import subprocess
import tempfile
import tomllib
import urllib.request

REPO = Path(__file__).resolve().parent.parent
REVISION = '451f915566d57436555c6eb4ee77bb4717eb01de'
DIGEST = 'b96b2723b0af55b5c4d837916e12035f172fe9ebbab2e90ba6c64f2f21fd5d8b'
NAME = 'marquee-wolf-xenia-pilot'


def rootless():
    info = json.loads(subprocess.check_output(['podman','info','--format=json']))
    if info.get('host',{}).get('security',{}).get('rootless') is not True:
        raise RuntimeError('This streaming pilot requires rootless Podman')


def recipe_hash():
    digest = hashlib.sha256()
    for source in sorted((REPO / 'catalog/streaming/xenia').iterdir()):
        if source.is_file():
            digest.update(source.name.encode()); digest.update(b'\0'); digest.update(source.read_bytes())
    return digest.hexdigest()


def build(args):
    receipt = json.loads(args.base_receipt.read_text())
    image = receipt['image']
    if not re.fullmatch(r'sha256:[0-9a-f]{64}', image) or receipt.get('schema_version') != 1 or not receipt.get('blastem_image'):
        raise RuntimeError('Use the reviewed Sonic/Wolf image receipt')
    workroot = args.data / 'streaming/xenia-pilot'
    workroot.mkdir(parents=True, exist_ok=True, mode=0o700)
    with tempfile.TemporaryDirectory(prefix='.build-', dir=workroot) as work:
        work = Path(work)
        app = work / 'xenia.AppImage'
        url = 'https://github.com/xenia-canary/xenia-canary/releases/download/451f915/xenia_canary_linux.AppImage'
        with urllib.request.urlopen(url, timeout=60) as response, app.open('wb') as out:
            shutil.copyfileobj(response, out)
        if hashlib.sha256(app.read_bytes()).hexdigest() != DIGEST:
            raise RuntimeError('Pinned Xenia download checksum mismatch')
        app.chmod(0o700)
        with (workroot / 'build.log').open('w') as log:
            subprocess.run([str(app), '--appimage-extract'], cwd=work, stdout=log, stderr=subprocess.STDOUT, check=True)
            (work / 'squashfs-root').rename(work / 'xenia')
            app.unlink()
            for source in (REPO / 'catalog/streaming/xenia').iterdir():
                if source.is_file(): shutil.copyfile(source, work / source.name)
            license_url = f'https://raw.githubusercontent.com/xenia-canary/xenia-canary/{REVISION}/LICENSE'
            with urllib.request.urlopen(license_url, timeout=30) as response:
                (work / 'xenia/LICENSE').write_bytes(response.read(1024 * 1024))
            subprocess.run(['podman','build','--pull=never','--force-rm','--layers','--iidfile',str(work / 'image-id'),
                            '--build-arg','HOST_IMAGE=' + image,str(work)], stdout=log,stderr=subprocess.STDOUT,check=True)
        result = {'schema_version':1,'image':(work / 'image-id').read_text().strip(),
                  'base_image':image,'xenia_revision':REVISION,'xenia_sha256':DIGEST,
                  'recipe_sha256':recipe_hash(),'live_streaming_verified':False}
        (workroot / 'current.json').write_text(json.dumps(result,indent=2))
        print(json.dumps(result))


def start(args):
    source = args.iso.resolve(strict=True)
    # Read-only inspection verifies the game format and identity, not just its name.
    result = subprocess.run([str(REPO / 'target/release/marquee'),'console-inspect','--system','xbox360',str(source)],capture_output=True,text=True,check=True)
    disc = json.loads(result.stdout)
    if disc.get('xbox360',{}).get('title_id') != '53450841':
        raise RuntimeError('This acceptance pilot is reviewed for Vanquish title 53450841 only')
    state = (args.data / 'streaming/wolf/state').resolve(strict=True)
    if not (state / 'config.toml').is_file():
        raise RuntimeError('Set up and pair the private Wolf pilot first')
    catalogue = tomllib.loads((state / 'config.toml').read_text())
    sonic = any(a.get('title') == 'Marquee Sonic pilot' for p in catalogue.get('profiles', []) for a in p.get('apps', []))
    if sonic and not args.sonic:
        raise RuntimeError('Pass --sonic to preserve the existing Sonic app input')
    receipt = json.loads((args.data / 'streaming/xenia-pilot/current.json').read_text())
    image = receipt['image']
    if not re.fullmatch(r'sha256:[0-9a-f]{64}', image) or receipt.get('xenia_sha256') != DIGEST or receipt.get('recipe_sha256') != recipe_hash():
        raise RuntimeError('Build the current reviewed streaming recipe first')
    inspect = subprocess.run([str(REPO / 'target/release/marquee'),'--data-dir',str(args.data),'streaming','--socket',str(state / 'runtime/wolf.sock'),'inspect'], stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
    if inspect.returncode == 0:
        raise RuntimeError('The paired Wolf host is already running; stop its owned session before replacing it')
    config = state / 'game-config'
    config.mkdir(exist_ok=True, mode=0o700)
    cmd = ['podman','run','--rm','--name',NAME,'--pull=never','--userns=keep-id',
           '--cap-drop=all','--security-opt=no-new-privileges','--security-opt=label=disable',
           '--device=nvidia.com/gpu=all','--device=' + args.render_node,
           '--volume',f'{state}:/state:rw','--volume',f'{source}:/games/vanquish.iso:ro',
           '--volume',f'{config}:/home/retro/.config:rw',
           '--env','WOLF_RENDER_NODE=' + args.render_node,'--env','WOLF_ENCODER_NODE=' + args.render_node,
           '--env','WOLF_USE_ZERO_COPY=FALSE']
    if args.sonic:
        cmd += ['--volume',f'{args.sonic.resolve(strict=True)}:/games/sonic.md:ro']
    # Keep the paired host identity, but do not mount any physical desktop socket.
    for port,proto in [(47984,'tcp'),(47989,'tcp'),(48010,'tcp'),(47999,'udp'),(48100,'udp'),(48200,'udp')]:
        cmd += ['--publish',f'127.0.0.1:{port}:{port}/{proto}']
    with (args.data / 'streaming/xenia-pilot/host.log').open('w') as log:
        subprocess.run(cmd + [image],stdout=log,stderr=subprocess.STDOUT,check=True)


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--data',type=Path,default=REPO / 'data/streaming-pilot')
    sub = parser.add_subparsers(dest='action',required=True)
    b = sub.add_parser('build')
    b.add_argument('--base-receipt',type=Path,default=REPO / 'data/streaming/st2b-image.json')
    s = sub.add_parser('start')
    s.add_argument('--iso',type=Path,required=True)
    s.add_argument('--render-node',required=True)
    s.add_argument('--sonic',type=Path,help='Retain the existing Sonic pilot input, read-only')
    sub.add_parser('stop')
    args = parser.parse_args()
    args.data = args.data.resolve()
    rootless()
    if args.action == 'build': build(args)
    elif args.action == 'start': start(args)
    else:
        label = subprocess.check_output(['podman','inspect','--format','{{ index .Config.Labels "io.marquee.integration" }}',NAME],text=True).strip()
        if label != 'xenia-stream-pilot-v1':
            raise RuntimeError('Refusing to stop a container not owned by this pilot')
        subprocess.run(['podman','stop',NAME],check=True)


if __name__ == '__main__':
    main()
