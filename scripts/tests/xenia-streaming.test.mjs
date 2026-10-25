import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';

test('Xenia private display preserves host identity and unrelated data, rejects catalogue drift', () => {
  const result = spawnSync('python3', ['-c', `
from pathlib import Path
import runpy,tempfile,tomllib
x = runpy.run_path('catalog/streaming/xenia/xenia-entrypoint.py')
inline = runpy.run_path('catalog/streaming/sonic/game-entrypoint.py')['inline']
game = runpy.run_path('catalog/streaming/xenia/xenia-game.py')
args = game['command'](Path('/state/game-saves/xenia'))
assert args[:3] == ['gamescope','--backend','sdl']
assert '--target=/games/vanquish.iso' in args
assert '--content_root=/state/game-saves/xenia/data/content' in args
assert tomllib.loads(game['CONFIG'])['HID']['Key']['keybind_back'] == 'Z'
with tempfile.TemporaryDirectory() as d:
 p = Path(d)/'config.toml'
 p.write_text('hostname="retained"\\npaired_clients=[{client_cert="retained"}]\\n[gstreamer]\\nvalue=1\\n[[profiles]]\\nid="moonlight-profile-id"\\n[[profiles.apps]]\\ntitle="Marquee streaming test"\\n')
 x['configure'](p,inline)
 before = p.read_bytes()
 parsed = tomllib.loads(p.read_text())
 assert parsed['paired_clients'] == [{'client_cert':'retained'}]
 assert parsed['hostname'] == 'retained'
 assert parsed['profiles'][0]['apps'][-1] == x['APP']
 x['configure'](p,inline)
 assert p.read_bytes() == before
 p.write_text(p.read_text().replace('Marquee streaming test','unrelated'))
 before = p.read_bytes()
 try: x['configure'](p,inline)
 except RuntimeError: pass
 else: raise AssertionError('unrelated catalogue accepted')
 assert p.read_bytes() == before
`], {encoding:'utf8'});
  assert.equal(result.status,0,result.stderr);
});
