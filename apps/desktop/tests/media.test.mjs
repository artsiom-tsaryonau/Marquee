import assert from 'node:assert/strict';
import { test } from 'node:test';
import { MediaUrls } from '../src/lib/mediaUrls.svelte';

const asset = (id, available = true) => ({ id, available });

test('healthy media resolves independently; retry requests only the failed item', async () => {
  const requests = [];
  let rejectSlow;
  const media = new MediaUrls(id => {
    requests.push(id);
    return id === 'slow' && requests.filter(value => value === id).length === 1
      ? new Promise((resolve, reject) => { rejectSlow = reject; }) : Promise.resolve(`/media/${id}`);
  });
  const first = media.load([asset('healthy'), asset('slow'), asset('missing', false)]);
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(media.urls.healthy, '/media/healthy', 'a pending or failing peer cannot hold back healthy previews');
  const concurrent = media.load([asset('healthy'), asset('slow')]);
  rejectSlow({ code: 'TEST', message: 'One item failed' });
  await Promise.all([first, concurrent]);
  assert.equal(media.errors.slow, 'One item failed');
  assert.equal(media.urls.healthy, '/media/healthy');
  assert.deepEqual(requests, ['healthy', 'slow'], 'unavailable and duplicate items are not requested');
  await media.load([asset('slow')]);
  assert.equal(requests.length, 2, 'reactive updates must not cause a retry loop');
  await media.retry('slow');
  assert.equal(media.urls.slow, '/media/slow');
  assert.equal(media.errors.slow, undefined);
  assert.deepEqual(requests, ['healthy', 'slow', 'slow']);
  media.dispose();
});

test('decode failures can retry and late replies do not update a closed gallery', async () => {
  let complete;
  const media = new MediaUrls(id => id === 'late'
    ? new Promise(resolve => { complete = resolve; }) : Promise.resolve('/media/image'));
  await media.load([asset('image')]);
  media.fail('image');
  assert.equal(media.urls.image, undefined);
  assert.match(media.errors.image, /could not load/);
  await media.retry('image');
  assert.equal(media.urls.image, '/media/image');
  const loading = media.load([asset('late')]);
  await Promise.resolve();
  media.dispose();
  complete('/media/late');
  await loading;
  assert.equal(media.urls.late, undefined);
});
