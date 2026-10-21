import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, copyFileSync, rmSync, utimesSync, unlinkSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), 'marquee-launch-test-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  for (const dir of ['scripts', 'tools', 'apps/desktop/src', 'crates/core', 'catalog']) mkdirSync(join(root, dir), { recursive: true });
  const script = join(root, 'scripts/run.sh');
  copyFileSync(new URL('../run.sh', import.meta.url), script);
  for (const file of ['Cargo.toml', 'Cargo.lock']) writeFileSync(join(root, file), 'fixture');
  const source = join(root, 'apps/desktop/src/component with spaces.svelte');
  writeFileSync(source, 'fixture');
  const log = join(root, 'calls.jsonl');
  const target = join(root, 'target output');
  const common = `import fs from 'node:fs'; import path from 'node:path'; const record = (value) => fs.appendFileSync(process.env.LAUNCH_TEST_LOG, JSON.stringify(value) + '\\n');`;
  const tool = (name, body) => writeFileSync(join(root, 'tools', name), `#!${process.execPath}\n${common}\n${body}`, { mode: 0o755 });
  tool('cargo', `record({kind:'cargo'}); process.stdout.write(JSON.stringify({target_directory: process.env.LAUNCH_TEST_TARGET}));`);
  tool('pnpm', `record({kind:'pnpm', args:process.argv.slice(2)});
    if(process.env.LAUNCH_TEST_FAIL) process.exit(1);
    const binary = path.join(process.env.LAUNCH_TEST_TARGET, 'release/marquee-desktop');
    fs.mkdirSync(path.dirname(binary), {recursive:true});
    fs.writeFileSync(binary, ${JSON.stringify(`#!${process.execPath}
${common}
record({kind:'run',web:process.env.MARQUEE_WEB ?? null,args:process.argv.slice(2),disable:process.env.WEBKIT_DISABLE_DMABUF_RENDERER ?? null});
`)}, {mode:0o755});`);
  const env = { ...process.env, PATH: `${join(root, 'tools')}:${process.env.PATH}`, LAUNCH_TEST_LOG: log, LAUNCH_TEST_TARGET: target };
  delete env.WEBKIT_DISABLE_DMABUF_RENDERER;
  delete env.MARQUEE_REBUILD;
  delete env.MARQUEE_WEB;
  delete env.NODE_TEST_CONTEXT;
  const run = (mode, args = [], extra = {}) => spawnSync('/bin/bash', [script, mode, ...args], { env: { ...env, ...extra }, encoding: 'utf8' });
  const calls = () => readFileSync(log, 'utf8').trim().split('\n').filter(Boolean).map(JSON.parse);
  return { root, source, target, run, calls };
}

test('desktop builds an optimized embedded app, preserves argv and does not disable acceleration', (t) => {
  const f = fixture(t);
  const result = f.run('desktop', ['a path with spaces']);
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(f.calls().find((c) => c.kind === 'pnpm').args, ['tauri', 'build', '--no-bundle']);
  assert.deepEqual(f.calls().at(-1), { kind: 'run', web: null, args: ['a path with spaces'], disable: null });
});

test('an unchanged production binary is reused and both explicitly enables web access', (t) => {
  const f = fixture(t);
  assert.equal(f.run('desktop').status, 0);
  const before = f.calls().filter((c) => c.kind === 'pnpm').length;
  assert.equal(f.run('both').status, 0);
  assert.equal(f.calls().filter((c) => c.kind === 'pnpm').length, before);
  assert.equal(f.calls().at(-1).web, '1');
});

test('changed or deleted source inputs and explicit rebuilds invalidate the cached executable', (t) => {
  const f = fixture(t);
  assert.equal(f.run('desktop').status, 0);
  const future = new Date(Date.now() + 1000);
  utimesSync(f.source, future, future);
  assert.equal(f.run('desktop').status, 0);
  assert.equal(f.calls().filter((c) => c.kind === 'pnpm').length, 2);
  unlinkSync(f.source);
  assert.equal(f.run('desktop').status, 0);
  assert.equal(f.calls().filter((c) => c.kind === 'pnpm').length, 3);
  assert.equal(f.run('desktop', [], { MARQUEE_REBUILD: '1' }).status, 0);
  assert.equal(f.calls().filter((c) => c.kind === 'pnpm').length, 4);
});

test('failed builds never launch a missing or stale binary', (t) => {
  const f = fixture(t);
  assert.notEqual(f.run('desktop', [], { LAUNCH_TEST_FAIL: '1' }).status, 0);
  assert.equal(f.calls().some((c) => c.kind === 'run'), false);
});
