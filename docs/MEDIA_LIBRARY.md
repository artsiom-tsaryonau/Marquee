# Marquee media library and remote playback

Updated 2026-10-24. Owner-selected direction: Marquee hosts its own remote media
library with a Jellyfin-like experience alongside games. This is a roadmap;
the implemented OST player serves local desktop/browser use, not remote accounts
or a production LAN media service.

## Product shape

Add a top-level **Music** destination with albums, artists, soundtracks and a
searchable track view. Game OST tabs open the same albums and playback queue;
albums also remain reachable when unlinked from a game. Keep games and music
visually related, with separate Play accents and explicit actions. Use album
artwork, consistent metadata, clear unavailable states and a persistent player.

Start with music. Films, series and other media require their own identity,
metadata and playback work before joining the library. A connection to an
existing Jellyfin server is an optional provider, not a prerequisite for Marquee
hosting its library. Reuse the existing core and manager rather than building a
second catalogue or acquisition manager.

## Delivery and acceptance

| Priority | Work | Acceptance before expanding |
| --- | --- | --- |
| ML0 | Finish local music acceptance | Native app playback, seek, volume, pause on game launch, navigation and error behavior accepted on the actual desktop; keyboard and responsive layouts checked |
| ML1 | Global Music library and queue | All saved albums browsable independently of games; search/filter, queue inspection/reorder and play-next; album/game links survive edits and rescans |
| ML2 | Library folders, metadata and artwork | Reviewed folders and incremental rescans; bounded tag/art parsing, album/artist identity and explicit overrides; missing files preserved; artwork ownership/source retained |
| ML3 | Remote hosting foundation | Explicit opt-in service, paired/revocable clients and protected authentication; TLS or documented trusted private transport; authorization for catalogue and media, bounded opaque IDs, byte ranges, expiry/reconnect and multi-client isolation tested |
| ML4 | Direct remote music client | Authenticated browse/search/artwork/queue and playback on a second device; seeking, offline/expired sessions and codec negotiation tested; credentials absent from logs and exported media URLs |
| ML5 | Session and output controls | Show where music plays; separate queues by playback session; explicit transfer/control of an authorized session, with pause/reconnect behavior tested and no accidental host playback |
| ML6 | Optional providers and expansion | Jellyfin adapter, reviewed acquisition jobs and additional media types only after their own interoperability, format and resource limits pass |

Metadata and artwork improve recognition; they never gate local Play. Follow
[Jellyfin's music guidance](https://jellyfin.org/docs/general/server/media/music/)
on tags, album organization and artwork as a reference, while preserving manual
Marquee links rather than merging releases merely because their names match.

## Two ways to reach Marquee

**Direct media playback** sends catalogue data and authorized audio to a remote
client, which decodes it and owns its playback position. It needs the ML3/ML4
service and client work; the current localhost music endpoint is not sufficient.
Do not expose the existing developer API bearer token as remote account access.
Keep authentication separate from short-lived, narrowly scoped media access,
and review cross-origin policy and proxy behavior before enabling clients.

**Stream Marquee** captures a launcher desktop and its audio for a Moonlight-like
receiver. Music must run inside that captured desktop/audio session. It is useful
for browsing and launching games remotely, but does not provide an independent
audio-only library client. Follow the existing
[streaming/audio hosting plan](STREAMING.md#launcher-music-and-media-streaming).
Never silently capture the user's unrelated host desktop audio.

Original imports, playable media and retained download/torrent payloads remain
separate. Acquisition follows the durable manager roadmap; playback should not
depend on retaining an archive or keeping a torrent active. This plan creates
no network listener, account or background download.
