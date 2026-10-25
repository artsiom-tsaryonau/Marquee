# Storage settings

Open **Settings → Storage** in the native app or the authenticated web surface.
Both call the same Rust storage API. This screen is an inventory and supported
folder settings surface; it does not yet move managed installations or profiles.

## Locations and usage

The inventory groups registered games, retained sources, manager installations,
emulator programs, BIOS/firmware, save profiles, media and configured streaming
storage. It includes missing dependency locations so you can distinguish a path
that is expected from a populated installation. A present folder is not proof
of valid BIOS contents or a launchable game.

Each row shows its registered path, resolved physical path when different,
available space on its volume and an **Open folder** action. A database or BIOS
file opens its containing folder. Only locations listed by the backend can be
opened through this action; arbitrary path arguments are not accepted.

**Measure usage** starts a background metadata walk; opening Settings does not
hash or recursively read the collection. You can leave the page while it runs.
Measurements remain in memory for the running app and can be refreshed.

Sizes are logical regular-file bytes, not allocated disk blocks. A sparse HDD
image can therefore appear larger than the space it consumes. Several rows
overlap (for example RPCS3 firmware and its profile, or manager storage and
installations); there is deliberately no sum of rows. Registered compatibility
roots are resolved for measurement, but nested symbolic links and special files
such as sockets are skipped. Unreadable entries and the 250,000-entry per-location
limit produce a partial result. This is a usage estimate, not a consistent backup
or content-integrity verification.

## Supported folder settings

### Collection folder

Choose an existing folder outside the application's data/config/cache trees.
Marquee saves its canonical path in the library setting `storage.collection_root`.
For this owner's existing collection it is `~/Games/Marquee`.

- Retained originals are shown at `<collection>/sources`.
- Immediate archive intake uses an **existing online platform root first**.
  When no such root exists, new platform folders go under
  `<collection>/library/imported-roms/<system>`.
- For new-platform destinations, the collection's device and directory identity
  are checked. A missing disk or replacement folder reports an error instead of
  silently creating a replacement collection. Choose the folder again after an
  intentional replacement or restoration to confirm its new identity.
- Reset restores the previous intake selection rules for future imports. It
  does not remove folders or registrations already created.

This setting does not move, discover or copy existing files. It does not redirect
the manager's durable preparation jobs, browser/torrent payloads, music imports,
firmware profiles or program builds. Manager jobs retain their reviewed
destinations and immutable receipts. Until a coordinated manager relocation is
implemented, those locations are intentionally read-only in this screen.

### Additional BIOS import folder

Choose an existing host folder to save the `bios_inbox` setting. At launch,
Marquee checks it in addition to `<data>/bios-inbox` and copies recognized
inputs into emulator destinations. Reset removes only the additional input
setting; it does not remove installed firmware, the built-in inbox or saves.
See [exact BIOS locations](RUNTIME_FILES.md#bios-and-firmware-are-dependencies-not-games).

On the web surface, the folder prompt expects a path on the **Marquee host**.
It does not upload files from the browser device. The native app uses its
folder picker. Both surfaces validate paths through the backend.

## Existing-file moves

Use the [verified offline relocation workflow](STORAGE_LAYOUT.md#offline-relocation-tool)
for reviewed source and ROM cohorts. Keep compatibility links while library
records, receipts or manifests still use them. This screen shows those physical
destinations without rewriting identities.

The remaining migration work is service-owned configuration for program,
installation, profile and streaming roots; an exclusive move/rollback operation
coordinated with their owners; and reviewed cleanup based on dependencies.
The usage screen never deletes archives, games, firmware, saves or caches.

## Verification

Rust tests cover compatibility-root measurement without following nested links,
folder validation, preserved library roots, persistent settings, new-platform
intake placement, unavailable collections and replacement directory handling. The shared API also limits
which folder settings can be changed.

Live 2026-10-25: the owner's library exposed 43 locations. Usage matched the
previous inventories for retained sources (174,298,281,751 bytes) and imported
ROMs (11,591,239,678 bytes). The Storage screen, BIOS filter and 390-pixel layout
were exercised in the authenticated web surface with no JavaScript exceptions
or horizontal overflow. This does not constitute a new emulator/gameplay test.
