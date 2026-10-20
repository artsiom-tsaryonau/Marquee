import type { Platform, Source } from './types';

export const sourceLabel: Record<Source, string> = {
  steam: 'Steam',
  gog: 'GOG',
  epic: 'Epic',
  lutris: 'Lutris',
  port: 'Port',
  rom: 'Emulated',
};

export const platformLabel: Record<Platform, string> = {
  unknown: 'Platform unknown',
  'pc-linux': 'Linux',
  'pc-windows': 'Windows',
  nes: 'NES',
  snes: 'SNES',
  n64: 'Nintendo 64',
  gamecube: 'GameCube',
  ps1: 'PlayStation',
  ps2: 'PlayStation 2',
  gba: 'Game Boy Advance',
};

/** Emulator catalogue system ids, as people say them. */
const systemNames: Record<string, string> = { ps1: 'PlayStation', ps2: 'PlayStation 2', ps3: 'PlayStation 3', psp: 'PSP', dreamcast: 'Dreamcast', wiiu: 'Wii U', xbox: 'Xbox', '3ds': 'Nintendo 3DS' };
export const systemLabel = (id: string): string => systemNames[id] ?? id.toUpperCase();
