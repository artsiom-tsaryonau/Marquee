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
