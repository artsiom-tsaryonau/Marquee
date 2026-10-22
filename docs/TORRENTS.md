# BitTorrent downloads and device transfers

Updated 2026-10-22. The embedded `librqbit` transport supports reviewed local
`.torrent` files, magnets and private device links. Transfers are off by default.
Tracker and DHT discovery are independent opt-in settings. No external torrent
client is required.

## Download, source package and installation are separate

A torrent transfers exactly its listed files. It may contain archives, installer
packages, disc images or already extracted data. Receiving a PS4 `.pkg` does not
install a PS4 game. Marquee currently has no PS4 package-install/game-library
adapter. A completed package download must never be labeled installed or ready
to play.

Downloads use a separate folder per info-hash, normally
`<data dir>/torrents/downloads/<info-hash>/`. They never write directly over an
existing game installation. Completed device transfers with a supported system
hint register their verified download directory as a library root and scan it;
failed scans remain retryable. General torrent downloads stay in their download
folder for the appropriate later import/install workflow.

Saved transfer records include identity, original name, destination, discovery
mode, source magnet/metadata, peers, pause and completion state. **Stop retains
the record, torrent metadata and files.** Missing completed payloads are shown
as source files missing; restart does not silently fetch them again. Explicit
Resume rechecks/redownloads as needed. Presence checks use filenames/sizes;
resuming invokes piece verification. Piece hashes verify transfer integrity,
not publisher authenticity or emulator compatibility.

This is a torrent transfer/history registry, not yet a universal archive and
installation registry. The later model should link source artifacts to separate
installation records (platform, version, patch/DLC, readiness and launch profile).
Deleting original packages may save space after installation is verified, but
seeding the original torrent requires its original files. Installed/extracted
data cannot substitute for a torrent of the packages that produced it.

## UI

Settings → **Torrents and devices**:

1. Enable BitTorrent transfers. Choose upload/seeding policy and speed limits.
2. For a torrent file, choose **Choose .torrent…**. Desktop opens a native picker;
   browser users supply an absolute path on the Marquee computer, not an upload.
3. Review the name, size, files, private flag, tracker count and destination.
   Review is offline. All listed files download; selective-file download is not
   implemented. Large lists preview the first 50 filenames.
4. Enable tracker discovery or DHT if needed, or provide direct peer IP:port
   addresses. Private torrents cannot use DHT; a tracker or direct peer is needed.
5. Choose **Download files**. The backend rereads metadata and checks its complete
   SHA-256 against the review token before starting. Changes require another review.
6. Progress, errors, file location, Pause, Resume/Retry and Stop remain visible.
   Status reconciliation runs across the app while transfers are enabled.

The magnet field accepts public magnets or a link copied from another Marquee
installation's Share action. Links with direct peers use the private engine and
omit public tracker parameters. Public magnets preserve discovery parameters
and require enabled discovery. Metadata resolution times out after 60 seconds
with a retryable error; an unreachable peer is not a completed download.

## Privacy and lifecycle

- Private device shares and public torrent downloads use **separate engines**.
  Turning on public discovery does not publish private-share hashes through DHT
  or trackers. Local multicast discovery and UPnP remain disabled.
- Tracker/DHT options contact their respective networks and expose the client's
  address. These options remain off until selected. Generated device torrents
  currently have no private flag or authenticated peer allow-list: keep device
  links/listening ports on a trusted LAN or VPN. This is not encrypted pairing.
- Upload policy applies to both engines. Speed limits are applied separately to
  the private and public sessions, not as one aggregate cross-session budget.
- Completed downloads may continue seeding when uploads are allowed; receive-only
  completion is shown without claiming that uploading is happening.
- State and cached metadata are written atomically with synchronization and
  private temporary-file permissions. Corrupt state and failed writes are errors,
  never an empty list or a successful mutation. Tracker credentials in metadata
  or magnet links remain backend-side; the review shows a tracker count.
- Resume may overwrite partial files **only in a marked job-owned folder**.
  Unowned folders, symlinks and special files refuse resume. Generic transport
  downloads refuse overwrite by default. Stop never deletes payloads.
- Legacy downloads that pointed directly into library folders are retained but
  refused automatic resume. Existing files are kept; migration to managed
  download folders remains a separate recovery task. Do not delete those files.

## CLI

```sh
# Offline: does not open the library DB or restore/network any existing jobs.
marquee torrent inspect "/path/to/game.torrent"

marquee torrent enable
marquee torrent discovery --trackers true --dht false
marquee torrent seeding false
marquee torrent limits 2048 0            # KiB/s up, down; 0 = unlimited
marquee torrent status                   # settings
marquee torrent list                     # durable jobs and file locations

# Downloads into ~/Downloads/Marquee/<info-hash>, not directly into the parent.
# Optional token is the metadata SHA-256 returned by inspect.
marquee torrent get "/path/to/game.torrent" --into ~/Downloads/Marquee --review-token <token>
marquee torrent get 'magnet:?xt=urn:btih:…' --into ~/Downloads/Marquee
marquee torrent pause <info-hash>
marquee torrent resume <info-hash>
marquee torrent stop <info-hash>           # keeps history and files

# Direct transfer without public discovery:
marquee torrent seed "roms/ps2/Game.iso" --listen 0.0.0.0:51413
marquee torrent get "/path/to/Game.iso.torrent" --into ~/Downloads/Marquee --peer 192.168.1.20:51413
```

`get` waits for verified completion; its timeout pauses the job and preserves
progress. Re-running for the same destination resumes its durable record. The
UI or CLI can resume saved jobs later; background operation lasts only while a
Marquee process remains running. A standalone `resume` command changes durable
state; use a running desktop/web process or `get` to keep the transport alive.
`MARQUEE_TORRENT_HOST` selects the address advertised in private device links,
for example a LAN or mesh-VPN address. Automatic peer discovery is not implemented.

## Verification and remaining work

Synthetic loopback tests cover byte-identical transfer, a real HTTP tracker
announce, offline metadata review, changed-review rejection, private-session
isolation, corrupt/write-failed state, unowned/symlink folders, scan failure/retry,
a partial-download pause/restart/resume, and restart with retained/missing source files followed by explicit redownload.
A production browser check exercises file review, direct-peer download, stop,
reload/history and source-missing Resume using disposable databases. The supplied
Shadow of the Colossus torrent was inspected offline only; its payload was not
fetched or installed.

Still open: native picker/controller acceptance, external-client interoperability,
real multi-machine/NAT and 20–50 GB interruption tests, selective-file download,
a background daemon, interface/VPN binding, authenticated device pairing/private
flags, universal source/installation records, automatic archive/package install
jobs and optional remote-play integration. DHT/public tracker configuration is
implemented; public network availability was not tested in this pass.
