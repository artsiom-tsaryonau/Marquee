# Library UX notes

1. **Source / launcher filter** — dropdown in the library filter bar (done).
2. **Dropdown styling** — shared `.ui-select` in `app.css` (done).
3. **Run method filter** — removed from library; edition/run picker stays on game page (done).
4. **Player mode filter** — single / multiplayer / co-op instead of raw counts (co-op shares multiplayer until we have richer metadata) (done).
5. **Library click → preview dialog** — still open; full game page kept for now.
6. **Platform tags** — shown on grouped cards when one title spans multiple platforms (done).
7. **Source page** — facts table + metadata section link; duplicate hero button removed (done).
8. **Cover attribution** — (i) icon on catalogue cover (done).
9. **No installation** — catalogue entries show “Not in library” on covers and a discovery note instead of Play (done).
10. **Library audit (2026-10-21)** — one "Add games" menu instead of five folder buttons; platform chips always labelled, biggest first, store games grouped as "Other"; primary row is search, Installed, Favourites, Filters (with active-count badge) and Sort; the five secondary filters open on demand; only sorts that apply to this library are listed; search, filters, sort, loaded cards and scroll position survive opening a game and pressing Back; cards load while scrolling (button kept as fallback); readable card titles (`(PD)`, `[C]`, `(USA)`, catalogue numbers removed, region moved to the subtitle, two lines, full title on hover); generated art no longer repeats the title; "0 h played" fixed; Esc clears search; catalogue import moved to the bottom (done).
11. **Games without files (2026-10-21)** — add by title and system, wishlist or owned; Play disabled; cover and facts by name; a matching scanned file attaches automatically; "Without files" filter (done). See docs/WANTED_GAMES.md.
12. **Home redesign + muted logo (2026-10-21)** — compact hero with poster and soft backdrop, opens on what you played last, library overview strip with platform shortcuts, wishlist row, calmer title; brand mark and app icon in quiet slate (done).
