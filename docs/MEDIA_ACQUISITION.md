# Media, firmware, and game data

Updated 2026-10-21. Aligns product copy with the same automation goal as store installs: **Marquee runs acquisition jobs**; manual folder pick remains a supported fallback, not the primary workflow.

## Target experience

1. User connects a store, library root, or other **authorized source** Marquee understands.
2. Marquee detects missing firmware, disc images, port inputs, or keys.
3. A **download or import job** fetches or copies files, verifies them, places them in the right runner/port layout, and updates readiness.
4. Play stays one action from Library; metadata and covers continue to load in the background.

Marquee does **not** ship copyrighted BIOS or ROM files inside the application. Automation means orchestrating sources the user is allowed to use (owned store builds, existing installs, configured connectors), not bundling warez.

## Implemented today

| Kind | Automation | Fallback |
| --- | --- | --- |
| Emulator binaries | Managed install/update from official builds | Package-manager / Flatpak detection |
| Port engines | Install/update from official GitHub releases | — |
| PS1/PS2 discs | Scan configured library folders | Add folder / CLI `library add-root` |
| PS1/PS2 BIOS | — | Tools **Import a folder**; CLI `bios import` |
| Cover art | Serial lookup and optional API keys | Procedural placeholder |
| Steam/Epic/GOG titles | Planned direct adapters | Lutris cache read-only projection |
| Vault-style ROM research | `scripts/vimm/` POC (headed browser, one file at a time) | Not wired into the app yet |

## Planned work (not gated on finishing PS1/PS2 live gates)

- **Downloads UI and job manager** shared with store installs: progress, cancel, retry, resume state.
- **Firmware jobs** for PS1/PS2 (and other systems): fetch when a configured source provides them, same layout rules as today's import.
- **Disc/ROM jobs** per system: catalog match, region/format selection, verify hashes where available.
- **Port data jobs**: detect retail files from linked Steam/GOG installs; run documented extractors (Jak/OpenGOAL, recomp ports) without manual argv editing.
- **Connector plug-in contract** so research flows (for example vault browsers) can graduate into supported sources without one-off scripts.

Manual import, **Open folder**, and external emulators stay available for edge cases and air-gapped libraries.

## Related docs

- [Roadmap](ROADMAP.md) — work order and store milestones
- [Direct store integration](STORE_INTEGRATION.md) — owned-game download jobs
- [Managed emulators and BIOS](EMULATORS.md) — current BIOS panel and runner install
- [Expansion catalogue](EXPANSION_CATALOGUE.md) — per-platform adapter candidates
