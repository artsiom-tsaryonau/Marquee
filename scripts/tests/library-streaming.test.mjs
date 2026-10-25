import test from 'node:test';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';

test('library host retires fixed slots, preserves pairing and refuses unrelated apps',()=>{
  const r=spawnSync('python3',['-c',`
import runpy,tempfile,tomllib
from pathlib import Path
entry=runpy.run_path('catalog/streaming/library/library-entrypoint.py')
inline=runpy.run_path('catalog/streaming/sonic/game-entrypoint.py')['inline']
with tempfile.TemporaryDirectory() as d:
 p=Path(d)/'config.toml'
 p.write_text('hostname="retained"\\npaired_clients=[{client_cert="test"}]\\n[gstreamer]\\nvalue=1\\n[[profiles]]\\nid="moonlight-profile-id"\\n[[profiles.apps]]\\ntitle="Marquee Vanquish pilot"\\n[profiles.apps.runner]\\ntype="process"\\nrun_cmd="/usr/bin/python3 /opt/marquee-wolf/xenia-game.py"\\n')
 entry['configure'](p,inline)
 result=tomllib.loads(p.read_text())
 assert result['paired_clients']==[{'client_cert':'test'}]
 assert result['profiles'][0]['apps']==[entry['APP']]
 assert p.with_suffix('.before-library.toml').exists()
 entry['configure'](p,inline)
 p.write_text(p.read_text().replace('/opt/marquee-wolf/library-game.py','/unrelated.py'))
 before=p.read_bytes()
 try:entry['configure'](p,inline)
 except RuntimeError:pass
 else:raise AssertionError('Unrelated command replaced')
 assert p.read_bytes()==before
`],{encoding:'utf8'});
  assert.equal(r.status,0,r.stderr);
});

test('stream launch manifests cannot inject shell commands or arbitrary executable argv',()=>{
 const r=spawnSync('python3',['-c',`
import runpy,tempfile
from pathlib import Path
from unittest.mock import patch
launcher=runpy.run_path('catalog/streaming/library/library-game.py')['launch']
template=runpy.run_path('catalog/streaming/xenia/xenia-game.py')
blastem=runpy.run_path('catalog/streaming/sonic/game.py')
with tempfile.TemporaryDirectory() as d:
 root=Path(d);(root/'config').mkdir()
 with patch('runpy.run_path',return_value=template):
  args,env=launcher({'schema_version':1,'adapter':'xenia','argv':['/bin/sh','-c','touch /bad']},root)
 assert '--target=/games/game.iso' in args and '/bin/sh' not in args
 assert 'DISPLAY' not in env and env['SDL_VIDEODRIVER']=='wayland'
 sample='save_path $USERDATA/blastem/$ROMNAME\\nscreenshot_path $HOME\\nvgm_path $HOME\\n\\t\\tl ui.load_state'
 with patch('runpy.run_path',return_value=blastem),patch.object(Path,'read_text',return_value=sample):
  args,env=launcher({'schema_version':1,'adapter':'blastem'},root)
 assert env['HOME']==str(root)
 assert (root/'.config/blastem/blastem.cfg').exists()
 assert args[-1]=='/games/game.md'
 try:launcher({'schema_version':1,'adapter':'arbitrary'},root)
 except RuntimeError:pass
 else:raise AssertionError('Unknown adapter accepted')
`],{encoding:'utf8'});
 assert.equal(r.status,0,r.stderr);
});
