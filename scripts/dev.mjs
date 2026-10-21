// Coordinate the authenticated core and Vite; never start an empty UI after a
// failed core launch, and never stop an existing desktop/web backend we reuse.
import { spawn } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import { createServer } from 'node:net';
import { resolve, join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');

export function backendAddress(value) {
  const url = new URL(value);
  if (url.protocol !== 'http:' || !['127.0.0.1', 'localhost'].includes(url.hostname)
      || !url.port || url.username || url.password || url.pathname !== '/' || url.search) {
    throw new Error('Marquee dev requires a loopback HTTP backend URL.');
  }
  return url;
}

export function options(args, env = process.env) {
  const rest = [...args];
  const at = rest.indexOf('--data-dir');
  if (at >= 0 && !rest[at + 1]) throw new Error('--data-dir needs a path.');
  const data = at >= 0 ? rest.splice(at, 2)[1] : env.MARQUEE_DATA_DIR;
  const port = Number(env.MARQUEE_DEV_PORT || 1420);
  if (!Number.isInteger(port) || port < 1 || port > 65535) throw new Error('Invalid MARQUEE_DEV_PORT.');
  return {
    data: resolve(data || join(env.XDG_DATA_HOME || join(env.HOME, '.local/share'), 'marquee')),
    port, rest,
  };
}

async function healthy(origin, token, data) {
  try {
    const res = await fetch(new URL('/api/health', origin), {
      method: 'POST', headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
      body: '{}', signal: AbortSignal.timeout(1500), redirect: 'error',
    });
    const health = res.ok ? await res.json() : null;
    const match = typeof health === 'string' && /^ok:\d+:(.*)$/.exec(health);
    return !!match && resolve(match[1]) === data;
  } catch { return false; }
}

async function existingBackend(config, env) {
  const token = await readFile(join(config.data, 'server.token'), 'utf8').then(s => s.trim()).catch(() => '');
  if (token.length < 32) return null;
  const baseAt = config.rest.indexOf('--port');
  const base = baseAt < 0 ? 47820 : Number(config.rest[baseAt + 1]);
  const candidates = env.MARQUEE_API_URL ? [backendAddress(env.MARQUEE_API_URL)]
    : Array.from({ length: 10 }, (_, i) => new URL(`http://127.0.0.1:${base + i}/`));
  const checks = await Promise.all(candidates.map(async url => {
    if (!await healthy(url.origin, token, config.data)) return null;
    url.hash = new URLSearchParams({ token }).toString();
    return url;
  }));
  return checks.find(Boolean) || null;
}

export async function main(args = process.argv.slice(2), env = process.env) {
  const config = options(args, env);
  // Fail before starting a core when an old Vite still owns the dev address.
  await new Promise((resolveReady, reject) => {
    const probe = createServer();
    probe.once('error', () => reject(new Error(`Port ${config.port} is already in use. Stop the old Vite server first.`)));
    probe.listen(config.port, 'localhost', () => probe.close(resolveReady));
  });
  const owned = new Set();
  const start = (command, argv, cwd, childEnv, stdio) => {
    const child = spawn(command, argv, { cwd, env: childEnv, stdio, detached: true });
    owned.add(child);
    child.once('exit', () => owned.delete(child));
    return child;
  };
  const stop = () => {
    for (const child of owned) {
      try { process.kill(-child.pid, 'SIGTERM'); } catch { /* already exited */ }
    }
  };
  const interrupted = () => { stop(); process.exitCode = 130; };
  process.once('SIGINT', interrupted);
  process.once('SIGTERM', interrupted);
  try {
    let backend = await existingBackend(config, env);
    let core;
    if (!backend) {
      core = start('cargo', ['run', '-q', '-p', 'marquee-cli', '--', '--data-dir', config.data, 'run', 'web', ...config.rest], root, env, ['inherit', 'pipe', 'inherit']);
      backend = await new Promise((ready, reject) => {
        let buffer = '';
        const timer = setTimeout(() => { stop(); reject(new Error('Marquee backend startup timed out.')); }, 300000);
        const fail = code => { clearTimeout(timer); reject(new Error(`Marquee backend exited (${code}). If desktop holds this library, restart it with scripts/run.sh both, then retry dev.`)); };
        core.once('error', error => { clearTimeout(timer); reject(error); });
        core.once('exit', fail);
        core.stdout.setEncoding('utf8');
        core.stdout.on('data', chunk => {
          process.stdout.write(chunk);
          buffer += chunk;
          const lines = buffer.split('\n');
          buffer = lines.pop();
          for (const line of lines) {
            if (!line.startsWith('Marquee web UI: ')) continue;
            try {
              const url = backendAddress(line.slice('Marquee web UI: '.length).trim());
              if ((new URLSearchParams(url.hash.slice(1)).get('token') || '').length < 32) throw new Error('Backend returned no authentication token.');
              clearTimeout(timer); core.off('exit', fail); ready(url);
            } catch (error) { clearTimeout(timer); reject(error); }
          }
        });
      });
      const token = new URLSearchParams(backend.hash.slice(1)).get('token');
      if (!await healthy(backend.origin, token, config.data)) throw new Error('Marquee backend did not become ready for this library.');
    } else {
      console.log('Reusing the running Marquee backend for this library.');
    }
    if (core && core.exitCode !== null) throw new Error('Marquee backend stopped before Vite could start.');
    const address = `http://localhost:${config.port}/${backend.hash}`;
    console.log(`Marquee dev UI: ${address}`);
    const vite = start('pnpm', ['dev', '--port', String(config.port), '--open', address], join(root, 'apps/desktop'),
      { ...env, MARQUEE_API_URL: backend.origin }, 'inherit');
    core?.once('exit', () => {
      if (owned.has(vite)) { console.error('Marquee backend stopped; stopping Vite.'); process.exitCode = 1; stop(); }
    });
    const code = await new Promise((done, reject) => { vite.once('exit', done); vite.once('error', reject); });
    if (!process.exitCode) process.exitCode = code ?? 1;
  } finally {
    process.off('SIGINT', interrupted); process.off('SIGTERM', interrupted); stop();
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch(error => { console.error(error.message); process.exitCode = 1; });
}
