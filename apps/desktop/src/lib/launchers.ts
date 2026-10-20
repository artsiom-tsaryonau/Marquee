import type { LauncherConfig, LauncherInfo } from './api/contracts';

export const launcherTemplates: Omit<LauncherConfig, 'id' | 'pinned'>[] = [
  { name: 'Steam', executable: 'steam', args: [], working_directory: null },
  { name: 'Lutris', executable: 'lutris', args: [], working_directory: null },
  { name: 'Heroic', executable: 'heroic', args: [], working_directory: null },
  { name: 'OpenGOAL', executable: '', args: [], working_directory: null },
];
export function launcherDraft(info?: LauncherConfig): LauncherConfig {
  return info ? { ...info, args: [...info.args] } : { id: `custom.${crypto.randomUUID()}`, name: '', executable: '', args: [], working_directory: null, pinned: false };
}
export function launcherMatches(info: LauncherInfo, query: string, onlyReady: boolean): boolean {
  return (!onlyReady || info.available) && `${info.name} ${info.executable}`.toLowerCase().includes(query.trim().toLowerCase());
}
