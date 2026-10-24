# Game soundtracks and music player

Updated 2026-10-25. Local soundtracks under Media and the compact player are implemented.
Jellyfin, yt-dlp/torrent audio acquisition and automatic release matching remain
planned. This stays independent of [streaming acceptance](STREAMING.md).

## Current implementation

Use **Music** in the main navigation to browse/search all albums or create an
album independently of a game. Open its card to add tracks or play. The player
queue supports play-next, append, selecting queued tracks and reordering/removing
upcoming tracks; it remains local to the current app session. See the
[media library foundation](MEDIA_LIBRARY.md#implemented-foundation).

Open a game → **Media** → **Soundtrack** → **Add soundtrack**. Save an album title, optional
composer/artist and HTTPS source page, then **Add tracks**. The native
app opens a multi-file picker. Browser mode asks for absolute paths on the
computer running Marquee; it does not upload browser files. Tracks are added in
filename order. MP3, FLAC, OGG and WAV are accepted for import, with extension and
header checks. Import success does not establish complete decoding or runtime
codec availability.

Use the album's circular **Play** control or select a track. Selecting the current
track pauses/resumes it without fetching it again or resetting its position.
The compact player survives navigation and provides consistent icon transport,
seek, elapsed/duration, mute and volume controls. Previous restarts the current
track after three seconds, otherwise selects the previous track. It skips
unavailable tracks, stops at the end of an album and
reports playback errors. It does not automatically play after restarting the
launcher. Volume and **Pause when a game starts** are saved on the current
surface. Music pauses before game/port launch and opening a stream viewer, and
on observed local session-start events. It never resumes automatically after
game exit or reconnect. The preference can explicitly allow music during play.
An externally opened stream is not currently observable by this player.

The 2026-10-24 controls pass replaces the browser-specific audio widget with
Marquee controls. Track rows show ordinals that become play/pause controls on
hover, focus or active playback; touch devices keep the icons visible. Repeated
filename ordinals are hidden only when they match the displayed track position.
Track removal, album editing/unlinking and player settings live in labelled
options disclosures instead of competing with Play. Disclosures close on Escape,
outside clicks or focus departure. All transport controls have accessible labels
and keyboard focus, while native range inputs preserve keyboard seeking/volume.
Only actual media duration/time events drive progress; there is no fake waveform.

Music has its own accent under **Settings → Appearance → Music accent**. Mint is
the default; presets and a custom color are saved on this surface independently
of the game Play accent. Album Play is circular and 48 px. The compact player
uses a Spotify-inspired 32 px circle, sharp rectangular pause bars and filled
previous/next glyphs; transport targets increase to 44 px on coarse pointers and
48 px in Big Screen. Secondary compact controls are normally 40 px.
Filled transport glyphs omit the rounded outline stroke that previously distorted the
pause bars. Contrast-adjusted icons and selected-track text keep custom colors
readable. The song list separates title/artist, album and duration; small screens
hide the repeated album column. The playing indicator respects reduced motion.

List durations come from bounded local header/container inspection using
[Symphonia 0.5.5](https://docs.rs/symphonia/0.5.5/symphonia/) (MPL-2.0), without
FFmpeg or audio decoding. Probes are limited to 2 MiB of reads, 256 seeks and
64 KiB of metadata, with embedded visuals disabled. Results, including unknown
durations, are cached in `music_media_metadata` against file size/modification
time. Unknown durations show **—** and never prevent playback; the loaded media
element supplies the current track's playback duration. This does not establish
that a codec can be decoded by the desktop/browser runtime.

The **Playback device** popover reports **This device** as current routing. It does not
select an output device or automatically mix music into a game stream. See the
[streamed launcher/audio plan](STREAMING.md#launcher-music-and-media-streaming).

The compact player's Queue, Volume, Device and Options controls share one row.
Volume uses a popup slider/mute control; Queue opens a bounded popover with
outside-click/Escape dismissal. Add tracks is a filled secondary action. Album
images stay within their frames; unlink/storage information appears at the
relevant action rather than as a persistent note after the track list. Extra
OST end padding is removed, while the app still reserves the player's real
height so the final track remains reachable.

Use **Link an existing soundtrack** to reuse an album for another game. Existing
explicit canonical game identities are reused; similar titles are not merged.
Unlinking preserves the album. Editing a shared album updates it for every linked
game. Track names currently come from filenames; embedded text tags and track
renaming/reordering remain later work.

Album artwork is read from embedded audio visuals when importing tracks (first
eight inputs, only when no album image exists). Album options → **Choose album
artwork** also accepts a local JPEG/PNG; removal restores the linked-game fallback
or music placeholder. Existing audio can be reselected through Add tracks to
inspect its embedded image without duplicating tracks. Explicit artwork is never
replaced by later audio imports. Linked-game fallback uses explicit game keys,
not similar names, and is labelled **Game artwork**, not a verified OST cover.
Artwork appears in Music album cards, album details and the player when playback
starts. Changing an image takes effect in a newly selected playback queue.

Source images are bounded to 4 MiB, decoded with 4096-pixel/64 MiB allocation
limits, resized to at most 512 pixels and re-encoded as JPEG (at most 256 KiB).
The normalized image and source label are stored in `music_album_artwork`, without
exposing original paths. Embedded inspection uses the existing 2 MiB read/256-seek
budget with at most 2 MiB of visuals; malformed artwork never blocks audio import.
Manual image validation errors preserve existing artwork. No remote image URL
fetcher, online album matching or implicit browser access is introduced.

An isolated HTTP test imported an FFmpeg-generated MP3 with an embedded PNG,
verified the normalized JPEG/source label, chose a manual cover, and reimported
the same MP3. The manual image remained and no duplicate track was created.
This verifies the actual import/API path; native visual acceptance remains open.

**Find OST on YouTube** opens a title-based search. **View source** opens a saved
page. Neither downloads, embeds or plays that URL, nor claims it is a matching
release. yt-dlp belongs to acquisition phase MU4: a pinned managed helper and
reviewed, cancellable manager jobs. The local player needs no yt-dlp or FFmpeg
installation and does not import browser cookies.

### Storage and media access

`music_albums`, `music_tracks` and `music_links` are separate library tables.
Copied audio lives under private `<data>/music/media` with SHA-256-derived names;
originals remain untouched. Imports allow 200 files, 512 MiB per file and 1 GiB
per batch. Relative paths, symlinks, nonregular files and extension/header
mismatches are rejected. Duplicate content in an album is skipped. Failed
batches publish no partial track list; copies already made can remain unreferenced.

Removing a track deletes its album attachment, including for other games sharing
that album, while keeping copied audio. Unlinking affects only the selected game.
There is no automatic orphan cleanup or disk-deletion action. Missing stored
files leave album/track metadata intact and mark the track unavailable; adding
the original again repairs the file.

Playback uses registered opaque track IDs on a loopback media endpoint, with
ordinary and suffix byte ranges for seeking. No arbitrary filesystem path is
accepted. Read capabilities are scoped to individual tracks and cannot authorize
API changes or other tracks. Removing the track invalidates its lookup. Responses
are uncached and send no referrers. The native app starts its media server on
first playback; its CSP permits loopback audio. Browser playback is same-origin.

### Acceptance

Tests cover persistence/restart, multi-game links, deduplication, failed batch
publication, bounds, symlinks, unsafe source URLs, missing files and separation
from game records. Real HTTP tests check track capabilities, API isolation,
seek/suffix ranges and malformed requests. Player tests check queue order,
unavailable tracks, stale responses, pause during loading, retry and preferences.
The full repository checks include native compilation and Svelte checks/tests/build.

An isolated live HTTP run also imported generated WAV, MP3, FLAC and OGG files,
verified exact copied bytes and partial-range reads, and independently decoded
the fetched media with FFmpeg. It confirmed that no game editions were created.
The server was stopped afterward; fixtures and receipts remain under ignored
`data/music-pilot`. FFmpeg was used only for this test, not by the player.

Visual UI, actual WebKit/browser decoding, audible output, seek and volume still
require live acceptance. No codec is claimed as successfully decoded on those
surfaces based on header/import tests. Physical controller focus awaits hardware.
MU0/MU1 are initial implementations, with acceptance gates still open.

The 2026-10-24 duration pass imported the existing 24 Sonic 2 MP3 files into an
isolated library and compared every parsed duration against the earlier FFmpeg
receipt. All were available, maximum difference was below one microsecond, and
repeat reads returned identical cached results. This tests real MP3 metadata,
not native audible playback. The main library was not modified by that test.

The owner's [YouTube playlist experiment](YOUTUBE_OST.md) tests a selected Sonic 2
soundtrack with a checksum-verified yt-dlp helper, without cookies. That guide
owns the acquisition evidence and URL-first browser-companion integration plan.
It does not turn the current OST source link into a downloader or complete MU4.

## Product experience

The owner-selected direction is **Marquee hosting its own remote media library**,
with a Jellyfin-like browsing experience inside the launcher. The delivery and
security gates are in [the media library roadmap](MEDIA_LIBRARY.md). An optional
Jellyfin connector remains useful later, but is not the primary hosting model.

Add a **Music** section to game details with album artwork, release title,
composer/artist, source and track list. Empty state: “Add a soundtrack” with
“Choose local files” and, once supported, “Find soundtrack” and “Connect Jellyfin”.
Discovery results should say “View release” until playable media is available.

Use a persistent compact player at the bottom of the launcher: artwork/title,
play/pause, previous/next, seek, elapsed/duration, volume and queue. An expanded
view can show the album and track list. Keep focus visible and controls accessible
to keyboard/controller; respect reduced motion. Waveforms require actual analysis
or a clearly decorative treatment, not invented playback data.

Music plays on **this device** by default. Pause or lower launcher music according
to a saved preference when a game/stream starts. Keep receiver audio and host game
audio separate to prevent feedback or accidental music capture. Reconnect must
not unexpectedly restart background music. Playback does not count as gameplay.

## Identity and storage

Model an album/release, tracks and media locations separately from game editions
and installations. One album may link to several editions. Original, remastered
and arranged releases need explicit matching; a title match is a suggestion.
Retain source attribution and manual overrides across rescans. Missing files or
an offline server should make tracks unavailable without deleting the album.

Keep audio attachments out of game-platform preparation. A Steam soundtrack is
a separate application type; owning one does not mean a game is installed.
Torrent source retention/seeding and the playable audio library are also separate.
Use the existing manager for acquisition and preparation rather than creating
another transfer manager.

## Delivery sequence

| Phase | Deliverable | Acceptance |
| --- | --- | --- |
| MU0 | Local album/track/media model, explicit file/folder attachment and manual edition linking | Rescan preserves links; missing and duplicate inputs handled; audio never registered as a game |
| MU1 | Local player and compact/expanded UI; target MP3, FLAC and OGG first | Verify decoding on the actual Tauri WebKit and browser surfaces, seek, queue, volume, errors, persistence and game/stream pause behavior; advertise only tested codecs |
| MU2 | Metadata discovery and reviewed release matching | MusicBrainz/official publisher/store links with attribution; respect service identification/rate limits; metadata results do not claim to contain downloads |
| MU3 | Optional Jellyfin server connection, album browsing and playback | Authentication, token protection, artwork, seek/range requests, reconnect and offline handling tested; avoid a full server crawl on every launch |
| MU4 | Reviewed URL/yt-dlp and torrent audio acquisition | User selects the source and files, reviews size/destination; jobs show progress, cancellation, verification and audio preparation; retained sources follow manager policies |
| MU5 | Optional chip-music playback | Review a decoder such as libgme for supported VGM/VGZ, SPC, NSF or GBS inputs; verify subsongs, loops/durations and exact format support before exposing controls |

WebKit/browser codec support must be measured; `canPlayType` is a hint, not a
successful decode. Add WAV or M4A/AAC only after testing. Do not transcode an MP3
to FLAC and present it as a quality upgrade. Keep imported originals intact.

## Provider boundaries

- **Local files:** use opaque media IDs and narrowly scoped reads. Do not expose
  arbitrary filesystem paths through a browser media endpoint. Bound metadata,
  artwork and playlist parsing; support byte ranges for seeking.
- **Discovery:** show metadata and source links first. Prefer explicit user
  matching over automatically attaching the first similarly named album.
- **Jellyfin:** connects an existing music library/server; it is not a general
  soundtrack search engine or torrent transport. Keep credentials in backend
  secret storage, out of URLs, browser logs and exported playlists. Review media
  proxy/authentication and CSP rather than exposing tokens to arbitrary pages.
- **yt-dlp:** optional, pinned helper with reviewed capabilities and FFmpeg where
  needed. Accept reviewed URLs rather than arbitrary command flags. Preserve
  attribution, file verification and bounded destinations; do not import browser
  cookies or credentials implicitly. Availability differs by source.
- **Torrents:** reuse reviewed manager jobs and verified file selection. Add an
  audio preparation route; do not send music archives to a console installer.
  Copy/prepare playable files independently of retained payloads so seeding does
  not accidentally modify the library or vice versa.

Acceptance includes malformed metadata, path traversal, disconnected storage,
expired authentication, interrupted downloads, unsupported codecs, keyboard and
controller focus, and music behavior during game launch/stream reconnect. No
unrequested music downloads or server connections are part of this plan.

Primary references: [MusicBrainz API](https://musicbrainz.org/doc/MusicBrainz_API),
[Jellyfin music libraries](https://jellyfin.org/docs/general/server/media/music/),
[Steam soundtrack applications](https://partner.steamgames.com/doc/store/application/soundtrackapp),
and [yt-dlp](https://github.com/yt-dlp/yt-dlp).


### Find soundtrack sources

Game → Media → Soundtrack → **Find soundtrack** searches YouTube playlists and
videos. **Preview tracks** shows playlist metadata; **Link source** adds or reuses
a source album for the game. Existing album metadata and tracks are preserved.
Source-only albums need local tracks before they can play in Marquee. This does
not start an audio download; durable yt-dlp acquisition remains planned.
See [YouTube discovery](YOUTUBE_OST.md) for helper and validation details.
