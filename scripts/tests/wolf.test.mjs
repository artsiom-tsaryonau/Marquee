import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

test('Wolf source adaptations pin floating dependencies and strip sensitive log data; drift fails closed', () => {
  const root = mkdtempSync(join(tmpdir(), 'marquee-wolf-patch-'));
  const recipe = resolve('catalog/streaming/wolf');
  const lock = JSON.parse(readFileSync(join(recipe, 'source-lock.json'), 'utf8'));
  function put(name, text) {
    mkdirSync(join(root, name, '..'), { recursive: true });
    writeFileSync(join(root, name), text);
  }
  try {
    put('CMakeLists.txt', Object.keys(lock.cmake_tags).map(tag => `FetchContent_Declare(dependency GIT_TAG ${tag})`).join('\n'));
    put('src/moonlight-server/CMakeLists.txt', 'GIT_REPOSITORY https://github.com/games-on-whales/mdns_cpp.git)');
    put('src/moonlight-server/api/endpoints.cpp', 'logs::log(logs::warning, "Invalid event: {} - {}", req.body, event.error().what());');
    put('src/moonlight-server/api/unix_socket_server.cpp', 'logs::log(logs::debug, "Request {}", req.body);');
    put('src/moonlight-server/rest/helpers.hpp', 'logs::log(logs::trace, "Header {}", request->parse_query_string());\nlogs::log(logs::trace, "Response {}", xml_to_str(xml));');
    put('src/moonlight-server/rest/servers.cpp', 'logs::log(logs::debug, "Received POST /pin/ pin:{} secret:{}", pin, secret);\nlogs::log(logs::info, "Insert pin at http://{}:{}/pin/#{}", pair_sig->host_ip, http_port, secret);');
    put('src/moonlight-server/wolf.cpp', '  // PulseAudio sink-input router (hostname -> session_id -> virtual_sink_<session>)');
    const args = [join(recipe, 'patch-source.py'), root, join(recipe, 'source-lock.json')];
    const first = spawnSync('python3', args, { encoding: 'utf8' });
    assert.equal(first.status, 0, first.stderr);
    const patched = readFileSync(join(root, 'CMakeLists.txt'), 'utf8');
    for (const revision of Object.values(lock.cmake_tags)) assert.ok(patched.includes(revision));
    const mdns = readFileSync(join(root, 'src/moonlight-server/CMakeLists.txt'), 'utf8');
    assert.ok(mdns.includes(lock.mdns_cpp));
    for (const name of ['api/endpoints.cpp', 'api/unix_socket_server.cpp', 'rest/helpers.hpp', 'rest/servers.cpp']) {
      const text = readFileSync(join(root, 'src/moonlight-server', name), 'utf8');
      assert.doesNotMatch(text, /req\.body|event\.error|pair_sig->host_ip|parse_query_string|xml_to_str/);
    }
    assert.match(readFileSync(join(root, 'src/moonlight-server/wolf.cpp'), 'utf8'), /if \(!audio_server\)/);
    const repeated = spawnSync('python3', args, { encoding: 'utf8' });
    assert.notEqual(repeated.status, 0, 'Reapplying to unexpected/already-modified source must fail');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
