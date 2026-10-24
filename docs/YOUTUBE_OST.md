# YouTube OST experiment and companion integration

Updated 2026-10-24. This is an operator-run acquisition experiment, separate from
the implemented [local OST tab/player](SOUNDTRACKS.md). It is not a new durable
manager transport or a YouTube downloader button in the browser companion.

## Source and tooling

The owner selected [Sonic the Hedgehog 2 OST](https://www.youtube.com/playlist?list=PLvNp0Boas721Im_ZBhvDEuCqzLsRvlKb3),
uploaded by DeoxysPrime: 24 entries, approximately 43 minutes 33 seconds in the
playlist metadata. The watch URL's `list` parameter identifies the full playlist;
the initial `v` parameter identifies its first track. The uploader is not assumed
to be the composer or official release publisher.

The pilot uses the official Linux x86-64 standalone **yt-dlp 2026.10.21**, verified
against the release's `SHA2-256SUMS` and asset digest before execution:
`d8bd33fca9d5b23bd5b9952b3fca36db5c46331933fda9f598a3da14ce2c243f`.
It is stored privately under ignored `data/music-pilot/youtube/tools`, without a
system install. Existing Node 24 supplies the JS runtime, and existing FFmpeg
converts audio to MP3. The standalone includes the matching EJS solver; no remote
components or external plugins are enabled.

Playlist inspection and the initial title-track download succeeded **without
cookies or browser-profile access**. This source does not currently need the
browser extension to lend its authenticated session. Downloading public media
without cookies is the preferred first attempt; cookies are not a universal
requirement or a guarantee of success.

## Acquisition recipe and evidence

The foreground experiment explicitly sets `--ignore-config`, `--no-cookies`,
`--no-cache-dir`, `--no-js-runtimes` plus the selected Node path,
`--no-plugin-dirs` and `--no-remote-components`. It uses bounded connection/retry
settings, a 50 MiB input-file limit, a 3 MiB/s rate limit, fixed index/video-ID
output names and a private download archive. It first tested the 12-second title
track, then requested entries 1–24. There is no automatic sign-in, CAPTCHA action,
cookie extraction, proxy change or browser launch.

Audio-only source files are retained with `--keep-video` (the yt-dlp flag name
also applies to the original audio container). MP3 derivatives are prepared with
readable ordered filenames. Conversion does not improve source fidelity. Each
track is checked against its expected video ID, decoded completely with FFmpeg,
probed for audio/duration and hashed. Local hashes and decoding are transport/
structure evidence, not an official soundtrack checksum or rights attestation.

Private metadata, raw media, derivatives, resume history and receipts live under
`data/music-pilot/youtube`; they must not be committed. Raw extractor metadata can
include transient media URLs and should not be exported as a public report.
Do not delete or overwrite these sources when attaching copied audio to Marquee.
The existing OST importer owns album/track registration and copies playable audio
into the library's music storage, independently of game scanning.

All 24 tracks downloaded without cookies. Complete MP3 decoding, expected video
IDs, duration checks and hashes passed: 62,495,502 bytes of prepared MP3 audio,
approximately 43 minutes 22 seconds of probed audio. The original audio containers
were retained. The owner explicitly chose attachment to the main Sonic 2 entry.
After a private database backup, Marquee's normal library lease and authenticated
OST API registered one album with all 24 tracks available, in playlist order.
First/last track media responses matched their prepared hashes, and the game
edition records were unchanged. The temporary import server stopped afterward.
This proves acquisition, local decode and library/media handoff, not audible
playback in the native WebKit player; that remains a separate listening check.

## Extension integration: URL first

The current FMHY companion uses `activeTab`, scripting, native messaging and
Chrome download tracking for Vimm/Axekin. It has no `cookies` permission and its
typed provider protocol does not accept YouTube as a game provider. Do not feed
a playlist into Vimm's numeric game ID or treat audio as a console archive.

The next integration should be:

1. A user-invoked **Send soundtrack to Marquee** action reads only the selected
   YouTube tab URL. Validate HTTPS, exact allowed host/path, playlist/video IDs,
   duplicate query parameters, length and credentials; normalize the URL and
   discard irrelevant tracking parameters.
2. The paired native bridge forwards a typed music-source request to the hidden
   manager. It never accepts arbitrary shell flags or an unrestricted executable.
   This can reuse the existing paired channel, not the game-provider job shape.
3. The manager's pinned helper inspects metadata without cookies. Marquee shows
   playlist title/uploader, ordered track selection, duration and size estimates
   (clearly estimates), destination, output format and target game/album.
4. Approval binds the chosen source/video IDs and helper version. Durable jobs
   support progress, cancel, interruption/retry, duplicate prevention and verified
   audio preparation. Closing the launcher must not silently terminate accepted
   work. Per-track failures remain visible; partial success is not a complete album.
5. Publish a receipt to the OST importer containing provenance, ordered tracks,
   original/derived file hashes and the explicit game/album link. Keep source
   retention separate from player copies and preserve manual album metadata.

No additional browser permission is needed for the URL-only handoff. The browser
does not need to perform the media download; the manager should own yt-dlp.
Already downloaded MP3s work through **OST → Choose local files** today.

## Optional authenticated fallback

Only add a cookie handoff if an observed source requires it and the user chooses
that route. Prefer a fresh explicit grant for the selected YouTube session over
silently reading an entire Chrome/Firefox profile with `--cookies-from-browser`.
That CLI option can read more browser cookies than the selected site needs.

An extension fallback needs optional `cookies` and narrowly scoped YouTube host
permissions, requested during the user's action. Export only relevant cookies
through the paired native channel to a private temporary Netscape-format jar.
Do not put values in URLs, arguments, logs, job databases, extension storage,
receipts or exports. Use an opaque short-lived secret reference; remove the jar
on success, cancellation or expiry, and require renewed authorization to resume
an expired authenticated job. Do not broaden to all sites or Google account
cookies implicitly. Site restrictions may still prevent downloading.

The current companion's permissions and live registration are unchanged. The
cookie fallback, typed music manager jobs and popup handoff are still planned;
this successful public-playlist experiment does not establish those capabilities.

Primary references: [official release](https://github.com/yt-dlp/yt-dlp/releases/tag/2026.10.21),
[EJS/runtime setup](https://github.com/yt-dlp/yt-dlp/wiki/EJS),
[cookie handling FAQ](https://github.com/yt-dlp/yt-dlp/wiki/FAQ#how-do-i-pass-cookies-to-yt-dlp),
[Chrome cookie API permissions](https://developer.chrome.com/docs/extensions/reference/api/cookies).
