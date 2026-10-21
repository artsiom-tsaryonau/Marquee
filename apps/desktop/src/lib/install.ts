export type Pcsx2Channel = 'auto' | 'unpacked';

export type InstallRun =
  | { type: 'pcsx2'; channel: Pcsx2Channel }
  | { type: 'setup' };

export interface InstallOption {
  id: string;
  title: string;
  detail: string;
  hint: string;
  run: InstallRun;
}

export interface Installable {
  id: string;
  title: string;
  subtitle: string;
  options: InstallOption[];
}

/** Tools the Install menu can open. Each option in a dialog is runnable on click. */
export const installables: Installable[] = [
  {
    id: 'pcsx2',
    title: 'PCSX2',
    subtitle: 'PlayStation 2 emulator',
    options: [
      {
        id: 'auto',
        title: 'Recommended',
        detail: 'Download the official release, verify it, and unpack it into Marquee’s data dir. No Flatpak, no host packages, no setup wizard.',
        hint: 'Managed by Marquee',
        run: { type: 'pcsx2', channel: 'auto' },
      },
      {
        id: 'setup',
        title: 'Open PCSX2 setup',
        detail: 'Launch PCSX2 so you can add BIOS files you already own.',
        hint: 'Never downloaded by Marquee',
        run: { type: 'setup' },
      },
    ],
  },
];

export function installable(id: string): Installable | undefined {
  return installables.find((tool) => tool.id === id);
}

export function installOption(toolId: string, optionId: string): InstallOption | undefined {
  return installable(toolId)?.options.find((option) => option.id === optionId);
}
