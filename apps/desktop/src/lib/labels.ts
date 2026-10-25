import type { Platform, Source } from './types';

export const sourceLabel: Record<Source, string> = {
  steam: 'Steam',
  gog: 'GOG',
  epic: 'Epic',
  lutris: 'Lutris',
  port: 'Native port',
  rom: 'Emulated',
  catalogue: 'Catalogue',
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
  genesis: 'Genesis / Mega Drive',
  atari2600: 'Atari 2600', atari5200: 'Atari 5200', sms: 'Master System', atari7800: 'Atari 7800',
  tg16: 'TurboGrafx-16', tgcd: 'TurboGrafx-CD', cdi: 'CD-i', segacd: 'Sega CD', jaguar: 'Jaguar',
  '32x': 'Sega 32X', saturn: 'Saturn', jaguarcd: 'Jaguar CD', dreamcast: 'Dreamcast', xbox: 'Xbox',
  xbox360: 'Xbox 360', 'xbox360-digital': 'Xbox 360 Digital', ps3: 'PlayStation 3', wii: 'Wii', wiiware: 'WiiWare',
  wiiu: 'Wii U', gb: 'Game Boy', lynx: 'Lynx', gamegear: 'Game Gear', virtualboy: 'Virtual Boy', gbc: 'Game Boy Color',
  neogeo: 'Neo Geo', ds: 'Nintendo DS', psp: 'PSP', '3ds': 'Nintendo 3DS',
};

/** Emulator catalogue system ids, as people say them. */
const systemNames: Record<string, string> = { genesis: 'Genesis / Mega Drive', ds: 'Nintendo DS', neogeo: 'Neo Geo', ps1: 'PlayStation', ps2: 'PlayStation 2', ps3: 'PlayStation 3', ps4: 'PlayStation 4', ps5: 'PlayStation 5', psp: 'PSP', dreamcast: 'Dreamcast', wiiu: 'Wii U', xbox: 'Xbox', xbox360: 'Xbox 360', '3ds': 'Nintendo 3DS', gb: 'Game Boy', gbc: 'Game Boy Color', gba: 'Game Boy Advance' };
export const systemLabel = (id: string): string => systemNames[id] ?? id.toUpperCase();
