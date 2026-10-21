# Launcher and store marks

Updated 2026-10-21. Reference for **recognisable icons** on Tools cards, Library source chips, and future store adapters. Marks identify products the user already uses; they are not an endorsement. Prefer **monochrome SVG** (`currentColor`) in the UI so chips and cards stay theme-consistent.

## What Marquee already has

| Location | Coverage |
| --- | --- |
| [`apps/desktop/src/lib/brands.ts`](../apps/desktop/src/lib/brands.ts) | SVG paths from [Simple Icons](https://simpleicons.org/) (CC0 **data**; trademarks stay with owners): `steam`, `gog`, `epic`, `lutris`, `playstation`, `psp`, `sega`, `linux` |
| [`apps/desktop/src/lib/components/Mark.svelte`](../apps/desktop/src/lib/components/Mark.svelte) | Maps library `source` / platform ids to those marks (`steam`, `gog`, `epic`, `lutris`, …) |
| Managed emulators | AppImage / package **application icons** copied at install time (`ensure_icon` in core) — shown on `ToolCard` via `coverSrc(e.icon)` |
| Tools launcher presets | Steam, Lutris, Heroic still use generic `glyph="monitor"` on [`ToolCard`](../apps/desktop/src/lib/components/ToolCard.svelte) — **gap** |

## Entities Marquee names in code or roadmap

### Store clients and library managers

| Entity | Recognisable mark | Official / canonical assets | Simple Icons slug | UI notes |
| --- | --- | --- | --- | --- |
| **Steam** | Black locomotive on white (or inverted) | [Steamworks branding](https://partner.steamgames.com/doc/marketing/branding) (partner PDFs; strict for commercial use) | `steam` | Already in `brands.ts`. Library chips OK; wire Tools preset `detected.steam`. |
| **Epic Games Store** | Shield + “Epic Games” wordmark (not the parent Epic Games logo alone) | [Epic Games Store logo guidelines](https://dev.epicgames.com/docs/epic-games-store/requirements-guidelines/store-assets/logo-guidelines) | `epicgames` (path already stored as `epic`) | Distinguish **store** from Epic corporate mark. |
| **GOG** | “GOG.com” logotype in frame | [GOG press room](https://www.gog.com/pressroom/press-kit) | `gogdotcom` (stored as `gog`) | Same mark for **gogdl** CLI — it is a GOG protocol tool, not a separate brand. |
| **Amazon Games** | Orange “smile” / Games branding | Amazon developer / Games press kits (no slug in Simple Icons) | — | Heroic integrates **Nile** for Amazon; use Amazon Games mark only where that store is shown, not for Heroic itself. |
| **Lutris** | Orange joystick / “Lutris” wordmark | Upstream icon: `lutris/lutris` → `share/icons/hicolor/scalable/apps/lutris.svg` | `lutris` | Flatpak app id `net.lutris.Lutris` uses the same artwork. Legacy bridge only; still worth a correct mark on Tools. |
| **Heroic Games Launcher** | Purple “H” / shield app icon | [HeroicGamesLauncher](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher) `public/icon.png`, Flathub `com.heroicgameslauncher.hgl` | `heroicgameslauncher` | Not the Epic shield — Heroic is a third-party launcher. |
| **Legendary** | Orange “legendary” wordmark / app icon | [legendary-gl/legendary](https://github.com/legendary-gl/legendary) README social image, `assets/windows_icon.ico` | — | CLI-only; tongue-in-cheek MMORPG rarity name. Use project art, not Epic’s store logo. |
| **NCSOFT PURPLE** | Iridescent sphere with vertical slits + “PURPLE” wordmark | [purple.plaync.com](https://purple.plaync.com/download) product pages; no public vector kit found | — | Managed recipe in [PURPLE.md](PURPLE.md). NC corporate logo is separate (yellow/blue). |

### Compatibility and runtimes

| Entity | Recognisable mark | Official / canonical assets | Simple Icons slug | UI notes |
| --- | --- | --- | --- | --- |
| **Wine** | Wine glass + “Wine” | [winehq.org](https://www.winehq.org/) branding | `wine` | Execution component; optional badge on Windows titles. |
| **Valve Proton** | Purple/blue “Proton” wordmark (Steam compatibility tool, ~2024) | Shipped with Steam compatibility tools; **not** in the Proton GitHub tree as a standalone kit | — | **Do not** use Simple Icons `proton` — that is **Proton AG** (mail/VPN). |
| **GE-Proton** | Community “GE” / Proton-GE release art | [GloriousEggroll/proton-ge-custom](https://github.com/GloriousEggroll/proton-ge-custom) release notes / Heroic Wine Manager tiles | — | Not Valve’s mark; label as “GE-Proton” or “custom Proton” in UI copy. |
| **umu** | Project icon from umu-launcher packaging | [Open-Wine-Components/umu-launcher](https://github.com/Open-Wine-Components/umu-launcher) (check `data/` / AppImage icon) | — | Marquee pins a portable umu zipapp for Purple; show umu mark only on runtime rows, not as a “store”. |
| **Flatpak** | Green “F” / Flatpak triangle | [flatpak.org](https://flatpak.org/press/) | `flatpak` | Useful for “Lutris (Flatpak)” and Heroic Flatpak hints, not as a game source. |

### Games and ports (Tools-adjacent)

| Entity | Recognisable mark | Official / canonical assets | Simple Icons slug | UI notes |
| --- | --- | --- | --- | --- |
| **OpenGOAL** | Jak-inspired launcher artwork | [open-goal/launcher](https://github.com/open-goal/launcher) release assets | — | Port source in library, not a PC store. |
| **PCSX2 / DuckStation / …** | Upstream emulator icons | Catalogue + managed install copies | — | Already handled per emulator. |

## Simple Icons policy (recommended default)

1. **Add paths** to `brands.ts` only from [simpleicons.org](https://simpleicons.org/) (or the same path data), with the existing file header comment.
2. **Extend `brandFor`** in `Mark.svelte` and map launcher preset ids (`steam`, `lutris`, `heroic`) in `Tools.svelte` — either pass a `brand` prop on `ToolCard` or reuse `<Mark id="…" />` in the card header.
3. **Do not** add Simple Icons `proton` for Steam Proton. Ship a small custom SVG traced from Valve’s compatibility-tool artwork, or use a neutral glyph plus the text “Proton”.
4. **Legendary, PURPLE, umu, OpenGOAL**: vendor SVG/PNG from upstream repos (check license in each repo); store under `apps/desktop/static/brands/` with a short `README` listing source URL and commit hash.

## Trademark and press-kit hygiene

- **Identification, not sponsorship** — same pattern as current `brands.ts` comment.
- **Store guidelines** — Epic and Valve restrict logo use in marketing; in-app identification of the user’s connected client is the usual case, but avoid implying partnership.
- **NCsoft / PURPLE** — use only to open or label the official launcher the user installed; do not repurpose the sphere mark for unrelated “purple” UI chrome.
- **GE-Proton** — community build; do not label it “Proton” without “GE” or “custom” where ambiguity matters.

## Implemented (Tools UI)

- PNG bundle: [`apps/desktop/public/brands/`](../apps/desktop/public/brands/) with provenance in [`README.md`](../apps/desktop/public/brands/README.md).
- Mapping: [`apps/desktop/src/lib/brandIcons.ts`](../apps/desktop/src/lib/brandIcons.ts) → `ToolCard` on launcher presets and Purple/Lutris source cards.
- Library chips still use SVG [`brands.ts`](../apps/desktop/src/lib/brands.ts) + `Mark.svelte` (consistent filter row).

## Follow-up

1. Store connection wizards (S1/E1/G1): reuse `brandIcon()` for Epic/GOG/Steam job UI.
2. Runtime rows (umu, GE-Proton, Wine): secondary badges; avoid Simple Icons `proton` (Proton AG).
3. Optional: replace `steam.png` if Valve partner assets become available for in-app identification.

## Sources checked (2026-10-21)

- Simple Icons slugs: `steam`, `gogdotcom`, `epicgames`, `lutris`, `heroicgameslauncher`, `flatpak`, `wine`, `valve` (not used for Proton tool)
- Lutris upstream icon path; Heroic / Legendary GitHub; Epic store logo guidelines; GOG press kit; PURPLE product site; Flathub `com.heroicgameslauncher.hgl`
