export type Pcsx2Channel = 'auto' | 'unpacked' | 'flatpak' | 'appimage';

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
        detail: 'Download the official AppImage and unpack it into Marquee’s data dir. No Flatpak, no host packages, no installer wizard.',
        hint: 'Managed by Marquee',
        run: { type: 'pcsx2', channel: 'auto' },
      },
      {
        id: 'unpacked',
        title: 'Unpack into Marquee',
        detail: 'Same as Recommended: extract linux-appimage-x64-Qt.AppImage to ~/.local/share/marquee/runners/pcsx2/current.',
        hint: 'No FUSE',
        run: { type: 'pcsx2', channel: 'unpacked' },
      },
      {
        id: 'appimage',
        title: 'Keep as AppImage',
        detail: 'Save the official AppImage and run it as a file (may need FUSE).',
        hint: '~57 MB',
        run: { type: 'pcsx2', channel: 'appimage' },
      },
      {
        id: 'flatpak',
        title: 'Flathub',
        detail: 'Installs net.pcsx2.PCSX2 as a user Flatpak (--user --noninteractive). Optional; Marquee does not need it.',
        hint: 'flatpak',
        run: { type: 'pcsx2', channel: 'flatpak' },
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
