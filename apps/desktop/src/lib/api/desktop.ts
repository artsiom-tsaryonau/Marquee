import { getCurrentWebview } from '@tauri-apps/api/webview';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import type { CoreEvent, LibraryApi } from './contracts';
import { asError, commands, type Call } from './core';

const call: Call = async <T>(cmd: string, args?: Record<string, unknown>) => {
  try {
    return await invoke<T>('core_call', { cmd, args: args ?? null });
  } catch (err) {
    throw asError(err);
  }
};

let listenReady: Promise<void> = Promise.resolve();

export const desktopApi: LibraryApi = {
  kind: 'desktop',
  ...commands(call),
  mediaUrl:(id)=>invoke<string>('media_url',{id}),
  async pickMedia(kind) {
    const selected=await open({directory:false,multiple:true,title:kind==='image'?'Add images':kind==='video'?'Add videos':'Add game media',filters:[{name:'Game media',extensions:kind==='image'?['png','jpg','jpeg']:kind==='video'?['mp4','webm']:['png','jpg','jpeg','mp4','webm']}]});
    return typeof selected==='string'?[selected]:selected??[];
  },
  async pickMediaDisc() {
    const selected=await open({directory:false,multiple:false,title:'Browse disc media',filters:[{name:'ISO disc image',extensions:['iso']}]});
    return typeof selected==='string'?selected:null;
  },
  musicUrl: (trackId) => invoke<string>('music_url', { trackId }),
  async pickMusicCover() {
    const selected = await open({ directory: false, multiple: false, title: 'Choose album artwork', filters: [{ name: 'Album artwork', extensions: ['jpg', 'jpeg', 'png'] }] });
    return typeof selected === 'string' ? selected : null;
  },
  async pickMusic() {
    const selected = await open({ directory: false, multiple: true, title: 'Add soundtrack audio', filters: [{ name: 'Audio files', extensions: ['mp3', 'flac', 'ogg', 'wav'] }] });
    return typeof selected === 'string' ? [selected] : selected ?? [];
  },
  async pickTorrent() {
    const selected = await open({ directory:false, multiple:false, title:'Review torrent download', filters:[{name:'BitTorrent metadata',extensions:['torrent']}] });
    return typeof selected === 'string' ? selected : null;
  },
  async pickPs3Firmware() {
    const selected = await open({ directory: false, multiple: false, title: 'Choose PS3 firmware package', filters: [{name: 'PS3 firmware package', extensions: ['pup']}] });
    return typeof selected === 'string' ? selected : null;
  },
  async pickConsoleContent() {
    const selected = await open({ directory:false, multiple:false, title:'Inspect PS4 or PS5 content', filters:[{name:'Console content',extensions:['pkg','rar']}] });
    return typeof selected === 'string' ? selected : null;
  },
  async pickPs3Content() {
    const selected = await open({ directory: false, multiple: false, title: 'Inspect PS3 content', filters: [{ name: 'PS3 content candidates', extensions: ['zip', '7z', 'rar', 'iso', 'pkg', 'pup', 'sfo'] }] });
    return typeof selected === 'string' ? selected : null;
  },
  async pickArchives() {
    const selected = await open({ directory:false, multiple:true, title:'Import ZIP, 7z or RAR game archives', filters:[{name:'Game archives',extensions:['zip','7z','rar']}] });
    return typeof selected === 'string' ? [selected] : selected ?? [];
  },
  onArchiveDrop: (handler) => getCurrentWebview().onDragDropEvent((event) => { if (event.payload.type === 'drop') handler(event.payload.paths); }),
  async pickRoot() {
    const selected = await open({ directory: true, multiple: false, title: 'Choose a folder of game files' });
    return typeof selected === 'string' ? selected : null;
  },
  async pickExecutable() {
    const selected = await open({ directory: false, multiple: false, title: 'Choose a launcher executable or AppImage' });
    return typeof selected === 'string' ? selected : null;
  },
  webUrl: () => invoke<string | null>('web_url'),
  subscribe(handler) {
    let stopped = false;
    let stop = () => {};
    listenReady = listen<CoreEvent>('core-event', (event) => { if (!stopped) handler(event.payload); }).then((unlisten) => {
      if (stopped) unlisten();
      else stop = unlisten;
    });
    return () => { stopped = true; stop(); };
  },
  whenReady() {
    return listenReady;
  },
};

export function isDesktopRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}
