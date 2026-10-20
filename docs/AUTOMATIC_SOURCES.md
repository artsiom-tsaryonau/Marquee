# Legacy Lutris library bridge

Updated 2026-10-21. **Priority superseded:** the user wants Marquee to replace the separate library-manager role, and does not need existing Lutris configurations imported. The authoritative direction is [direct Steam → Epic/Legendary → GOG/gogdl](ROADMAP.md), with [Marquee-managed compatibility](STORE_INTEGRATION.md).

This file documents the bridge already implemented during the earlier Lutris-first approach. Keep it as optional legacy functionality; do not expand it into a required source, installer, account-login flow, or Wine-management dependency. Existing code still auto-discovers a local database. Moving that behavior behind an explicit optional integration is a planned code change, not something this documentation update already does.

## Implemented now

- On startup, Marquee detects an existing native or Flatpak Lutris `pga.db`, selects the corresponding launcher preset, and reads the library automatically. No opt-in path/configuration wizard is required for an existing detected installation.
- An explicit saved disconnect takes precedence over automatic discovery. Advanced settings allow exceptional paths, a different backend command, and installed-only mode.
- Configured installations and cached GOG/Epic/Steam listings enter the unified library with separate store identity and Lutris launch provenance. Cached listings stay uninstalled; their platform stays unknown when the cache does not provide a verified platform.
- Matching configured and cached entries uses **service plus product ID**, never title or product ID alone. Database paths contribute to entry IDs so different Lutris databases cannot collide. Multiple configured copies remain distinct. Transitioning from a cached entry to a configured installation currently changes its projection ID; durable provider-to-installation identity mapping remains follow-up work.
- Existing Lutris playtime is converted from hours to minutes. Marquee does not credit the short launcher handoff as gameplay.
- Installed games hand off through `lutris:rungameid/<id>`. Supported cached store entries hand off through Lutris's service-install URI, with the service/product payload encoded as a single path segment. This opens the existing Lutris installation flow; it does not guarantee an unattended download/install or bypass store authentication.
- The source is read-only. Marquee reads selected `games` and `service_games` fields; it does not read store tokens, cookies, Wine YAML, or cached `details` payloads and does not migrate/write the source database.
- The view refreshes when Marquee regains focus and every minute while visible and focused. New installations and Lutris playtime appear after that refresh. This reads the local cache; it does not initiate an online store sync.
- The library renders 48 cards at a time with Show more, while search/filter/count operate over the complete imported view.
- Tools shows source status and counts. Database and command controls are tucked under Advanced settings. Disconnect removes only the imported view; source files, installed games, and other Marquee libraries remain intact.

The adapter currently handles one selected Lutris database at a time. Native is preferred when both native and Flatpak databases exist; Advanced settings can select the other. Marquee does not silently mix prefixes from different installations.

## Legacy use and capability reference

Lutris provides existing working runner configurations, compatibility settings, community installer recipes, store-library integrations, and install/launch orchestration. The bridge can reuse those configurations for users who explicitly choose it. This user does not require that migration path. Direct store connections and Marquee-managed Wine/Proton setup must work without Lutris; no bridge capability is a prerequisite.

Sources checked on 2026-10-21:

- [Official Lutris overview and CLI](https://github.com/lutris/lutris): runners, installers, per-game settings, and local library database.
- [Official FAQ](https://lutris.net/faq): supported store ecosystem and runtime purpose.
- [Database schema](https://github.com/lutris/lutris/blob/master/lutris/database/schema.py): configured games and service cache are separate tables.
- [GOG service](https://github.com/lutris/lutris/blob/master/lutris/services/gog.py) and [Epic service](https://github.com/lutris/lutris/blob/master/lutris/services/egs.py): native service integration code.
- [CLI handling](https://github.com/lutris/lutris/blob/master/lutris/gui/application.py) and [URI parsing](https://github.com/lutris/lutris/blob/master/lutris/api.py): numeric launch IDs and store-install handoffs.

Lutris's public game/installer catalogue is not a universal game-download entitlement. Connected store caches reflect previously synced account listings and may be stale.

## Remaining legacy-only work

- Move automatic discovery behind an explicit legacy-source choice, outside normal onboarding. Preserve all user files, installed games, prefixes, and existing settings when changing the default.
- Keep regression coverage so the optional bridge does not break existing users.
- Defer further account sync, new installer handoffs, backend installation, or migration features for this adapter.

The former proposal to build core setup and store-login automation around Lutris is superseded. Shared store/runtime implementation tasks now live in [STORE_INTEGRATION.md](STORE_INTEGRATION.md). The normal UX is connect Steam/Epic/GOG → install → play in Marquee; helper selection and paths are advanced settings.

## Verification

Five adapter regression tests pass independently: automatic native/Flatpak defaults, store-aware matching, installed-only/disconnect handling, nonmutating reads, encoded handoff URIs, and schema/missing-file behavior. The actual machine's adapter probe found 4 installed entries and 1,164 cached store listings (651 Epic, 215 GOG, 298 Steam), producing 1,167 entries after matching. The existing database checksum was unchanged after the probe.

Frontend state/navigation tests pass, as do Svelte check and production build. No real game, store installer, or account-login flow was executed. Browser visual verification remains unavailable in this tool session. Full workspace verification must account for concurrent PS1 changes in the shared tree; the independent adapter harness does not validate those changes.
