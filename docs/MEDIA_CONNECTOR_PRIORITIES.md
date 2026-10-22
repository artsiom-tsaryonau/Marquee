# Media connector priorities (ROM and preservation sources)

Updated 2026-10-21. Planning note for **which external ROM/disc sources Marquee should automate**, informed by community indexes such as [FMHY Gaming → ROM sites](https://fmhy.net/gaming#rom-sites). FMHY is a **link directory**, not a host; listing a site here does **not** mean Marquee endorses redistribution or that use is legal in your jurisdiction. Marquee never bundles copyrighted BIOS or ROM files.

Updated boundary (2026-10-22): provider work runs behind the hidden [marquee-manager](MARQUEE_MANAGER.md); this document ranks sources, not job/process ownership. The [roadmap](ROADMAP.md) puts manager preparation first and preserves Steam → Epic → GOG store order. Provider policy here does not forbid user-reviewed generic torrent transport; it does not authorize automation of tracker indexes or gated sites.

## Principles for any connector

| Rule | Rationale |
| --- | --- |
| **User-initiated jobs** | One title (or one firmware file) per job; clear consent; cancel/retry like store downloads. |
| **No silent bulk** | No background scraping of whole vaults, torrent swarms, or Telegram channels. |
| **Verify when possible** | Match [No-Intro](https://no-intro.org/) / [Redump](http://redump.org/) DATs or catalogue SHA-1 before marking installed. |
| **Headed browser when required** | Sites with bot checks, interstitial ads, or per-download gates use a **user-visible** browser (local connector tooling; not shipped in this repo). |
| **Metadata ≠ entitlement** | Catalogue import (Vimm JSON, hashes, covers) does not imply a licensed copy or launchable install. |
| **Secrets stay backend-side** | Site sessions/cookies stay in the browser; manager/store credentials stay in protected backend storage. UI/events do not carry secrets. |

## Priority tiers

### Tier 0 — Already in scope (no FMHY dependency)

| Source | Role | Status |
| --- | --- | --- |
| **Local library roots** | PS1/PS2 (and future systems) scan, CUE/CHD rules | Implemented |
| **`bios import` / Tools folder** | Firmware the user already has | Implemented (fallback) |
| **Steam / Epic / GOG adapters** | Owned PC/console-adjacent builds, port inputs from installs | Roadmap S1/E1/G1 |
| **Managed emulator/port binaries** | Official GitHub / release assets only | Implemented |

### Tier 1 — First ROM connectors (build next)

| Source | FMHY / community role | Marquee approach | Notes |
| --- | --- | --- | --- |
| **Vimm's Lair** | Starred general vault on FMHY | **Downloads job**: catalog browse (metadata via JSON import), **one download at a time**, paired companion/headed browser (CDP remains research only), user completes ads/dialogs | POC in [fmhy-marquee-scrapers](https://github.com/artsiom-tsaryonau/fmhy-marquee-scrapers) (`connectors/vimm/`); respect site rate limits and ToS |
| **Catalogue metadata only** | Same vault, saved HTML/JSON | Keep **import-only** path (`catalogue import`); link/match by hash | Implemented; stays separate from download jobs |

**Tier 1 acceptance:** Reviewed provider selection → manager acquisition → source artifact → supported preparation/validation → installation receipt → library registration. Hash mismatch blocks preparation; missing reference hashes remain explicit. Provider completion alone cannot mark a game installed.

### Tier 2 — Preservation HTTP mirrors (after Tier 1 job manager)

| Source | FMHY / community role | Marquee approach | Notes |
| --- | --- | --- | --- |
| **Myrient-style mirrors** | FMHY points at community **Myrient backup** lists (e.g. Minerva / savemyrient); [Myrient](https://myrient.erista.me/) is a direct HTTP preservation mirror | **Optional connector**: user pastes or selects a **known mirror base URL** + relative path from DAT/catalogue; `curl`-style download with resume; **mandatory** hash check against Redump/No-Intro or catalogue SHA-1 | No crawling; user or catalogue supplies exact path; mirror choice is user configuration |
| **No-Intro / Redump DATs** | Listed under FMHY **ROM resources** | Ship or fetch **DAT subsets** per system; verification step in job pipeline | Metadata only in app; DAT files are hashes, not ROMs |
| **Archive.org** (e.g. WAD collections) | Occasional FMHY entries | Case-by-case: **identifier + file** from user selection; same hash/job rules | Not a generic “search Archive” spider |

**Tier 2 acceptance:** Download works without a headed browser when the URL is stable; failure messages distinguish HTTP 404, checksum mismatch, and disk full.

### Tier 3 — Metadata, patches, and community indexes (no retail download automation)

| Source | FMHY section | Marquee approach |
| --- | --- | --- |
| **GameTechWiki ROM & ISO list**, **ROM CSE** | Meta-index | Documentation links only; optional “open in browser” from catalogue |
| **ROMhacking.net**, **SMWCentral**, **Homebrew Hub** | Hacks / homebrew | Link-out; future **patch job** applies user-supplied patch to **user-supplied base ROM** |
| **NSWDB / 3DSDB** | Release trackers | Optional **metadata** sync (title ID, version), not PKG/XCI download |
| **psx-covers / ps2-covers** | Art | Align with [enrichment](ENRICHMENT.md) providers |

### Tier 4 — Do not automate (explicit non-goals)

FMHY lists many of these; they are poor fits for Marquee’s job model, legal posture, or support cost.

| Pattern | Examples (as listed on FMHY) | Why not |
| --- | --- | --- |
| **Ad-heavy mirror farms** | WowROMs, RomsTime, ROMsFun, Romspedia, long tail of repost sites | Fake download buttons, malware risk; unstable HTML; no stable API |
| **Sign-up / forum gates** | roms.by, TheRomDepot, RetroGameTalk | Credential handling, CAPTCHA, account bans |
| **Torrent + VPN workflows** | RuTracker ROMs, BlueRoms, NSWTL, magnet link indexes | Tracker politics, VPN requirement, swarm etiquette—not a launcher core competency |
| **Telegram / Discord gated** | GNS, many Switch/PS5 lists | No stable automation contract; ToS and safety issues |
| **Password-in-index hosts** | AlvRo, Pippo Library (FMHY notes passwords) | Security anti-pattern; fragile |
| **Broken or script-dependent hosts** | Emuparadise (+ user-script “fix”) | Depends on third-party scripts; downloads intentionally disabled upstream |
| **Console PKG stores mixed with PC warez** | DLPSGame, Super PSX (“avoid PC games”) | Wrong product surface; high legal and UX risk |
| **Unofficial No-Intro bulk archives** | FMHY rentry “No-Intro Archive” | Full-set redistribution; user must supply own dumps or use Tier 2 path + hash |
| **BIOS / “emulator files” search hints** | FMHY “search for Emulator Files” | Marquee uses **user firmware import** or future **authorized** firmware jobs—not BIOS warez hosts |

User can always **import a folder** or use **Open in browser** for sources Marquee does not automate.

## FMHY section → tier map (quick reference)

| FMHY block | Default tier | Marquee stance |
| --- | --- | --- |
| [ROM resources](https://fmhy.net/gaming#rom-resources) (DATs, managers, patchers) | 2–3 | Verification and library hygiene; integrate DAT check in jobs |
| [ROM sites](https://fmhy.net/gaming#rom-sites) (general) | 1–4 | **Vimm** → 1; **Myrient backups** → 2; most starred/long-tail → 4 |
| [Nintendo ROMs](https://fmhy.net/gaming#nintendo-roms) | 3–4 | Trackers/metadata optional; Switch Telegram hosts → 4 |
| [PlayStation ROMs](https://fmhy.net/gaming#playstation-roms) | 3–4 | NoPayStation-style **owned** PKG workflows are store-like—revisit only with explicit auth model; PS2 ISO hosts → 4 |
| [Retro browser emulators](https://fmhy.net/gaming#retro-browser-emulators) | — | Out of scope (play-in-browser, not library files) |
| Arcade / MAME sets (FMHY arcade rows) | 3–4 | Future **MAME** adapter needs **user-provided** ROM set + version profile; no auto-download of merged sets |

## Implementation sequence (connector track)

1. **Manager foundation** — follow M0–M2 in [MARQUEE_MANAGER.md](MARQUEE_MANAGER.md); reuse its contracts rather than create another queue.
2. **Vimm browser-worker bridge** — manager M4; keep provider code in the sibling connector repo and reuse catalogue IDs.
3. **Preparation and receipt publication** — reference-hash/DAT checks when available, explicit unknowns otherwise; mismatch blocks installation. Only the library can register the resulting installation.
4. **Configurable mirror base URL** — Tier 2 Myrient-style fetches with catalogue-relative paths only.
5. **Firmware jobs** — authorized paths only (e.g. dump-from-own-console workflows); never scrape BIOS hosts.

## Related docs

- [Media acquisition](MARQUEE_MANAGER.md#acquisition-user-flow) — product-level automation vs fallback
- [Catalogue metadata](CATALOGUE_METADATA.md) — Vimm JSON import and linking
- [Expansion catalogue](EXPANSION_CATALOGUE.md) — per-platform launch adapters
- [Launcher branding](LAUNCHERS.md#launcher-and-store-marks) — unrelated to ROM sources; Tools UI only

Sources checked 2026-10-21: FMHY gaming page (ROM resources, ROM sites, Nintendo ROMs, PlayStation ROMs); Marquee catalogue schema v5 import format in `marquee-core`.
