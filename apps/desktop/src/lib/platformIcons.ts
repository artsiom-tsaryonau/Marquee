import type { Platform } from './types';

type Hardware = 'handheld' | 'wide-handheld' | 'dual-screen' | 'nes' | 'snes' | 'n64' | 'genesis' | 'ps1' | 'ps2' | 'ps3' | 'dreamcast' | 'cube' | 'tower' | 'xbox' | 'arcade' | 'pc' | 'console' | 'headset';
export type PlatformIconStyle = { hardware: Hardware; shell: string; accent: string; screen?: string };
const styles: Partial<Record<Platform, PlatformIconStyle>> = {
  gb: { hardware: 'handheld', shell: '#d2d1bb', accent: '#a84b77', screen: '#8d9f59' },
  gbc: { hardware: 'handheld', shell: '#7061c4', accent: '#75dfcc', screen: '#b6dba9' },
  gba: { hardware: 'wide-handheld', shell: '#7164ba', accent: '#e4def9' },
  gamegear: { hardware: 'wide-handheld', shell: '#343e52', accent: '#6dbeed' },
  lynx: { hardware: 'wide-handheld', shell: '#3d424a', accent: '#edb855' },
  psp: { hardware: 'wide-handheld', shell: '#353c4a', accent: '#b7c8e2' },
  ds: { hardware: 'dual-screen', shell: '#c8d5e5', accent: '#83d3df' },
  '3ds': { hardware: 'dual-screen', shell: '#579bc6', accent: '#b8e4fb' },
  nes: { hardware: 'nes', shell: '#d3d6dd', accent: '#ed626c' },
  snes: { hardware: 'snes', shell: '#c4c5d5', accent: '#9b83d2' },
  n64: { hardware: 'n64', shell: '#4a5265', accent: '#e4b95d' },
  genesis: { hardware: 'genesis', shell: '#343d4e', accent: '#ea717a' },
  sms: { hardware: 'nes', shell: '#394456', accent: '#ed626c' },
  ps1: { hardware: 'ps1', shell: '#c5cddb', accent: '#7fcaad' },
  ps2: { hardware: 'ps2', shell: '#435580', accent: '#79c9ff' },
  ps3: { hardware: 'ps3', shell: '#444b60', accent: '#a7bbdb' },
  dreamcast: { hardware: 'dreamcast', shell: '#e4e6dd', accent: '#ec9d68' },
  gamecube: { hardware: 'cube', shell: '#8e7dc9', accent: '#c9baf1' },
  wii: { hardware: 'tower', shell: '#e1e8ee', accent: '#83d3ec' },
  wiiware: { hardware: 'tower', shell: '#e1e8ee', accent: '#83d3ec' },
  wiiu: { hardware: 'wide-handheld', shell: '#49546a', accent: '#7ccee6' },
  xbox: { hardware: 'xbox', shell: '#3b4c47', accent: '#85cf76' },
  xbox360: { hardware: 'tower', shell: '#d4e0d3', accent: '#85c47e' },
  'xbox360-digital': { hardware: 'tower', shell: '#d4e0d3', accent: '#85c47e' },
  neogeo: { hardware: 'arcade', shell: '#b66464', accent: '#f0c76c' },
  atari2600: { hardware: 'console', shell: '#8f6a50', accent: '#edbd72' },
  atari5200: { hardware: 'console', shell: '#485169', accent: '#bfcee5' },
  atari7800: { hardware: 'console', shell: '#51566a', accent: '#e2a56e' },
  saturn: { hardware: 'ps1', shell: '#505a6e', accent: '#a9b7e4' },
  segacd: { hardware: 'genesis', shell: '#414d65', accent: '#8fafd7' },
  '32x': { hardware: 'genesis', shell: '#3d465b', accent: '#d8b56b' },
  tg16: { hardware: 'console', shell: '#4c5265', accent: '#d68d79' },
  tgcd: { hardware: 'dreamcast', shell: '#d3d1c2', accent: '#96b8c5' },
  cdi: { hardware: 'console', shell: '#485467', accent: '#8cc3d6' },
  jaguar: { hardware: 'n64', shell: '#3c4456', accent: '#e37c7c' },
  jaguarcd: { hardware: 'ps1', shell: '#40495c', accent: '#e37c7c' },
  virtualboy: { hardware: 'headset', shell: '#ac5c65', accent: '#e1a3a8' },
  'pc-linux': { hardware: 'pc', shell: '#758caf', accent: '#8ed6bd' },
  'pc-windows': { hardware: 'pc', shell: '#758caf', accent: '#8cc8f3' },
};

export function platformIconStyle(platform: string): PlatformIconStyle {
  return (Object.hasOwn(styles, platform) ? styles[platform as Platform] : undefined) ?? (platform === 'pc' ? styles['pc-linux']! : { hardware: 'console', shell: '#69778d', accent: '#b5c2d9' });
}

/** A decorative identifier for missing artwork, never a claimed game logo. */
export function gameInitials(title: string): string {
  const words = title.replace(/\([^)]*\)|\[[^\]]*\]/g, '').match(/[\p{L}\p{N}]+/gu) ?? [];
  const meaningful = words.filter(word => !['the', 'a', 'an', 'of'].includes(word.toLowerCase()));
  const selected = meaningful.length ? meaningful : words;
  return (selected.length > 1 ? selected.slice(0, 2).map(word => [...word][0]).join('') : [...(selected[0] ?? '?')].slice(0, 2).join('')).toLocaleUpperCase();
}
