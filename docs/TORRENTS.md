# BitTorrent downloads and device transfers

Updated 2026-10-22. The embedded `librqbit` transport supports reviewed local
`.torrent` files, magnets and private device links. Transfers are off by default.
Tracker and DHT discovery are independent opt-in settings. No external torrent
client is required.

## Background manager and ownership

`marquee-manager` owns transport execution and writes transfer history. Closing
Marquee or a CLI client leaves accepted transfers running. A one-second manager
heartbeat restores eligible opted-in jobs and checkpoints progress without a UI.
Private/public engines, discovery choices, owned destinations and source retention
are preserved. The worker never opens `library.db`; a running/reopened launcher
registers completed private device transfers and retries failed library scans.
General downloads remain source artifacts, not installed games.

On the first torrent command, the client holds/acquires the library instance lease,
backs up the exact legacy history and settings to mode-600
`manager/torrents/legacy-backup.json`, and stages `entries.json` and `settings.json`
in that directory. The last synchronized write replaces `torrents/entries.json`
with an ownership marker. Older embedded writers cannot parse that marker and
fail closed. The manager starts no torrent engine before cutover completes.
An older live launcher blocks adoption. Corrupt records, changed interrupted
migration inputs and unsafe state files are preserved and reported as errors.
Payload folders, `.marquee-transfer-*` markers and cached `torrents/<hash>.torrent`
files stay in their original locations; no source is moved or downloaded by migration.

After cutover, manager settings are authoritative; legacy SQLite values are not
reimported. IPC v5 requires a compatible companion; a requested manager start
can drain an older v1–v4 worker before replacing it. Rollback is manual: stop all
manager/library owners, preserve the current manager state, then reconcile later
changes against the backup before restoring legacy history/settings. Restoring
only the old entry list loses later changes. Never restore it beside a live manager.

The [manager plan](MARQUEE_MANAGER.md) tracks the supported archive handoff and
remaining platform recipes. No general package installer is implemented.

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

## Completed archive → reviewed preparation

Open **Downloads and archives** from Library. Completed torrents appear with their
individual filenames and any linked preparation job. Choose a ZIP, 7z or RAR and
an explicit supported platform, then **Inspect torrent archive**. The manager
rechecks the torrent pieces covering that file (including neighboring files at
piece boundaries), hashes the selected archive, inspects its plan and atomically
links a single review to `(info-hash, relative path)`. The next step is the normal
**Approve and prepare** action; completion alone never extracts or registers it.

The review and immutable receipt retain info-hash, selected path, original download
directory, size and SHA-256. Torrent integrity is verified; release authenticity
and emulator compatibility remain unverified. A changed file, partial transfer,
unknown path, missing boundary file or symlink refuses inspection. Filenames
identifying updates/DLC/firmware/keys and unsupported packages are blocked from
this base-game workflow. A supported-looking extension is still checked by the
archive/platform recipe; it is not sufficient for approval.

Repeated review returns the same job; changing its platform does not silently
replace it. Cancelled reviews can be reopened after validation. Preparation keeps
original torrent files for seeding, and installs into a separate immutable folder.
Library registration remains in Core. Failed registration can recover after
restart or source removal without extracting again or creating another edition;
the source link and receipt survive cleanup. Missing torrent sources remain in
history and require explicit Resume to redownload. Installed files cannot seed a
torrent of its original archives.

Manager job schema **4** backs up schema 2/3 to a private
`jobs-v<version>-backup-<uuid>.db` before advancing without rewriting payloads.
IPC **v4** rejects older clients that would discard torrent provenance; only
shutdown can negotiate v1/v2/v3. Existing reviews without torrent evidence retain
their original identity and receipt fingerprint. Library schema remains 6.

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
   The manager reconciles status even after the app closes.

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
- **VPN for public torrents:** `marquee torrent proxy socks5://127.0.0.1:1080`
  (`--off` removes it; settings field `proxy`) sends the public engine's peer and
  HTTP tracker connections through a loopback SOCKS5 proxy, for example the VPN
  container in platform-generic-scripts `torrent-vpn` (gluetun, ExpressVPN over
  OpenVPN). The proxied session is also bound to the loopback interface, so what
  librqbit would send directly (DHT, UDP trackers, uTP) cannot leave the machine:
  DHT and the inbound listener are off and `udp://` trackers do not respond, which
  leaves HTTP trackers and given peers. If the VPN is down the proxy is gone and
  nothing downloads (fails closed). Only loopback `socks5://` URLs are accepted;
  private device shares never use the proxy.
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
- Pause, Stop and settings changes save live transfer counters/completion before
  removing or resetting engine handles; this does not depend on UI status polls.
  Controls and status reconciliation are serialized with settings changes, and
  asynchronous work from a retired engine cannot publish new transfer state.
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
marquee torrent artifacts                # completed files and linked preparation
marquee torrent prepare <info-hash> Game.zip --system genesis  # inspect; prints review/token
marquee prepare start <job-id> --review-token <token>          # approve and detach

