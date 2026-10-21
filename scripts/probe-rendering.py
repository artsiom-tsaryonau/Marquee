#!/usr/bin/env python3
"""Optional synthetic renderer comparison. Requires gjs, GTK3 and WebKit2 4.1 GI.

Opens temporary test windows with ephemeral profiles. No game/account data is read.
Not a measurement of Marquee game-library interaction or a universal FPS target.
"""
import os,subprocess,time,json,pathlib,tempfile
results=[]
for mode in ['gpu','shared-memory','software']:
 env=os.environ.copy();env['WEBKIT_DISABLE_DMABUF_RENDERER']='1' if mode=='software' else '0';env['WEBKIT_DMABUF_RENDERER_FORCE_SHM']='1' if mode=='shared-memory' else '0'
 # Keep the current display backend; do not access the Marquee profile.
 p=subprocess.Popen(['gjs', str(pathlib.Path(__file__).with_suffix('.js'))],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
 start=time.monotonic(); observations=[]
 while p.poll() is None and time.monotonic()-start<22:
  try:
   children=pathlib.Path(f'/proc/{p.pid}/task/{p.pid}/children').read_text().split()
   for pid in children:
    root=pathlib.Path('/proc')/pid
    if (root/'comm').read_text().strip()=='WebKitWebProces':
     names=[x.joinpath('comm').read_text().strip() for x in (root/'task').iterdir()]
     fds=[]
     for f in (root/'fd').iterdir():
      try:
       target=os.readlink(f)
       if target.startswith(('/dev/dri','/dev/nvidia')):fds.append(target)
      except OSError:pass
     observations.append({'pid':pid,'threads':sorted(set(names)),'graphics_fds':sorted(set(fds))})
  except (FileNotFoundError,ProcessLookupError):pass
  time.sleep(.5)
 if p.poll() is None:p.terminate()
 out,err=p.communicate(timeout=5)
 row={'mode':mode,'exit_code':p.returncode,'wall_seconds':round(time.monotonic()-start,2),'stdout':out.strip(),'stderr':err[-4000:],'renderer_observation':max(observations,key=lambda x:len(x['threads'])+len(x['graphics_fds'])) if observations else None}
 results.append(row);print(json.dumps(row),flush=True)
artifact = pathlib.Path(tempfile.mkdtemp(prefix='marquee-render-probe-')) / 'results.json'
artifact.write_text(json.dumps(results,indent=2))
print('Results: ' + str(artifact))
