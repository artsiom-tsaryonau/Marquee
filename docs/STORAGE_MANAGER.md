# Storage settings

Open **Settings → Storage** in the native app or the authenticated web surface.
Both call the same Rust storage API. It inventories storage, configures supported
input folders and provides reviewed same-filesystem moves for managed game
installations. Emulator profiles and working firmware remain separate.

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
destinations and immutable receipts. Use the separate **Move managed
installations** controls below for an existing installation tree; the collection
folder setting itself never moves it.

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

### Managed installations

1. Close games and stop the owned streaming host. Finish/cancel pending
   preparations and browser jobs, and disable torrent transport.
2. Choose **Stop manager for maintenance**. It drains accepted work; wait for
   **Manager is stopped**. The move then takes the exclusive manager lease.
3. **Choose destination and review** selects an existing parent directory.
   Its unused `managed-installations` child is the proposed destination.
4. Review source, destination, size, free space and affected games. Receipt
   hashes and the whole installation tree are checked in the background.
5. Choose **Move installations… → Confirm move**. Marquee verifies the input
   again, renames it without overwriting a destination, retains its old parent
   path as a compatibility link, then verifies output hashes and receipts.
6. When complete, **Start manager** restores normal worker operation. New jobs
   use the new physical root; existing receipt payloads and library paths stay
   unchanged.

The first implementation supports the **same filesystem only**. It requires no
second payload-sized copy. Only registered, receipt-covered installations move;
staged, unregistered or modified files block the operation. Source downloads,
BIOS, working firmware, memory cards, saves and emulator programs stay put.
Local/streamed game launches, native-port launches and ordinary scans are blocked
while a move is active or needs recovery. Close other software using the tree,
too; Marquee cannot control arbitrary programs launched outside it.

The durable private records are `<data>/manager/installation-moves/<UUID>.json`,
`current.json` and `location.json`. If interrupted, stop the manager and use
**Resume move…** or **Roll back…** in Storage. Rollback verifies unchanged
contents and registered receipts before restoring the previous location. It
refuses changed/new installations rather than deleting them. Historical move
journals and old compatibility paths remain; do not remove them as cache files.

The first applied move advances the **manager jobs database** to schema 5 so
older managers, which cannot validate the relocated parent, refuse to write it.
Library schema, job IDs and receipt fingerprints do not change. Rollback restores
the location, not the older manager binary/schema. Keep compatible executables
and take consistent library/manager backups before maintenance.

### Original sources and imported ROMs

Use the [verified offline relocation workflow](STORAGE_LAYOUT.md#offline-relocation-tool)
for reviewed source and ROM cohorts. Keep compatibility links while library
records, receipts or manifests still use them. This screen shows those physical
destinations without rewriting identities.

The remaining migration work is cross-filesystem copy/promotion, program,
profile, firmware and streaming-root moves, and reviewed cleanup based on dependencies.
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

The subsequent managed move relocated Vanquish's 7,838,695,424-byte ISO to
`~/Games/Marquee/library/managed-installations`. The two-step review UI, manager
stop/restart and narrow layout passed. All 858 media paths remained available;
editions, media, roots, library receipts and manager job payloads matched their
backed-up baselines. Xenia and its stream session started through the retained
path and were then stopped. No new gameplay/save acceptance is claimed.
Tests additionally cover interrupted forward/reverse renames, destination
collisions, changed inputs, exclusive ownership, immutable receipt fingerprints,
subsequent moves, and new preparation after a real manager-process restart.
