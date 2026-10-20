# Purple for Aion 2

Implemented 2026-10-21 as a narrow, user-requested managed Windows recipe. This starts the compatibility infrastructure without changing the direct-store order: Steam → Epic/Legendary → GOG/gogdl. Lutris is not required or invoked.

## User flow

1. Start the real desktop app or its authenticated browser surface and open **Tools → Purple for Aion 2**.
2. Choose **Install Purple**. Marquee downloads a pinned umu helper, prepares a dedicated GE-Proton/Steam runtime and 64-bit prefix, then installs the recipe dependencies.
3. Complete any dependency installer windows and the official Purple installer. Keep its default installation folder. If Purple starts at the end, close it so setup can finish.
4. Wait for **Launcher installed**, then choose **Open Purple for Aion 2**.
5. Sign in inside Purple, choose your account/region there, then select **Store → Aion 2** and install the game. NCSOFT documents this flow on its [Aion 2 download page](https://aion2.plaync.com/ko-kr/download/index). Game downloads and patching belong to Purple.

No executable, prefix or Wine settings are required in the normal flow. The initial runtime can be a substantial additional download beyond the 388 MiB Purple installer. Host prerequisite: x86-64 Linux with Python 3.11+, a functioning desktop session, Vulkan/graphics drivers and the usual Steam runtime/container support. Runtime/container failures appear in the local setup log; Marquee does not install system packages or change driver configuration.

**Compatibility gate remains open.** NCSOFT's PC requirements list Windows. This implementation does not establish that current Purple sign-in, Aion 2, or its protection software works on Linux. Launcher installation, launcher opening, authenticated game installation, and successful gameplay are separate acceptance steps. Marquee does not bypass Purple/authentication or invent a direct-game URI. No Aion 2 game card, installed-game claim or playtime is fabricated from launcher activity.

## Implemented lifecycle

- Setup is a background core job shared by native and authenticated browser surfaces. Leaving Tools does not cancel it; returning reloads status. The worker retains the core instance lease. The application/server must remain running.
- **Stop setup** terminates the owned setup process group, retaining existing prefix data. A current HTTP download finishes or times out before cancellation takes effect. Detached third-party children may require closing their installer windows. Retry resumes completed dependencies rather than deleting/replacing the prefix.
- Dependency completion is recorded atomically after success. An interrupted receipt write is recoverable from exact entries in `winetricks.log`. An installer returning success without the expected Windows executable is a failure.
- First setup lets umu select/download GE-Proton into Marquee's own runtime tree, then records the concrete runtime path. Subsequent operations use that path, without choosing a new latest runtime. A missing pinned runtime blocks opening instead of silently switching. General runtime upgrade/repair UI is still planned.
- Only one setup/open operation can run for this managed Purple instance. Setup is blocked while Purple is open, and repeated Open is blocked while its supervised process remains active. This does not discover Purple processes started outside Marquee.
- Readiness requires the current recipe receipt, installer completion, a Windows executable header, the pinned runtime and the managed helper. It is a launcher installation check, never an Aion 2 compatibility result.
- Each setup command has a two-hour timeout. Logs rotate on the next stage above 8 MiB; a stage stops if its log exceeds 32 MiB. Setup logs are private. Normal Purple account/session output is discarded; credentials are entered only in Purple and are never requested through Marquee.
- The browser demo displays the card but cannot install/open Purple. Its older Purple wizard choice leads to Tools rather than simulating a successful install. Other Windows recipes remain demo previews.

## Storage and command surface

Under `<Marquee data>/windows/purple/`:

| Path | Purpose |
| --- | --- |
| `prefix/` | Dedicated Wine/Proton prefix, including Purple and any games installed through it |
| `runtime/` | umu runtime and GE-Proton compatibility tools; does not modify the user's Steam or Lutris runners |
| `helpers/umu-1.4.4` | Verified portable Python zipapp |
| `receipt.json` | Recipe version, concrete runtime path, completed steps; atomically replaced |
| `setup.log` | Bounded private setup diagnostics, displayed as a path under Setup details |

Downloads use a temporary directory under this tree and are removed when the job ends. The root is mode 0700; logs/receipt use private file permissions. Existing prefixes are preserved. No database migration is required. Progress/error text and process activity are in-memory; after app restart readiness is reconstructed from the receipt/files, and interrupted steps can be retried. Abrupt application termination is not a supported clean cancellation mechanism.

Shared native/HTTP commands: `purple_status`, `install_purple`, `cancel_purple`, `open_purple`. Setup returns immediately with status; UI polls every 1.5 seconds while Tools is mounted. No generic game session is created for a launcher. Arguments and environment variables are passed directly to `Command`, without a shell.

## Recipe provenance and pins

The dependency sequence comes from [Lutris's Purple V7 recipe](https://lutris.net/games/install/37634/view): `vcrun2010`, `d3dx10`, `dotnet48`, `vcrun2022`, `corefonts`, `arial`. Marquee implements its own orchestration and uses the newer official installer linked by Aion 2. This is a reference recipe, not a compatibility endorsement for Aion 2.

| Component | Pin / verification |
| --- | --- |
| [umu-launcher 1.4.4](https://github.com/Open-Wine-Components/umu-launcher/releases/tag/1.4.4) zipapp tar | `eb590691841f7fad3fc3ad8fd5db4ccb87849fe7948e62b28ece7a4ee48cc851` (release asset digest, downloaded and verified) |
| [Official Purple installer 2.26.1019.7](https://gs-purple-inst.download.ncupdate.com/Purple/PURPLE_Installer_2_26_1019_7.exe) | 406,420,584 bytes; SHA-256 `99b882e490e6fa773eaf60cdfcf73956d0eecbc54435ae35e056062eeac0c52f` (measured 2026-10-21) |

Downloads are size bounded and hashed before execution. Only the expected regular `umu/umu-run` tar member is installed; links/duplicates are rejected. A changed or removed upstream installer must fail rather than fall back to an unchecked latest file. Recipe pins live in `crates/core/src/purple.rs`; changing prerequisites/installer must bump the recipe version and repeat live validation. GE-Proton/Steam runtime and dependency downloads are delegated to upstream umu/winetricks; they are not all independently content-pinned by Marquee. See [umu documentation](https://github.com/Open-Wine-Components/umu-launcher/blob/main/docs/umu.1.scd) for prefix, winetricks and runtime behavior.

## Release acceptance still required

Tests use isolated fake helper processes: dependency failure/resume, receipt recovery, exact argv/environment, cancellation, missing runtime and archive extraction. The real umu zipapp's `--version` was checked and the official installer was downloaded/hashed, but neither a real prefix nor Purple/game was launched during implementation. No Windows EULA was accepted on the user's behalf.

Implementation checks passed: 88 Rust tests across the shared tree, 33 frontend tests, strict Clippy, Svelte check (zero errors/warnings), native desktop compilation and production frontend build. Eight Rust tests exercise the Purple recipe; frontend checks cover its shared command transport, demo restrictions and isolation from emulator install events. Rendered UI inspection was unavailable in this session.

On a disposable desktop profile: complete setup; open/close Purple twice; restart Marquee and reopen; authenticate in the intended region; install Aion 2; verify actual game startup, input, gameplay, exit and updates. Record versions and errors. Do not label Aion 2 supported until those checks pass. Store-account sync and general Windows-app installation remain separate roadmap milestones.