# Downloads into ~/Downloads/Marquee/<info-hash>, not directly into the parent.
# Optional token is the metadata SHA-256 returned by inspect.
marquee torrent get "/path/to/game.torrent" --into ~/Downloads/Marquee --review-token <token>
marquee torrent get 'magnet:?xt=urn:btih:…' --into ~/Downloads/Marquee
marquee torrent pause <info-hash>
marquee torrent resume <info-hash>
marquee torrent stop <info-hash>           # keeps history and files

# Direct transfer without public discovery:
marquee torrent seed "roms/ps2/Game.iso" --listen 0.0.0.0:51413
marquee torrent get "/path/to/<info-hash>.torrent" --into ~/Downloads/Marquee --peer 192.168.1.20:51413
```

`get` waits for verified completion; its timeout pauses the job and preserves
progress. Re-running for the same destination resumes its durable record. The
UI or CLI can resume saved jobs later; execution continues in the manager after
the client exits. `seed` queues a persistent background share and returns; stop it
with `torrent stop <info-hash>`. `create` writes hash-named metadata under
`torrents/created/<info-hash>.torrent` using the manager transport, leaving an
existing imported job’s tracker metadata intact. Seeded jobs cache metadata at
`torrents/<info-hash>.torrent`. All torrent commands
except offline `inspect` use IPC and can run while the launcher owns the library.
The first adoption alone needs the library lease. A custom `seed --listen` binding
is retained in `manager/torrents/private-listen.json`; an existing conflicting
private listener must be stopped before changing its binding. Stop/start the manager
to change the listener, and keep custom ports available.
`MARQUEE_TORRENT_HOST` selects the address advertised in private device links,
for example a LAN or mesh-VPN address. Automatic peer discovery is not implemented.

`manager stop` checkpoints and stops torrents/seeding immediately and drains
accepted archive preparation before exiting. It keeps enabled/paused preferences;
explicit Start resumes eligible jobs. Passive status polling respects the stopped
marker. A per-transfer Stop remains a persistent pause. Existing installed games
can be read/launched with the manager offline.

## Verification and remaining work

Synthetic loopback tests cover byte-identical transfer, download through a test
SOCKS5 proxy (no listener, proxy relays the peer) and no download with the proxy
down, a real HTTP tracker
announce, offline metadata review, changed-review rejection, private-session
isolation, corrupt/write-failed state, unowned/symlink folders, scan failure/retry,
a partial-download pause/restart/resume, completed downloads paused/stopped/disabled
without status polling and reopened with their completion/counters intact, and
restart with retained/missing source files followed by explicit redownload.
Migration/process tests cover legacy-owner refusal, exact private backups and
settings/path retention, unchanged-input crash recovery, manager pause/restart/resume,
client-independent completion checkpoints and no library writes for a synthetic
package. A Core/real-manager test covers private-device registration, failed scan
retry, CLI access while the library is leased, and library reads after manager stop.
An earlier production browser check exercises file review, direct-peer download, stop,
reload/history and source-missing Resume using disposable databases. The supplied
Shadow of the Colossus torrent now has a full receive-only live transport pass
(see below). Its packages remain uninstalled.

Still open: native picker/controller acceptance, external-client interoperability,
real multi-machine/NAT and 20–50 GB interruption tests, selective-file download,
a live public download through the VPN proxy, authenticated device pairing/private
flags, universal source/installation records, unsupported package preparation recipes and optional remote-play integration. DHT/public tracker configuration is
implemented; public network availability was not tested in this pass.

## Shadow of the Colossus live transport acceptance (2026-10-22)

With explicit owner approval, the supplied PS4 torrent downloaded all
**13,584,236,544 bytes**: base package 13,567,262,720 bytes, patch 16,842,752 bytes,
and DLC 131,072 bytes. Info-hash: `4c4622a0531147d7d549a6aa4048db80d5c526b5`.
This isolated test enabled tracker/DHT discovery and disabled uploads; recorded
uploaded bytes were **0**. It ran before the separately added proxy feature and
is not evidence of VPN/proxy routing. The completed transfer was paused and its
manager stopped. All three packages were SHA-256 hashed and retained under
`data/manager-pilot/shadow-torrent-20261022/payload/<info-hash>/`; ignored
`acceptance.json` records sizes/hashes and the blocked preparation result.

The new artifact listing reports all three `.pkg` files as unsupported for archive
preparation; a PS4 preparation request returns `TORRENT_PREPARATION`. No `library.db`
was created in the pilot, and no package was installed/executed. This is a real
13.58 GB transport/integrity pass, not PS4 installation, title authenticity or game
boot acceptance. Payloads remain untracked and the original torrent is unchanged.
