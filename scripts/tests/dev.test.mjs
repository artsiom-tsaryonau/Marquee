import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { spawn } from 'node:child_process';
import { backendAddress, options } from '../dev.mjs';

const helper = new URL('../dev.mjs', import.meta.url);
const token = 'a'.repeat(64);
async function port() {
  const server = createServer();
  await new Promise(r => server.listen(0, 'localhost', r));
  const value = server.address().port;
  await new Promise(r => server.close(r));
  return value;
}
async function fixture(t) {
  const dir = mkdtempSync(join(tmpdir(), 'marquee-dev-test-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const tools = join(dir, 'tools'), data = join(dir, 'data'), calls = join(dir, 'vite.json');
  mkdirSync(tools); mkdirSync(data);
  const env = { ...process.env, PATH: `${tools}:${process.env.PATH}`, MARQUEE_DATA_DIR: data,
    MARQUEE_DEV_PORT: String(await port()), DEV_TEST_CALLS: calls, DEV_TEST_DATA: data };
  delete env.MARQUEE_API_URL;
  const tool = (name, source) => writeFileSync(join(tools, name), `#!${process.execPath}\n${source}`, { mode: 0o755 });
  tool('pnpm', `import fs from 'node:fs';
    fs.writeFileSync(process.env.DEV_TEST_CALLS,JSON.stringify({args:process.argv.slice(2),target:process.env.MARQUEE_API_URL}));`);
  const launch = async () => {
    const child = spawn(process.execPath, [helper.pathname], { env, stdio: ['ignore', 'pipe', 'pipe'] });
    t.after(() => { if (child.exitCode === null) child.kill(); });
    let output = ''; child.stdout.on('data', b => output += b); child.stderr.on('data', b => output += b);
    const timer = setTimeout(() => child.kill(), 10000);
    const code = await new Promise(r => child.on('exit', r)); clearTimeout(timer);
    return { code, output };
  };
  return { dir, data, calls, tool, launch };
}

test('dev data selection matches desktop/XDG and removes --data-dir from backend subcommand args', () => {
  const env = { HOME:'/home/test',XDG_DATA_HOME:'/data',MARQUEE_DATA_DIR:'/desktop' };
  assert.equal(options([], env).data, '/desktop');
  assert.equal(options([], {HOME:'/home/test',XDG_DATA_HOME:'/data'}).data, '/data/marquee');
  assert.deepEqual(options(['--port','48001','--data-dir','/explicit'],env).rest,['--port','48001']);
  assert.equal(options(['--data-dir','/explicit'],env).data,'/explicit');
  assert.throws(() => backendAddress('https://remote.example/#token=secret'));
  assert.throws(() => backendAddress('http://name:secret@127.0.0.1:47820/'));
});

test('dev waits for a ready backend and passes its actual port and authenticated UI address to Vite', async t => {
  const f = await fixture(t);
  f.tool('cargo', `import http from 'node:http'; import fs from 'node:fs';
    const data=process.env.DEV_TEST_DATA;
    fs.writeFileSync(data+'/server.token','${token}');
    const server=http.createServer((req,res)=>{
      res.setHeader('Content-Type','application/json');
      if(req.headers.authorization!=='Bearer ${token}'){res.writeHead(401);res.end('{}');return;}
      res.end(JSON.stringify('ok:3:'+data));
    });
    server.listen(0,'127.0.0.1',()=>console.log('Marquee web UI: http://127.0.0.1:'+server.address().port+'/#token=${token}'));`);
  const result = await f.launch(); assert.equal(result.code,0,result.output);
  const called = JSON.parse(readFileSync(f.calls,'utf8'));
  assert.match(called.target,/^http:\/\/127\.0\.0\.1:\d+$/);
  assert.deepEqual(called.args.slice(0,3),['dev','--port',called.args[2]]);
  assert.ok(called.args.includes('--open'));
  assert.match(called.args.at(-1),new RegExp(`^http://localhost:\\d+/#token=${token}$`));
  await assert.rejects(fetch(called.target,{signal:AbortSignal.timeout(1000)}));
});

test('a failed backend never starts Vite and explains an occupied desktop library', async t => {
  const f = await fixture(t); f.tool('cargo', "console.error('INSTANCE_BUSY');process.exit(1);");
  const result = await f.launch(); assert.notEqual(result.code,0);
  assert.match(result.output,/scripts\/run.sh both/);
  assert.throws(() => readFileSync(f.calls));
});

test('dev reuses only an authenticated backend for the same data directory and leaves it running', async t => {
  const f = await fixture(t);
  const server = createServer((req,res) => {
    res.setHeader('Content-Type','application/json');
    if(req.headers.authorization!==`Bearer ${token}`){res.writeHead(401);res.end('{}');return;}
    res.end(JSON.stringify(`ok:7:${f.data}`));
  });
  await new Promise(r=>server.listen(0,'127.0.0.1',r)); t.after(()=>server.close());
  // An explicit loopback origin covers a desktop backend outside the default port range.
  const origin=`http://127.0.0.1:${server.address().port}`;
  writeFileSync(join(f.data,'server.token'),token);
  f.tool('cargo', "console.error('must not start another backend');process.exit(99);");
  f.tool('pnpm', `import fs from 'node:fs';fs.writeFileSync(process.env.DEV_TEST_CALLS,JSON.stringify({target:process.env.MARQUEE_API_URL}));`);
  const child=spawn(process.execPath,[helper.pathname],{env:{...process.env,PATH:`${join(f.dir,'tools')}:${process.env.PATH}`,MARQUEE_DATA_DIR:f.data,MARQUEE_API_URL:origin,MARQUEE_DEV_PORT:String(await port()),DEV_TEST_CALLS:f.calls},stdio:'ignore'});
  const code=await new Promise(r=>child.once('exit',r)); assert.equal(code,0);
  assert.equal(JSON.parse(readFileSync(f.calls,'utf8')).target,origin);
  assert.equal((await fetch(origin,{headers:{Authorization:`Bearer ${token}`}})).status,200);
});
