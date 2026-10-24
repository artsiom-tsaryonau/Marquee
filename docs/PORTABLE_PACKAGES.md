# Portable Marquee installations — proposal

Updated 2026-10-25. Design/roadmap only; no package exporter, importer or new
executable format is implemented.

A prepared game can be reusable across Marquee instances without converting it
into a native executable. For PS3, retain the verified disc ISO that RPCS3 already
boots directly. Repacking or extracting that ISO again is unnecessary. RPCS3
still runs the PS3 executable; its CPU/GPU-specific caches are not a portable
replacement for the game and emulator.

The portable unit should contain a versioned manifest, immutable payload file
manifest and a reviewed runtime-recipe reference. A self-contained export adds
the payload; a manifest-only export must explicitly require an existing matching
artifact at the destination. Neither form may claim to contain missing data.

The manifest should record:

- Platform, title ID, edition/version where established, base/update/DLC role and
  relationships. A title ID alone is not a complete payload identity.
- Relative payload paths, sizes and SHA-256 values; original artifact provenance,
  preparation recipe/version and output receipt identity. File hashes establish
  byte identity, not publisher authenticity or game compatibility.
- Runtime ID/version/OS/CPU and reviewed launch recipe ID. The importer resolves
  that recipe locally; manifests cannot supply shell scripts or arbitrary argv.
- Portable title-specific settings with an explicit allowlist. Display, audio
  devices, GPU choice, driver paths, absolute paths and credentials stay local.
- Optional metadata and locally owned artwork/media with their source labels.
  Saves are separate, opt-in transfers with conflict review, not default payload.

Runtime installation, PS3 firmware and local account/license/key material remain
separate requirements. Do not silently bundle a user's entire RPCS3 profile or
copy its `dev_hdd0` across libraries: that mixes installed data, saves, users and
other games. Runtime program files may be shared read-only; each instance retains
its own mutable configuration, firmware/user storage and caches. A transferred
PS3 game can still need title-specific settings or updates on the receiving host.

This extends the artifact/installation registry described in
[MARQUEE_MANAGER.md](MARQUEE_MANAGER.md), rather than adding another installation
authority. An output manifest remains useful after an optional source archive is
removed. Torrent seeding still depends on the exact original torrent payload,
not the prepared installation; package exports cannot replace that dependency.

## Implementation sequence

1. **MP0 — Manifest/receipt contract.** Specify a versioned JSON schema and create
   a small synthetic fixture. Keep local instance IDs out of portable identity;
   define input/output artifact references and runtime requirements.
2. **MP1 — Reviewed local export/import.** Begin with a small already supported
   emulator game. Preview contents, space, dependencies and save conflicts. Use
   manager jobs, bounded extraction, no-replace publication, immutable receipts,
   idempotent registration and failure/restart recovery. Verify paths, hashes,
   duplicate members and resource limits before any library changes.
3. **MP2 — Cross-instance acceptance.** Export from isolated instance A, import
   into B with separate user data, launch/exit/relaunch and verify that A and B's
   saves/settings remain independent. Test incomplete/modified packages and
   unavailable runtimes. Only then add the larger PS3 direct-ISO path.
4. **MP3 — Optional private transfer.** Stream immutable payloads between paired
   instances with authentication, resumption and content verification. Reuse the
   registry and manager; network transfer does not imply installation or play.

A directory-based manifest plus payload is a suitable first internal form. An
archive extension/container can follow the local acceptance test; inventing a
custom compression or executable format is unnecessary. Self-contained exports
may temporarily require another full payload copy, so space checks and an
explicit destination are essential for a 38 GB disc.
