import type { GameEntry, PortEntry } from './types';
import { systemLabel } from './labels';

/** Availability is installation-specific; installed files alone do not imply readiness. */
export function gameStatus(game: GameEntry): { label: string; tone: 'ok' | 'warn' | 'dim' | 'note' } | null {
  if (game.source === 'catalogue') return { label: 'Catalogue only', tone: 'dim' };
  if (game.wanted) return { label: game.wanted === 'wishlist' ? 'Wishlist' : 'Game files required', tone: 'note' };
  if (game.rootOffline) return { label: 'Drive unavailable', tone: 'warn' };
  if (game.mediaMissing) return { label: 'Files unavailable', tone: 'warn' };
  if (game.setupState === 'runner_missing') return { label: 'Emulator required', tone: 'warn' };
  if (game.setupState === 'missing') return { label: 'Firmware required', tone: 'warn' };
  if (game.portStatus === 'needs-rom' || game.portStatus === 'needs-data') return { label: 'Game files required', tone: 'warn' };
  if (!game.installed || game.portStatus === 'not-installed') return { label: 'Not installed', tone: 'dim' };
  if (game.portStatus === 'installed') return { label: 'Port installed · setup unchecked', tone: 'note' };
  if (game.portStatus === 'update') return { label: 'Update available', tone: 'note' };
  return null;
}

export function canPlay(game: GameEntry): boolean {
  return game.source !== 'catalogue' && !game.wanted && game.installed && !game.rootOffline && !game.mediaMissing &&
    game.setupState !== 'runner_missing' && game.setupState !== 'missing' &&
    (!game.portStatus || game.portStatus === 'ready');
}

export function emulatorName(game: GameEntry): string {
  return game.runnerName?.replace(' (Marquee)', '') ?? ({ ps1: 'DuckStation', ps2: 'PCSX2', genesis: 'BlastEm', ds: 'melonDS', neogeo: 'FinalBurn Neo' } as Record<string, string>)[game.platform] ?? `${systemLabel(game.platform)} emulator`;
}

/** Missing data opens the supported folder fallback, never reinstalls the engine. */
export function portAction(port: Pick<PortEntry, 'status'>): { label: string; operation: 'launch' | 'install' | 'folder' } {
  switch (port.status) {
    case 'installed': return { label: 'Open port', operation: 'launch' };
    case 'ready': return { label: 'Play', operation: 'launch' };
    case 'update': return { label: 'Update', operation: 'install' };
    case 'needs-rom':
    case 'needs-data': return { label: 'Open game files folder', operation: 'folder' };
    default: return { label: 'Install', operation: 'install' };
  }
}

export function portStatusLabel(port: Pick<PortEntry, 'status'>): string {
  return { installed:'Installed · setup unchecked', ready:'Ready to play', update:'Update available · setup unchecked', 'needs-rom':'Game files required', 'needs-data':'Game files required', 'not-installed':'Not installed' }[port.status];
}
