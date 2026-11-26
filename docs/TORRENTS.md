# Local network game transfers

Updated 2026-11-26. Marquee uses embedded `librqbit` to move your game files
between your devices on a private local network. No external torrent client is
required. Transfers are off by default.

## Network boundary

- Public torrent discovery is unsupported. DHT, tracker announcements,
  multicast discovery, UPnP port forwarding and proxies are disabled.
- Every receive request needs explicit local device peers. Private IPv4,
  IPv4 link-local, IPv6 unique-local/link-local and loopback addresses are
  accepted; globally routed, multicast and unspecified peer addresses are rejected.
- The listener binds to a specific local address. Both incoming connections and
  outgoing connections (including peer-exchange suggestions) pass the engine's
  mandatory local-address allowlist.
- HTTP torrent URLs are rejected. Tracker and alternate-source URLs embedded
  in metadata or pasted magnets are not used.
- This is direct file sharing on your trusted private network, not authenticated
  device pairing. A device with a reachable local address and share link can
  receive the shared files. Off-network sharing is outside this feature's scope.

Changing the old tracker/DHT/proxy fields through the API or CLI cannot enable
public networking. Persisted settings from earlier versions are read with those
fields disabled. Old public jobs retain their files and history but do not start
automatically. Explicit Resume converts a job to the local engine only when its
saved receive peers satisfy the LAN policy. Jobs without a local peer stay offline.
The manager protocol version changed so new clients drain an older worker before
starting one with this policy.

## In the app

1. Open **Games → Downloads and archives** (also available through
   **Settings → Local sharing → Open downloads**).
2. Enable local transfers. In **Settings → Local sharing**, choose whether your
   device may upload, and set the shared upload/download speed limits.
3. On the source device, open a game and choose **Share → Copy link**.
4. Paste that link under **Local device transfers** on the receiving device and
   choose **Receive files**. Failed submissions retain the link and show a
   persistent error beside the field.
5. Progress, transfer rate, connected-device count, Pause/Resume and Stop are
   in the same Downloads panel. The background manager continues accepted work
   when the launcher closes. Stop retains files and history.
6. Supported raw device-shared game files register in the library. For a
   downloaded archive, choose its platform, **Inspect torrent archive**, then
   **Approve and prepare**. Download completion alone does not install a game.

To receive using a saved `.torrent`, choose **Choose .torrent…**, review its files
and destination, and enter the seeding device's private IP and port. The backend
binds review approval to the complete metadata SHA-256 and checks it again before
starting. Tracker addresses in that file are ignored. All files download;
selective-file download is not implemented. File lists have Show more controls.

Stopping the manager checkpoints transfers and stops receiving/sharing. Accepted
archive preparation drains first. Start the manager to resume eligible local jobs.
The original archive stays available for local sharing after preparation.

## CLI

```sh
marquee torrent enable
marquee torrent seeding true
marquee torrent limits 1024 5120  # KiB/s; zero means unlimited

# Seeds in the background manager; uses a private local listener.
marquee torrent seed /path/to/Game.iso
# Choose a specific local interface and ephemeral port when needed:
marquee torrent seed /path/to/Game.iso --listen 192.168.1.20:0

marquee torrent inspect /path/to/game.torrent  # offline, no library opened
marquee torrent get /path/to/game.torrent --into ~/Downloads/Marquee --peer 192.168.1.20:51413
marquee torrent get 'magnet:?xt=urn:btih:…&x.pe=192.168.1.20:51413' --into ~/Downloads/Marquee
marquee torrent list
marquee torrent pause <info-hash>
marquee torrent resume <info-hash>
marquee torrent stop <info-hash>  # retains history and files
marquee torrent artifacts
marquee torrent prepare <info-hash> Game.zip --system genesis
```

`get` waits for verified completion; a timeout pauses it and preserves progress.
Re-running against the same owned destination resumes its durable record.
`create` writes metadata without starting a seed. The legacy `discovery` command
only accepts both options false; `proxy --off` clears an old setting. Neither can
activate public networking.

## Ownership, integrity and preparation

`marquee-manager` owns transfer execution and JSON history without opening the
library database. The launcher registers completed device transfers and retries
failed library publication. General archive downloads remain source artifacts
until an approved preparation produces a receipt.

The initial ownership cutover requires the library lease, preserves the exact
legacy inputs in a private backup, and replaces the embedded writer's history
with an ownership marker. Corrupt records and interrupted or conflicting inputs
are preserved and reported. Downloads use owned per-transfer folders and reject
unsafe paths, symlinks and unowned destinations.

Archive preparation rechecks pieces overlapping the selected file, including
neighboring bytes at piece boundaries. Changed, partial, unsupported and
update/DLC/firmware packages cannot silently become base-game installations.
Reviewed preparation keeps torrent provenance and a receipt so retries and source
cleanup cannot create duplicate library registrations. Integrity does not establish
publisher identity, game authenticity or emulator compatibility.

## Verification

Local tests cover exact-byte transfer, file/folder seeding, tracker non-contact,
public peer/discovery rejection, incoming/outgoing session filtering, pause/resume,
manager restart and legacy adoption, owned destinations, piece verification,
preparation receipts and publication recovery. Rendered UI acceptance uses an
isolated library and synthetic game archive; it does not establish game playability.
