# Cleanup

What can be deleted from a Marquee checkout and its data folders, what it costs, and how to do it. Nothing here touches your games unless stated.

## Git-ignored build and tool output (safe, regenerated)

| Path | Typical size | What it is | Cost of deleting |
| --- | --- | --- | --- |
| `target/debug/incremental` | ~3 GB | Cargo incremental cache | Almost nothing; slightly slower next debug build |
| `target/debug` | ~9 GB | Debug builds and test binaries | Rebuilt by `cargo build` or `cargo test` |
| `target/release` | ~2 GB | Optimized builds used by `scripts/install-emulators.sh` | Rebuilt on next release run (minutes) |
| `target` (everything) | ~12 GB | All of the above | `cargo clean` does the same; slow first rebuild |
| `node_modules/`, `apps/desktop/dist/` | varies | Frontend dependencies and build | `npm install` / rebuild |
| `scripts/vimm/.venv` | ~140 MB | Python virtualenv for the Vimm collector | Recreated by `uv sync` |
| `scripts/vimm/.poc/`, `__pycache__/` | small | Collector scratch | Recreated on use |

Commands (run from the repo root):

```sh
rm -rf target/debug/incremental   # cheapest win
cargo clean                       # everything under target/
```

## Marquee data (outside the repo, `~/.local/share/marquee/`)

| Path | What it is | Notes |
| --- | --- | --- |
| `runners/<id>/` | Emulators installed from `catalog/emulators/` | Re-download from Tools, or `scripts/install-emulators.sh [id...]`. Uninstall via the UI keeps configs. |
| `<id>/xdg-config`, `xdg-data`, `xdg-cache` | Per-emulator settings, saves, caches | **Contains saves and controller settings.** Delete `xdg-cache` freely; keep the rest unless you want a reset. |
| `library.db` | Your library, play history, favourites | Do not delete casually. |
| `library.db.bak-*`, `<id>.bak-*` | One-off backups made during migrations | Safe to remove once the library looks right. |
| `bios-inbox/`, `firmware/` | Imported BIOS and firmware | User-supplied copies; removing them breaks launching those systems until re-imported. |

## Library files inside the repo (git-ignored)

| Path | What it is | Notes |
| --- | --- | --- |
| `roms/` | Your game files | Your data. |
| `roms/_removed/` | Non-game files moved aside by the homebrew clean-up, with `MANIFEST.txt` listing each file and why it moved | Review the manifest, then delete the folder to reclaim the space. Move a file back into `roms/` and rescan to restore it. |
| `roms/fetch.log` | Download log from the collector | Safe to delete. |
| `sample/`, `data/` | Saved Vimm pages and collector data | Needed only to re-run the collector offline. |
| `PS2_bios.zip` (repo root) | Copy of the PS2 BIOS pack | Duplicate once imported into `bios-inbox/`; safe to delete. Keep it out of git (`*.zip` is ignored). |

## Library entries, not files

`marquee prune` removes library entries whose files are gone, but only if they were never played, are not favourites, and have no custom title. Anything you have touched is kept. Run it after moving or deleting game files:

```sh
cargo run -q --release -p marquee-cli -- prune
```

## What the `.gitignore` covers

Game images and disc formats (`*.iso`, `*.chd`, `*.cue`, `*.nes`, `*.gbc`, ...), archives (`*.7z`, `*.tar.gz`, `*.AppImage`), saves and states (`*.sav`, `*.srm`, `*.state`, `*.mcd`), local databases (`*.db*`), logs, `runners/`, `bios-inbox/`, `firmware/`, `.env*`, keys and certificates, and frontend tool caches. Run `git status --ignored` to see what is currently ignored and taking space.
