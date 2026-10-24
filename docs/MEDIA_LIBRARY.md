# Marquee media library and remote playback

Updated 2026-10-24. Owner-selected direction: Marquee hosts its own remote media
library with a Jellyfin-like experience alongside games. This is a roadmap;
the implemented OST player serves local desktop/browser use, not remote accounts
or a production LAN media service.

## Implemented foundation

The **Media** destination and game-page Media tab hold explicit local extras
(PNG/JPEG stills and MP4/WebM clips). Disc inspection reads ISO9660 directories
only and copies a selected member; originals stay put. Videos default to spoiler
until revealed. Raw M2V cinematics need conversion; audio pairing is not
implemented. This is not films/series identity or a remote media service.

The **Music** navigation destination browses every saved album, including albums
unlinked from games. Search matches album titles, artists and track names. Open
an album for its existing track list, file import and details controls. **Add
album** creates an independent album without a fabricated game link; game OST
tabs can link it later. Album artwork now supports bounded embedded-image reads
on audio import and explicitly chosen local JPEG/PNG images. A linked game's
cover is a labelled fallback, never an asserted release cover. Embedded text
tags, release discovery and folder rescans remain ML2 work. The
[soundtrack guide](SOUNDTRACKS.md) owns image limits and precedence.

The player's **Queue** control opens the current selection and upcoming tracks.
Track options offer **Play next** and **Add to queue**. Upcoming tracks can move
earlier/later or be removed without restarting the current audio. Selecting a
queued track plays it; selecting the current one toggles pause. Starting another
album replaces the queue. The queue survives page navigation within this app
session but is not saved across app restarts or shared with another device.

The player occupies a full-width bottom footer with a centered utility row and
volume/device/queue popovers. Its measured height reserves
page space and internal sidebar padding, including narrow layouts. The desktop
rail is flush with the window edges and extends behind the player overlay. **Add games** belongs
to the Games library header; **Big Screen** is in the desktop toolbar at the top
right. Narrow desktop navigation sits above the player, and Big Screen hints
also reserve its height. Rendered desktop/responsive/controller acceptance is
still required; these implementation details do not close ML0 or remote gates.

## Local game extras — current use and limits

Open **Media** in the navigation rail to browse collections independently of
playable games. On a game's details page, its **Media** tab shows linked
collections. Create a collection there, or link an existing one. Unlinking keeps
the collection in Media; it never creates or removes a game installation.

- **Add files** copies explicitly chosen PNG/JPEG images and MP4/WebM videos.
  Originals stay untouched. Up to 20 files and 2 GiB per batch; images are limited
  to 16 MiB, bounded to 8192 pixels per side/256 MiB decoded allocation, then
  normalized to PNG at a maximum 2560 pixels per side without upscaling.
- **Browse disc** reads ISO9660 primary directories, including level-3 split
  extents, without mounting or scanning game payloads. A size/mtime/directory
  signature binds selected imports to the inspected image; it is not a content
  authenticity hash. Only a selected member is streamed into managed storage.
  Limits: depth 8, 512 directories, 20,000 directory records, 16 MiB total
  directory data and 2,000 media candidates. Unsupported layouts fail explicitly.
- Images open in a contained gallery with previous/next and fullscreen controls.
  Videos use native controls for seeking, volume and fullscreen. Position is
  persisted periodically and on pause/close. Immediate reopen uses the latest
  position; closing before metadata loads cannot replace an existing resume point
  with zero. Starting video pauses Marquee's OST playback.
- Imported videos default to spoilers. Both titles and playback stay hidden until
  **Reveal videos** is selected. Media details allow editing titles and spoiler
  labels. The MP4/WebM container check is not a codec guarantee: unsupported
  playback produces an explicit conversion message.
- Raw M2V cinematics are indexed but **Needs conversion** prevents importing them
  as playable clips. Automated conversion jobs, companion audio pairing,
  subtitles, generated video posters and video-specific metadata are still open.
  The one-off GoW3 conversion below is an acceptance pilot, not a shipped
  automatic converter.

Additive `game_media_*` tables store independent collection IDs, explicit game
links, opaque asset IDs, source attribution and resume state. Managed copies use
content hashes beneath `<data>/game-media/`. Removing an asset revokes its stream
lookup but keeps original files and retained managed copies; garbage collection
is separate work. HTTP access uses item-scoped capabilities, distinct from music
capabilities and the API bearer token, with byte ranges and HEAD support. Both
native and browser surfaces remain loopback-only; this does not close ML3/ML4.

### GoW3 disc acceptance — 2026-10-24

The owner's extracted BCUS98111 ISO contains **69 media candidates**, including
56 video entries; four are MP4 promotional clips. Their adjacent video metadata
identifies Gran Turismo 5, Heavy Rain, ModNation and MAG. These are disc extras,
not asserted GoW3 gameplay cinematics.

The isolated pilot imported three images and the selected 91,404,648-byte MAG
MP4. It is H.264/AAC, 1920×1080 and 76.777 seconds. A separate selected
68,005,304-byte `CRONOSDEATH.M2V` was prepared manually using installed NVIDIA
NVENC into a 26,555,330-byte H.264 MP4, 1280×720, 33 seconds. That preview is
explicitly **video only**; the separate game audio has not been paired. The source
ISO and archive remain intact, and no bulk cinematic extraction was performed.

Headless Chrome acceptance exercised actual video decoding/playback, spoiler
reveal, image loading, immediate resume at 24 seconds and a 390-pixel layout
without horizontal overflow. Native desktop audio,
fullscreen and interactive acceptance remain separate checks. Isolated logs,
codec probes and screenshots are kept under ignored
`data/manager-pilot/media-ui-pilot/`; no media payloads enter Git.

The main library now contains **God of War III — Disc extras** with two artwork
images, the correctly labelled MAG promo and the GoW3 video-only preview:
121,452,900 managed bytes in total. It has no fabricated PS3 installation/game
link. Open it through Media; a future GoW3 game entry can link this same
collection. This does not make the PS3 title playable.


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
