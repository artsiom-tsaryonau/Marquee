# Game soundtracks and music player

Updated 2026-10-24. Owner-requested feature plan; no music player, Jellyfin adapter
or soundtrack acquisition provider is implemented by this document. Build the
local experience first, without blocking [streaming acceptance](STREAMING.md).

## Product experience

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
