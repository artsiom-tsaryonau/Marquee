import assert from 'node:assert/strict';
import { test } from 'node:test';
import { factRows } from '../src/lib/metadataFacts';

test('a review defaults only missing fields and keeps existing corrections explicit', () => {
  const rows = factRows({ year: 1999, description: 'My description', developer: 'Same developer' },
    { year: 2000, description: 'Provider description', developer: 'Same developer', publisher: 'Publisher', source: 'provider' });
  assert.deepEqual(rows.filter(row => row.changed && row.missing).map(row => row.field), ['publisher']);
  assert.equal(rows.find(row => row.field === 'year').before, 1999);
  assert.equal(rows.find(row => row.field === 'year').after, 2000);
  assert.equal(rows.find(row => row.field === 'developer').changed, false);
  assert.equal(rows.some(row => row.field === 'source'), false);
});

test('unknown provider values never offer to clear facts, and empty current strings count as missing', () => {
  const rows = factRows({ developer: '  ', players: 0 }, { developer: 'Developer', publisher: null, players: 1, genre: '' });
  assert.deepEqual(rows.map(row => row.field), ['developer', 'players']);
  assert.equal(rows[0].missing, true);
  assert.equal(rows[1].missing, false);
  assert.equal(factRows(null, { year: 2000 })[0].missing, true);
});
