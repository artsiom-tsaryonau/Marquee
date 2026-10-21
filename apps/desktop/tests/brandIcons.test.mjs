import assert from 'node:assert/strict';
import { test } from 'node:test';

const { brandIcon, launcherBrandIcon, sourceBrandIcon } = await import('../src/lib/brandIcons.ts');

test('brandIcon serves bundled PNG paths', () => {
  const steam = brandIcon('steam');
  assert.match(steam.src, /\/brands\/steam\.png$/);
  assert.equal(steam.tone, 'light');
  assert.equal(brandIcon('heroic').tone, 'color');
});

test('launcherBrandIcon matches detected presets', () => {
  assert.equal(launcherBrandIcon({ id: 'detected.steam', name: 'Steam', executable: 'steam' })?.src, brandIcon('steam').src);
  assert.equal(launcherBrandIcon({ id: 'detected.lutris.flatpak', name: 'Lutris (Flatpak)', executable: 'flatpak' })?.src, brandIcon('lutris').src);
  assert.equal(launcherBrandIcon({ id: 'detected.heroic', name: 'Heroic', executable: 'heroic' })?.src, brandIcon('heroic').src);
  assert.equal(launcherBrandIcon({ id: 'custom.x', name: 'My App', executable: '/opt/foo' }), undefined);
});

test('sourceBrandIcon maps managed sources', () => {
  assert.equal(sourceBrandIcon('purple').src, brandIcon('purple').src);
  assert.equal(sourceBrandIcon('lutris').src, brandIcon('lutris').src);
});
