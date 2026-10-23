import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';

test('Sonic pilot preserves pairing and synthetic app, is idempotent, and rejects unrelated catalogues', () => {
  const result = spawnSync('python3', ['-c', `
from pathlib import Path
import runpy, tempfile, tomllib
configure = runpy.run_path('catalog/streaming/sonic/game-entrypoint.py')['configure']
configuration = runpy.run_path('catalog/streaming/sonic/game.py')['configuration']
sample = 'save_path $USERDATA/blastem/$ROMNAME\\nscreenshot_path $HOME\\nvgm_path $HOME\\n\\t\\tl ui.load_state'
out = configuration(sample)
assert 'f6 ui.save_state' in out and 'f9 ui.load_state' in out
assert '$USERDATA' not in out and '$HOME' not in out
try:
 configuration('changed upstream configuration')
except RuntimeError:
 pass
else:
 raise AssertionError('Configuration drift was accepted')
original = '''hostname = "Marquee local pilot"
config_version = 7
uuid = "retained-host"
paired_clients = [{client_cert = "test-certificate"}]
[[profiles]]
id = "moonlight-profile-id"
[[profiles.apps]]
title = "Marquee streaming test"
start_virtual_compositor = false
[profiles.apps.runner]
type = "process"
run_cmd = "/bin/sleep 2147483647"

[gstreamer]
test = "retained-encoding"
ratio = 0.5
'''
with tempfile.TemporaryDirectory() as tmp:
 p=Path(tmp)/'config.toml'
 p.write_text(original)
 configure(p)
 first=p.read_text()
 data=tomllib.loads(first)
 before=tomllib.loads(original)
 for key in ['uuid','paired_clients','hostname','gstreamer']:
  assert data[key] == before[key], key
 assert data['profiles'][0]['apps'][0] == before['profiles'][0]['apps'][0]
 assert data['profiles'][0]['apps'][1]['start_virtual_compositor'] is True
 configure(p)
 assert p.read_text() == first
 for text in [original.replace('Marquee streaming test','Other app'), first.replace('/opt/marquee-wolf/game.py','/opt/other.py'), original + 'unsupported = 1979-05-27T07:32:00Z']:
  p.write_text(text)
  try:
   configure(p)
  except RuntimeError:
   pass
  else:
   raise AssertionError('Unexpected catalogue accepted')
  assert p.read_text() == text
`], { encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
});
