# Correction pass — 2026-10-20

Applied to the current Marquee tree without resetting existing work.

| Audited issue | Correction | Regression evidence |
| --- | --- | --- |
| ZIP follows preexisting directory symlinks | Reject symlink traversal during ZIP/TAR extraction; reject release/user symlink conflicts before promotion | External save remains untouched |
| Restart loses a live emulator session and permits another launch | Persist PID + boot/start identity; checkpoint monotonic duration; recover and monitor matching processes; retain instance lease while monitored | Live process survives reopen; second launch stays busy; time credited once; reused PID rejected |
| Failed update partially replaces installed executable | Download/extract/validate in a private staging directory, preserve existing data there, rename with backup/rollback | Corrupt ZIP cannot change old executable; symlink conflict cannot promote; saves preserved |
| Nested TAR update reports new version but keeps old binary | Locate/unpack only in the fresh release directory before promotion | Nested TAR update promotes v2 over v1 |
| Global runner choice ignored after first launch | Stop creating implicit per-edition profiles; v3 migration clears legacy auto-created profiles | Launch A, select B, launch uses B; migration retains personal title/favorite |
| Other windows/tabs miss library mutations | Emit LibraryChanged after root/title/favorite/runner mutations; UI refreshes discs and ports | Core event assertions and frontend invalidation test |
| Truncated ISO aborts a whole scan | Per-file import warnings, preserving fatal DB errors | Good ISO imports beside bad ISO; warning exposed in scan summary |

Additional fixes found during validation: explicitly release the instance flock so a concurrent fork/exec cannot briefly prolong an already-dropped instance's lock; finalize sessions and credit playtime in one transaction; retain time even when the game exits unsuccessfully; hide rather than close the native window during a tracked game, then show it when the session finishes.

Schema v3 adds `sessions.process_identity`. Legacy launch profiles were automatically written on each Play, so migration clears them; media, titles, favorites, sessions, and playtime remain intact. This migration is separate from the proposed unified-library schema.

Recovery is Linux-specific (`/proc` identity and boot ID). An already-dead process cannot supply its exit status or exact time during Marquee downtime, so recovery credits the last saved checkpoint and reports an interrupted session. A recovered live process is timed until it disappears; its eventual exit status is unavailable. A small spawn-before-identity-write crash window remains; a future helper/process handshake is needed to eliminate it. Native ports retain their existing separate lifecycle and do not yet receive this session tracking.

Staged installation rollback handles operation failures, not an atomic transaction spanning filesystem renames and SQLite metadata across sudden power loss. Keep update-journal/crash recovery as follow-up work. Extraction assumes the private staging directory is not concurrently modified by another process under the same user.

Checks passed: 54 Rust tests (workspace excluding the desktop test binary), 22 frontend tests, desktop Rust check, Svelte check (zero errors/warnings), production frontend build, and whitespace validation. See the accompanying unified-library notes for UI and media verification limits.
