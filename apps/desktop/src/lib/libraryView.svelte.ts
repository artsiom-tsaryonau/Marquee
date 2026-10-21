import type { Platform, Source } from './types';
import type { PlayerMode } from './metadata';

export type SortKey = 'title' | 'recent' | 'playtime' | 'added' | 'critic' | 'short' | 'source';

/**
 * What the Library screen shows: search, filters, sort, how far the list was
 * scrolled and how many cards were loaded. It lives outside the component so
 * that opening a game and pressing Back returns to the same place instead of
 * to a reset list.
 */
export interface LibraryView {
  query: string;
  discovery: boolean;
  addMenuOpen: boolean;
  platform: Platform | '';
  sourceFilter: Source | '';
  region: string;
  language: string;
  revision: string;
  playerMode: PlayerMode;
  installedOnly: boolean;
  noFiles: boolean;
  favOnly: boolean;
  sort: SortKey;
  filtersOpen: boolean;
  visibleCount: number;
  scrollY: number;
  signature: string;
}

export const PAGE = 72;

const views = new Map<string, LibraryView>();

function fresh(): LibraryView {
  const view = $state<LibraryView>({
    query: '', discovery: false, addMenuOpen: false, platform: '', sourceFilter: '', region: '', language: '', revision: '', playerMode: '',
    installedOnly: false, noFiles: false, favOnly: false, sort: 'title', filtersOpen: false,
    visibleCount: PAGE, scrollY: 0, signature: '',
  });
  return view;
}

export function libraryView(key: string): LibraryView {
  let view = views.get(key);
  if (!view) {
    view = fresh();
    views.set(key, view);
  }
  return view;
}
