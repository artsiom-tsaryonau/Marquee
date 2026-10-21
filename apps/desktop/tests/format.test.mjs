import assert from 'node:assert/strict';
import { cleanTitle, titleRegion } from '../src/lib/format.ts';

assert.equal(cleanTitle('3787 - Pokemon - Edicion Platino (Spain)'), 'Pokemon - Edicion Platino');
assert.equal(cleanTitle('Back to Earth 3D (PD) [C]'), 'Back to Earth 3D');
assert.equal(cleanTitle('Metal Gear Solid (USA) (Disc 1) (Rev 1)'), 'Metal Gear Solid (Disc 1)');
assert.equal(cleanTitle('Sonic (USA, Europe) (En,Fr,De)'), 'Sonic');
assert.equal(cleanTitle('100000 Boys (LCP2000) [PD] [C]'), '100000 Boys (LCP2000)');
assert.equal(cleanTitle('Road Wars 2000 (PD) () [x]'), 'Road Wars 2000');
assert.equal(cleanTitle('2048'), '2048');
assert.equal(cleanTitle('(USA)'), '(USA)');
assert.equal(titleRegion('Pokemon (Spain)'), 'Spain');
assert.equal(titleRegion('Back to Earth 3D (PD)'), undefined);
console.log('format ok');
