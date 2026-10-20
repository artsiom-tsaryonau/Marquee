import type { Art, Motif } from './types';

const motifs: Motif[] = ['sun', 'rings', 'peaks', 'grid', 'waves', 'shards', 'orbit', 'monolith'];
const logos: Art['logo'][] = ['wide', 'serif', 'mono', 'heavy'];

/** Stable procedural art from an edition id / title. */
export function artFromSeed(seed: string): Art {
  let h = 2166136261;
  for (let i = 0; i < seed.length; i++) {
    h ^= seed.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  const hue = (h >>> 0) % 360;
  return {
    hue,
    hue2: (hue + 40 + ((h >>> 8) % 140)) % 360,
    motif: motifs[(h >>> 4) % motifs.length],
    logo: logos[(h >>> 12) % logos.length],
  };
}
