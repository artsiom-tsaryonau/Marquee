# Library transfer (BitTorrent)

An optional way to move your own game library between your own devices, using an embedded BitTorrent client ([`librqbit`](https://github.com/ikatson/rqbit)) in `crates/torrent`. It is **off by default** and is not a way to fetch games from third parties.

## Status

| Part | State |
| --- | --- |
| `marquee-torrent` crate: create `.torrent`, seed, download, progress, limits | Done. Integration test moves a 12 MiB file between two engines on loopback, byte-identical. |
| Setting `torrents` (`enabled`, `seeding`, `upload_kbps`, `download_kbps`) and the `get_torrent_settings` / `set_torrent_settings` API calls | Done. Default is disabled. |
| CLI `marquee torrent ...` | Done and checked end to end between two separate data directories (a 40 MB file, identical on arrival). |
| Settings UI, Tools/Downloads panel, "send to device" actions | Not started. |
| Tested across real machines, with 20–50 GB images, restart/resume, NAT | Not tested. |
| Automatic discovery of your other devices | Not started; today you give the seeder's address. |

## Privacy defaults

The engine starts with public DHT, tracker announces, local-network discovery and UPnP all **off**. Peers are only the addresses you pass in. `.torrent` files made here carry no tracker. Turning the master switch off refuses every `torrent` command that touches the network (`TORRENT_OFF`); turning seeding off keeps the client download-only.

Limitation: torrents created by `librqbit` are not flagged `private`, so anyone who learns the info-hash and can reach your listening port could fetch the data. Keep the port on your LAN or VPN (for example Tailscale or WireGuard) and do not share the magnet link. A private-flag and per-peer allow-list are follow-ups.

BIOS and firmware are not seeded unless you point the command at them.

## VPN

- **Your own devices on one network:** no VPN is needed.
- **Devices at different sites:** use a mesh VPN (Tailscale, or plain WireGuard) rather than a consumer VPN. It gives every device a stable private address, so `--peer 100.x.y.z:51413` works with no port forwarding and the traffic is encrypted. The same network would serve Apollo/Moonlight remote play and the AIRCloud web UIs; it is tracked as the parked **Mesh VPN** item in `aircloud-documentation/initiatives/next-steps.md` (revival reasons noted there, 2026-10-21).
- **ExpressVPN (or any consumer VPN):** useful only if you ever use public torrents. It does not connect your devices to each other and, as far as we know (unchecked), has no port forwarding, so it cannot help a seeder be reached.
- **Planned: `bind_interface` setting.** `librqbit` can bind all sockets to one network interface (`SO_BINDTODEVICE` on Linux). Naming the VPN's interface (for example `tailscale0`) would act as a kill switch: if the interface is gone the client has no network and refuses to start. Not implemented; whether ExpressVPN's Linux app exposes a usable interface is untested.

## CLI

```sh
marquee torrent status                  # settings as JSON
marquee torrent enable                  # master switch (default off)
marquee torrent seeding false           # download-only
marquee torrent limits 2048 0           # KiB/s up, down; 0 = unlimited

# On the device that has the game:
marquee torrent seed "roms/ps2/Game (USA).iso" --listen 0.0.0.0:51413
#   prints the .torrent path (under <data dir>/torrents), a magnet link, and the port

# On another device, after copying the .torrent file (or using the magnet):
marquee torrent get "Game (USA).iso.torrent" --into roms/ps2 --peer 192.168.1.20:51413
```

`seed` runs until interrupted. `get` verifies every piece and exits when the download is complete. Run `marquee scan` afterwards so the game appears in the library.

## Next

1. Settings and Tools UI (enable, seeding, limits, per-game "make available" and "get").
2. A background service in the desktop app that seeds torrents you have marked, with persistence and resume.
3. Peer discovery between your devices (saved addresses first, then mDNS on the LAN).
4. Private-flag torrents and an allow-list of peer addresses.
5. Pairing with remote play (Apollo/Moonlight): "play here after download" versus "play remotely". Apollo's app-registration API is unchecked.
6. Reuse the same flow for [archive intake](ROADMAP.md) and for validating that a received file matches the library hash.
