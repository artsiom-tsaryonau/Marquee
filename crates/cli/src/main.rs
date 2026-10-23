use clap::{Parser, Subcommand};
use marquee_core::error::AppError;
use marquee_core::model::CoreEvent;
use marquee_core::model::SessionState;
use marquee_core::{Core, FnSink, NoopSink};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "marquee", about = "Marquee library diagnostics")]
struct Args {
    /// Isolated data directory (never the real user DB in tests)
    #[arg(long)]
    data_dir: Option<PathBuf>,
    /// Install the in-process test runner (not for real play)
    #[arg(long)]
    allow_fake: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Native local streaming controls; does not open the library or start Wolf
    Streaming {
        /// Absolute path to Wolf's Unix API socket (WOLF_SOCKET_PATH)
        #[arg(long)]
        socket: PathBuf,
        #[command(subcommand)]
        command: StreamingCmd,
    },
    /// Paired browser worker bridge; never opens library.db
    #[command(subcommand)]
    Browser(BrowserCmd),
    /// Manage the hidden worker without opening or locking the library database
    #[command(subcommand)]
    Manager(ManagerCmd),
    /// Durable reviewed local-archive preparation jobs (manager M1)
    #[command(subcommand)]
    Prepare(PrepareCmd),
    #[command(subcommand)]
    Catalogue(CatalogueCmd),
    #[command(subcommand)]
    Library(LibraryCmd),
    Scan {
        #[arg(long)]
        root: Option<String>,
    },
    /// Unpack downloaded .zip/.7z/.rar game archives into the library, then rescan.
    /// Pass archive files or folders that contain them.
    Intake {
        paths: Vec<PathBuf>,
        /// Delete each archive after it unpacked and verified
        #[arg(long)]
        delete: bool,
        /// Do not rescan the library afterwards
        #[arg(long)]
        no_scan: bool,
    },
    /// Remove entries whose files are gone (never played, not favourites).
    Prune,
    List {
        #[arg(long)]
        system: Option<String>,
        #[arg(long)]
        json: bool,
    },
    #[command(subcommand)]
    Runners(RunnersCmd),
    #[command(subcommand)]
    Artwork(ArtworkCmd),
    #[command(subcommand)]
    Ports(PortsCmd),
    #[command(subcommand)]
    Bios(BiosCmd),
    #[command(subcommand)]
    Emulators(EmulatorsCmd),
    /// Optional private transfer of your library between your own devices
    #[command(subcommand)]
    Torrent(TorrentCmd),
    Launch {
        edition_id: String,
    },
    /// Run Marquee: `run web` serves the UI and API on 127.0.0.1 without a window
    #[command(subcommand)]
    Run(RunCmd),
    /// Read-only native/Flatpak Steam clients, library folders and installed manifests
    SteamInspect,
    Health,
}

#[derive(Subcommand)]
enum ManagerCmd {
    Status,
    Start,
    Stop,
}

#[derive(Subcommand)]
enum StreamingCmd {
    /// Inspect advertised apps, paired clients and active sessions (keys omitted)
    Inspect,
    /// Show pending client addresses (pairing secrets omitted)
    Pending,
    /// Accept a pending client's PIN; read the four digits from stdin, never argv
    Pair {
        #[arg(long)]
        client_ip: String,
    },
    /// Revoke the selected paired client's access
    Unpair { client_id: String },
    /// End the selected stream session; this can also stop its running game
    Stop { session_id: String },
}

#[derive(Subcommand)]
enum BrowserCmd {
    /// Recheck completed bytes and bind one platform review; never starts preparation
    Inspect {
        id: String,
        #[arg(long)]
        system: String,
    },
    /// Read a typed browser operation from bounded stdin (no credentials)
    Request {
        #[arg(long)]
        client_id: String,
    },
    List,
    /// Request cancellation; the connected browser must acknowledge it
    Cancel {
        id: String,
    },
}

#[derive(Subcommand)]
enum PrepareCmd {
    /// Accept a reviewed job and detach; registration happens when Marquee reconnects
    Start {
        id: String,
        #[arg(long)]
        review_token: String,
    },
    /// Inspect an archive and persist its review; writes no game files
    Inspect {
        path: PathBuf,
        #[arg(long)]
        system: String,
    },
    List,
    /// Prepare/register a reviewed job, or retry its interrupted phase
    Run {
        id: String,
        #[arg(long)]
        review_token: String,
    },
    /// Cancel before output preparation (keeps source archives)
    Cancel {
        id: String,
    },
}

#[derive(Subcommand)]
enum CatalogueCmd {
    /// Import provider metadata exported from saved HTML (never installs games)
    Import {
        path: PathBuf,
    },
    List,
    Match {
        edition_id: String,
    },
    Link {
        edition_id: String,
        record_id: String,
        media_id: String,
    },
    Unlink {
        edition_id: String,
    },
}

#[derive(Subcommand)]
enum RunCmd {
    /// Serve the UI and API for a browser (the desktop app is `scripts/run.sh desktop`)
    Web {
        #[arg(long)]
        port: Option<u16>,
        /// Built UI directory
        #[arg(long, default_value = "apps/desktop/dist")]
        web_dir: PathBuf,
    },
}

#[derive(Subcommand)]
enum TorrentCmd {
    /// Show whether transfer is enabled and its limits
    Status,
    /// Inspect a local torrent without contacting trackers or peers
    Inspect {
        path: PathBuf,
    },
    /// List durable transfers and download locations
    List,
    /// List completed torrent files and their linked preparation reviews
    Artifacts,
    /// Inspect a completed archive for a platform; approval remains separate
    Prepare {
        info_hash: String,
        path: PathBuf,
        #[arg(long)]
        system: String,
    },
    /// Set opt-in discovery for imported torrents (private device shares are unaffected)
    Discovery {
        #[arg(long, action = clap::ArgAction::Set)]
        trackers: bool,
        #[arg(long, action = clap::ArgAction::Set)]
        dht: bool,
    },
    Pause {
        info_hash: String,
    },
    Resume {
        info_hash: String,
    },
    Stop {
        info_hash: String,
    },
    /// Turn library transfer on
    Enable,
    /// Turn library transfer off
    Disable,
    /// Allow or stop serving pieces to other devices
    Seeding {
        #[arg(action = clap::ArgAction::Set)]
        on: bool,
    },
    /// Limit speeds in KiB/s (0 is unlimited)
    Limits {
        upload: u32,
        download: u32,
    },
    /// Send public torrents through a loopback SOCKS5 proxy (e.g. socks5://127.0.0.1:1080 from
    /// platform-generic-scripts torrent-vpn); nothing else leaves the machine. --off removes it
    Proxy {
        #[arg(required_unless_present = "off")]
        url: Option<String>,
        #[arg(long, conflicts_with = "url")]
        off: bool,
    },
    /// Make a .torrent for a game file or folder (saved under the data dir)
    Create {
        path: PathBuf,
    },
    /// Seed a file or folder in the background manager; prints the address and magnet
    Seed {
        path: PathBuf,
        /// Address to accept peers on
        #[arg(long, default_value = "0.0.0.0:51413")]
        listen: std::net::SocketAddr,
    },
    /// Download a .torrent file or magnet link from your other device
    Get {
        source: String,
        #[arg(long)]
        into: PathBuf,
        /// Address of the device seeding it (host:port); repeatable
        #[arg(long = "peer")]
        peers: Vec<std::net::SocketAddr>,
        #[arg(long, default_value_t = 86400)]
        timeout_secs: u64,
        /// Metadata SHA-256 from torrent inspect; rejects a file changed after review
        #[arg(long)]
        review_token: Option<String>,
    },
}

#[derive(Subcommand)]
enum EmulatorsCmd {
    /// Preview the verified DuckStation profile and detected host (no settings writes)
    Profile { id: String },
    /// Apply a profile: recommended, custom, or disabled (preserves current values)
    ProfileSet {
        id: String,
        #[arg(long, default_value = "recommended", value_parser = ["recommended", "custom", "disabled"])]
        mode: String,
        #[arg(long, default_value = "Automatic", value_parser = ["Automatic", "Vulkan", "OpenGL", "Software"])]
        renderer: String,
        #[arg(long, default_value_t = 1)]
        scale: u8,
        #[arg(long)]
        vsync: bool,
    },
    /// Emulators Marquee can install, and what is installed
    List,
    /// Ask GitHub whether installed emulators have a newer version
    Check,
    /// Install or update an emulator inside Marquee's own folder
    Install { id: String },
    /// Install every emulator your library folders need and that is not installed yet
    InstallNeeded,
    /// Build a reviewed pinned recipe with rootless Podman; wait and report logs
    Build { id: String },
    /// Last source-build status (including interrupted builds)
    BuildStatus { id: String },
    /// Restore the previous source-built program, preserving saves
    Rollback { id: String },
    /// Open an installed emulator's own window
    Open { id: String },
    /// Remove an emulator's program (settings and saves stay unless --purge-data)
    Uninstall {
        id: String,
        #[arg(long)]
        purge_data: bool,
    },
}

#[derive(Subcommand)]
enum BiosCmd {
    /// BIOS images Marquee holds and the region of each
    List,
    /// Copy every recognisable BIOS from a folder or file into Marquee
    Import { path: PathBuf },
    /// Delete every BIOS Marquee holds for ps1 or ps2
    Remove { system: String },
}

#[derive(Subcommand)]
enum PortsCmd {
    /// Catalogue and install state (offline)
    List,
    /// Ask GitHub whether installed ports have a newer release
    Check,
    /// Install or update a port from its official GitHub release
    Install { id: String },
    /// Start an installed port
    Launch { id: String },
}

#[derive(Subcommand)]
enum LibraryCmd {
    AddRoot {
        #[arg(long, default_value = "ps2")]
        system: String,
        path: PathBuf,
    },
}

#[derive(Subcommand)]
enum RunnersCmd {
    List {
        #[arg(long)]
        json: bool,
    },
    Select {
        runner_id: String,
    },
    Setup {
        runner_id: String,
    },
    Install {
        #[arg(long, default_value = "auto")]
        channel: String,
    },
    /// Delete the PCSX2 Marquee installed (settings, saves and BIOS stay)
    Uninstall,
}

#[derive(Subcommand)]
enum ArtworkCmd {
    Refresh {
        #[arg(long)]
        edition: Option<String>,
    },
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{}: {}", err.code, err.message);
            if let Some(ctx) = err.context {
                eprintln!("{}", ctx);
            }
            ExitCode::FAILURE
        }
    }
}

fn streaming(socket: &std::path::Path, command: &StreamingCmd) -> marquee_core::error::Result<()> {
    use marquee_core::streaming::wolf::WolfClient;
    use std::io::Read;

    let client = WolfClient::new(socket)?;
    let value = match command {
        StreamingCmd::Inspect => serde_json::to_value(client.inspect()?)?,
        StreamingCmd::Pending => serde_json::to_value(client.pending()?)?,
        StreamingCmd::Pair { client_ip } => {
            let mut bytes = Vec::new();
            std::io::stdin().take(65).read_to_end(&mut bytes)?;
            if bytes.len() > 64 {
                return Err(AppError::new(
                    "STREAMING_PIN",
                    "Enter only the four-digit PIN shown by Moonlight",
                ));
            }
            let pin = std::str::from_utf8(&bytes)
                .map_err(|_| AppError::new("STREAMING_PIN", "Enter a four-digit PIN"))?;
            client.pair(client_ip, pin.trim())?;
            serde_json::json!({"accepted": true, "message": "PIN accepted by Wolf; confirm pairing in Moonlight"})
        }
        StreamingCmd::Unpair { client_id } => {
            client.unpair(client_id)?;
            serde_json::json!({"accepted": true})
        }
        StreamingCmd::Stop { session_id } => {
            client.stop_session(session_id)?;
            serde_json::json!({"accepted": true, "message": "Stop requested; inspect sessions to confirm exit"})
        }
    };
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}

fn run() -> marquee_core::error::Result<()> {
    let args = Args::parse();
    // Streaming must remain usable while the desktop holds the library lease.
    // No data-directory resolution, manager start or transfer recovery here.
    if let Command::Streaming { socket, command } = &args.command {
        return streaming(socket, command);
    }
    // Inspect must stay offline even if the selected library has enabled jobs.
    // Do this before Core::open can restore transfers or acquire its DB lease.
    if let Command::Torrent(TorrentCmd::Inspect { path }) = &args.command {
        let (manifest, token) = marquee_core::transfer::inspect_file(path)?;
        let mut value = serde_json::to_value(&manifest)?;
        value["preparation_files"] =
            serde_json::to_value(marquee_manager::torrent::preparation_files(&manifest))?;
        value["review_token"] = token.into();
        println!("{}", serde_json::to_string_pretty(&value)?);
        return Ok(());
    }
    if matches!(&args.command, Command::Torrent(_)) {
        let paths = marquee_core::paths::Paths::resolve(args.data_dir.clone())?;
        let client = marquee_manager::Client::new(&paths.data_dir);
        if let Command::Torrent(command) = args.command {
            return torrent(&client, command);
        }
    }
    // Worker controls and preparation handoff must work while the launcher
    // owns library.db, and cannot start recovery/network work in Core.
    match &args.command {
        Command::Browser(command) => {
            use std::io::Read;
            let paths = marquee_core::paths::Paths::resolve(args.data_dir.clone())?;
            let client = marquee_manager::Client::new(&paths.data_dir);
            let value = match command {
                BrowserCmd::Request { client_id } => {
                    let mut bytes = Vec::new();
                    std::io::stdin()
                        .take(256 * 1024 + 1)
                        .read_to_end(&mut bytes)?;
                    if bytes.len() > 256 * 1024 {
                        return Err(AppError::new(
                            "BROWSER_REQUEST",
                            "Browser request exceeds 256 KiB",
                        ));
                    }
                    client.browser_request(client_id, serde_json::from_slice(&bytes)?)?
                }
                BrowserCmd::List => serde_json::to_value(client.browser_jobs()?)?,
                BrowserCmd::Inspect { id, system } => {
                    serde_json::to_value(client.inspect_browser(id, system)?)?
                }
                BrowserCmd::Cancel { id } => {
                    serde_json::to_value(client.request_browser_cancel(id)?)?
                }
            };
            println!("{}", serde_json::to_string(&value)?);
            return Ok(());
        }
        Command::Manager(command) => {
            let paths = marquee_core::paths::Paths::resolve(args.data_dir.clone())?;
            let client = marquee_manager::Client::new(&paths.data_dir);
            let status = match command {
                ManagerCmd::Status => client.status()?,
                ManagerCmd::Start => client.start_manager()?,
                ManagerCmd::Stop => client.stop()?,
            };
            println!("{}", serde_json::to_string_pretty(&status)?);
            return Ok(());
        }
        Command::Prepare(PrepareCmd::Inspect { path, system }) => {
            let paths = marquee_core::paths::Paths::resolve(args.data_dir.clone())?;
            let client = marquee_manager::Client::new(&paths.data_dir);
            println!(
                "{}",
                serde_json::to_string_pretty(&client.inspect(path, system)?)?
            );
            return Ok(());
        }
        Command::Prepare(PrepareCmd::Start { id, review_token }) => {
            let paths = marquee_core::paths::Paths::resolve(args.data_dir.clone())?;
            let client = marquee_manager::Client::new(&paths.data_dir);
            println!(
                "{}",
                serde_json::to_string_pretty(&client.start_job(id, review_token)?)?
            );
            return Ok(());
        }
        _ => {}
    }
    let core = Arc::new(Core::open(args.data_dir, args.allow_fake)?);
    match args.command {
        Command::Streaming { .. } => unreachable!("handled without Core"),
        Command::Browser(_) => unreachable!("handled without Core"),
        Command::Manager(_) => unreachable!("handled without Core"),
        Command::Prepare(command) => {
            let value = match command {
                PrepareCmd::Start { .. } => unreachable!("handled without Core"),
                PrepareCmd::Inspect { path, system } => {
                    serde_json::to_value(core.inspect_preparation(&path, &system)?)?
                }
                PrepareCmd::List => serde_json::to_value(core.preparation_jobs()?)?,
                PrepareCmd::Cancel { id } => serde_json::to_value(core.cancel_preparation(&id)?)?,
                PrepareCmd::Run { id, review_token } => {
                    let job = core.run_preparation(&id, &review_token)?;
                    println!("{}", serde_json::to_string_pretty(&job)?);
                    if job.state != marquee_core::preparation::State::Registered {
                        return Err(AppError::new(
                            job.error_code.as_deref().unwrap_or("PREPARATION_FAILED"),
                            job.error
                                .unwrap_or_else(|| "Preparation needs attention".into()),
                        ));
                    }
                    return Ok(());
                }
            };
            println!("{}", serde_json::to_string_pretty(&value)?);
        }
        Command::Catalogue(command) => match command {
            CatalogueCmd::Import { path } => {
                use std::io::Read;
                let mut bytes = Vec::new();
                std::fs::File::open(path)?
                    .take(900_001)
                    .read_to_end(&mut bytes)?;
                if bytes.len() > 900_000 {
                    return Err(AppError::new("METADATA", "Snapshot exceeds 900 KB"));
                }
                let result = core.import_catalogue(serde_json::from_slice(&bytes)?)?;
                println!(
                    "Imported {} catalogue records and {} source systems",
                    result.records.len(),
                    result.systems.len()
                );
            }
            CatalogueCmd::List => println!(
                "{}",
                serde_json::to_string_pretty(&core.snapshot()?.catalogue)?
            ),
            CatalogueCmd::Match { edition_id } => println!(
                "{}",
                serde_json::to_string_pretty(&core.match_catalogue(&edition_id)?)?
            ),
            CatalogueCmd::Link {
                edition_id,
                record_id,
                media_id,
            } => core.link_catalogue(&edition_id, &record_id, &media_id)?,
            CatalogueCmd::Unlink { edition_id } => core.unlink_catalogue(&edition_id)?,
        },
        Command::Library(LibraryCmd::AddRoot { system, path }) => {
            // Validates against the shipped descriptors (ps2, ps1).
            let label = marquee_core::catalog::by_id(&system)?.display_name;
            let root = core.add_root(&system, path, Some(label))?;
            println!("{}", serde_json::to_string_pretty(&root)?);
        }
        Command::Intake {
            paths,
            delete,
            no_scan,
        } => {
            let mut archives = Vec::new();
            for p in paths {
                if p.is_dir() {
                    for e in std::fs::read_dir(&p)?.flatten() {
                        let f = e.path();
                        let ext = f.extension().map(|x| x.to_string_lossy().to_lowercase());
                        if f.is_file() && matches!(ext.as_deref(), Some("zip" | "7z" | "rar")) {
                            archives.push(f);
                        }
                    }
                } else {
                    archives.push(p);
                }
            }
            archives.sort();
            let mut changed = false;
            let mut failed = 0usize;
            for a in &archives {
                match core.intake_archive(a, delete) {
                    Ok(r) => {
                        changed |= matches!(
                            r.outcome,
                            marquee_core::intake::Outcome::Imported
                                | marquee_core::intake::Outcome::KeptArchive
                                | marquee_core::intake::Outcome::AlreadyPresent
                        );
                        println!(
                            "{}: {:?}{} - {}",
                            a.display(),
                            r.outcome,
                            r.system.map(|s| format!(" ({s})")).unwrap_or_default(),
                            r.message
                        );
                    }
                    Err(e) => {
                        failed += 1;
                        println!("{}: error {} - {}", a.display(), e.code, e.message);
                    }
                }
            }
            if changed && !no_scan {
                let report = core.scan(None, None)?;
                println!("{}", serde_json::to_string_pretty(&report)?);
            }
            if failed > 0 {
                return Err(AppError::new(
                    "INTAKE_FAILED",
                    format!(
                        "{failed} of {} archive imports failed; successful imports were kept",
                        archives.len()
                    ),
                ));
            }
        }
        Command::Torrent(_) => unreachable!("handled without Core"),
        Command::Prune => {
            println!("removed {}", core.prune_missing()?);
        }
        Command::Scan { root } => {
            let report = core.scan(root.as_deref(), None)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::List { system, json } => {
            let mut snap = core.snapshot()?;
            if let Some(system) = system {
                snap.editions.retain(|e| e.system == system);
            }
            if json {
                println!("{}", serde_json::to_string_pretty(&snap.editions)?);
            } else {
                for edition in snap.editions {
                    println!(
                        "{}\t{}\t{}\t{}",
                        edition.id,
                        edition.system,
                        if edition.missing { "missing" } else { "ok" },
                        edition.title
                    );
                }
            }
        }
        Command::Runners(RunnersCmd::List { json }) => {
            let runners = core.refresh_runners()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&runners)?);
            } else {
                for runner in runners {
                    println!(
                        "{}\t{}\t{}\t{}",
                        runner.id,
                        runner.source,
                        runner.bios_state.as_str(),
                        runner.display_name
                    );
                }
            }
        }
        Command::Runners(RunnersCmd::Select { runner_id }) => {
            core.select_runner(&runner_id)?;
            println!("selected {runner_id}");
        }
        Command::Runners(RunnersCmd::Setup { runner_id }) => {
            core.open_setup(&runner_id)?;
            println!("opened setup for {runner_id}");
        }
        Command::Runners(RunnersCmd::Install { channel }) => {
            let sink = Arc::new(FnSink(|event: CoreEvent| match event {
                CoreEvent::ToolInstallProgress { message, .. } => println!("{message}"),
                CoreEvent::ToolInstallFinished { runner_id, .. } => {
                    println!("installed {}", runner_id.unwrap_or_else(|| "pcsx2".into()));
                }
                CoreEvent::ToolInstallFailed { message, .. } => {
                    eprintln!("{message}");
                }
                _ => {}
            }));
            let runners = core.install_pcsx2(&channel, sink)?;
            println!("{}", serde_json::to_string_pretty(&runners)?);
        }
        Command::Runners(RunnersCmd::Uninstall) => {
            core.uninstall_pcsx2()?;
            println!("PCSX2 removed. Settings, saves and BIOS were kept.");
        }
        Command::Emulators(cmd) => {
            let list = match cmd {
                EmulatorsCmd::Profile { id } => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&core.emulator_profile(&id)?)?
                    );
                    return Ok(());
                }
                EmulatorsCmd::ProfileSet {
                    id,
                    mode,
                    renderer,
                    scale,
                    vsync,
                } => {
                    let mode = match mode.as_str() {
                        "custom" => marquee_core::profiles::Mode::Custom,
                        "disabled" => marquee_core::profiles::Mode::Disabled,
                        _ => marquee_core::profiles::Mode::Recommended,
                    };
                    let config = marquee_core::profiles::Config {
                        mode,
                        custom: marquee_core::profiles::Options {
                            renderer,
                            resolution_scale: scale,
                            vsync,
                        },
                    };
                    println!(
                        "{}",
                        serde_json::to_string_pretty(
                            &core.configure_emulator_profile(&id, config)?
                        )?
                    );
                    return Ok(());
                }
                EmulatorsCmd::List => core.list_emulators()?,
                EmulatorsCmd::Check => core.check_emulator_updates()?,
                EmulatorsCmd::BuildStatus { id } => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(
                            &core.source_builds.status(&core.paths, &id)?
                        )?
                    );
                    return Ok(());
                }
                EmulatorsCmd::Rollback { id } => core.rollback_emulator(&id)?,
                EmulatorsCmd::Build { id } => {
                    core.build_emulator(&id)?;
                    let mut last_phase = String::new();
                    loop {
                        if let Some(status) = core.source_builds.status(&core.paths, &id)? {
                            if status.phase != last_phase {
                                println!("{}", status.message);
                                last_phase = status.phase.clone();
                            }
                            if status.state != "running" {
                                println!("Log: {}", status.log_path.display());
                                if status.state != "succeeded" {
                                    return Err(AppError::new("BUILD_FAILED", status.message));
                                }
                                break;
                            }
                        }
                        thread::sleep(Duration::from_millis(500));
                    }
                    core.list_emulators()?
                }
                EmulatorsCmd::Install { id } => {
                    let sink = Arc::new(FnSink(|event: CoreEvent| match event {
                        CoreEvent::ToolInstallProgress { message, .. } => println!("{message}"),
                        CoreEvent::ToolInstallFailed { message, .. } => eprintln!("{message}"),
                        _ => {}
                    }));
                    core.install_emulator(&id, sink)?
                }
                EmulatorsCmd::InstallNeeded => {
                    let sink = Arc::new(FnSink(|event: CoreEvent| match event {
                        CoreEvent::ToolInstallProgress { message, .. } => println!("{message}"),
                        CoreEvent::ToolInstallFailed { message, .. } => eprintln!("{message}"),
                        _ => {}
                    }));
                    let done = core.install_needed(sink)?;
                    println!(
                        "{}",
                        if done.is_empty() {
                            "nothing to install: every emulator your library needs is already there"
                                .to_string()
                        } else {
                            format!("installed: {}", done.join(", "))
                        }
                    );
                    return Ok(());
                }
                EmulatorsCmd::Open { id } => {
                    core.open_emulator(&id)?;
                    println!("opened {id}");
                    return Ok(());
                }
                EmulatorsCmd::Uninstall { id, purge_data } => {
                    core.uninstall_emulator(&id, purge_data)?
                }
            };
            println!("{}", serde_json::to_string_pretty(&list)?);
        }
        Command::Bios(cmd) => match cmd {
            BiosCmd::List => {
                for b in core.list_bios()? {
                    println!("{}\t{}\tv{}\t{}", b.system, b.region, b.version, b.file);
                }
            }
            BiosCmd::Import { path } => {
                let report = core.import_bios(&path)?;
                for b in &report.added {
                    println!("added {} {} v{} {}", b.system, b.region, b.version, b.file);
                }
                println!("{} added, {} ignored", report.added.len(), report.ignored);
            }
            BiosCmd::Remove { system } => {
                println!("removed {} file(s)", core.remove_bios(&system)?);
            }
        },
        Command::Ports(cmd) => {
            let list = match cmd {
                PortsCmd::List => core.list_ports()?,
                PortsCmd::Check => core.check_port_updates()?,
                PortsCmd::Install { id } => {
                    let sink = Arc::new(FnSink(|event: CoreEvent| {
                        if let CoreEvent::PortProgress { message, .. } = event {
                            println!("{message}");
                        }
                    }));
                    core.install_port(&id, sink)?
                }
                PortsCmd::Launch { id } => {
                    core.launch_port(&id)?;
                    println!("started {id}");
                    return Ok(());
                }
            };
            println!("{}", serde_json::to_string_pretty(&list)?);
        }
        Command::Artwork(ArtworkCmd::Refresh { edition }) => {
            let report = core.refresh_artwork(edition.as_deref(), None)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::Run(RunCmd::Web { port, web_dir }) => {
            let assets = marquee_core::server::dir_assets(web_dir);
            let handle = marquee_core::server::start(Arc::clone(&core), Some(assets), port)?;
            println!("Marquee web UI: {}", handle.url());
            println!("Press Ctrl+C to stop.");
            loop {
                thread::sleep(Duration::from_secs(3600));
            }
        }
        Command::SteamInspect => {
            println!("{}", serde_json::to_string_pretty(&core.inspect_steam())?)
        }
        Command::Health => {
            println!("{}", core.health()?);
        }
        Command::Launch { edition_id } => {
            let session = core.launch(&edition_id, &[], Arc::new(NoopSink))?;
            println!("session {}", session.id);
            loop {
                thread::sleep(Duration::from_millis(100));
                let snap = core.snapshot()?;
                let current = snap.sessions.iter().find(|s| s.id == session.id);
                match current {
                    Some(s) if s.state == SessionState::Running => continue,
                    Some(s) => {
                        println!(
                            "{} duration={}s exit={:?}",
                            s.state.as_str(),
                            s.duration_seconds.unwrap_or(0),
                            s.exit_code
                        );
                        if s.state == SessionState::Failed {
                            return Err(AppError::new(
                                "SPAWN_FAILED",
                                s.error.clone().unwrap_or_else(|| "session failed".into()),
                            ));
                        }
                        break;
                    }
                    None => break,
                }
            }
        }
    }
    Ok(())
}

fn torrent(
    client: &marquee_manager::Client,
    command: TorrentCmd,
) -> marquee_core::error::Result<()> {
    use marquee_manager::torrent::{Request, TorrentSettings, TransferStatus};
    if !matches!(
        command,
        TorrentCmd::Status | TorrentCmd::List | TorrentCmd::Artifacts | TorrentCmd::Inspect { .. }
    ) {
        client.start_manager()?;
    }
    let settings = || {
        if marquee_manager::torrent::adopted(client.data_dir())? {
            marquee_manager::torrent::read_settings(client.data_dir())
        } else {
            client.torrent::<TorrentSettings>(Request::Settings)
        }
    };
    let status = || client.torrent::<TransferStatus>(Request::Status);
    let save = |settings| client.torrent::<()>(Request::SetSettings { settings });
    let pause = |hash: &str, paused| {
        client.torrent::<()>(Request::Pause {
            hash: hash.into(),
            paused,
        })
    };
    match command {
        TorrentCmd::Artifacts => println!(
            "{}",
            serde_json::to_string_pretty(&client.torrent_downloads()?)?
        ),
        TorrentCmd::Prepare {
            info_hash,
            path,
            system,
        } => println!(
            "{}",
            serde_json::to_string_pretty(&client.inspect_torrent(&info_hash, &path, &system)?)?
        ),
        TorrentCmd::Inspect { path } => println!(
            "{}",
            serde_json::to_string_pretty(&marquee_core::transfer::inspect_file(&path)?.0)?
        ),
        TorrentCmd::List => println!("{}", serde_json::to_string_pretty(&status()?)?),
        TorrentCmd::Discovery { trackers, dht } => {
            let mut settings = settings()?;
            settings.trackers = trackers;
            settings.dht = dht;
            save(settings)?;
        }
        TorrentCmd::Pause { info_hash } => pause(&info_hash, true)?,
        TorrentCmd::Resume { info_hash } => pause(&info_hash, false)?,
        TorrentCmd::Stop { info_hash } => {
            client.torrent::<()>(Request::Stop { hash: info_hash })?
        }
        TorrentCmd::Status => println!("{}", serde_json::to_string_pretty(&settings()?)?),
        TorrentCmd::Enable | TorrentCmd::Disable => {
            let mut t = settings()?;
            t.enabled = matches!(command, TorrentCmd::Enable);
            save(t.clone())?;
            println!("library transfer {}", if t.enabled { "on" } else { "off" });
        }
        TorrentCmd::Seeding { on } => {
            let mut t = settings()?;
            t.seeding = on;
            save(t.clone())?;
        }
        TorrentCmd::Proxy { url, off: _ } => {
            let mut t = settings()?;
            t.proxy = url;
            save(t.clone())?;
            match &t.proxy {
                Some(proxy) => {
                    println!("public torrents use {proxy}; nothing else leaves the machine")
                }
                None => println!("public torrents connect directly"),
            }
        }
        TorrentCmd::Limits { upload, download } => {
            let mut t = settings()?;
            t.upload_kbps = upload;
            t.download_kbps = download;
            save(t.clone())?;
        }
        TorrentCmd::Create { path } => {
            let path = std::fs::canonicalize(path)?;
            let created: marquee_manager::torrent::Created = client.torrent(Request::Create {
                path,
                listen: None,
                seed: false,
            })?;
            println!(
                "{}\n{}\n{}",
                created.path.display(),
                created.info_hash,
                created.magnet
            );
        }
        TorrentCmd::Seed { path, listen } => {
            let path = std::fs::canonicalize(path)?;
            let created: marquee_manager::torrent::Created = client.torrent(Request::Create {
                path,
                listen: Some(listen),
                seed: true,
            })?;
            println!("torrent file: {}\nmagnet: {}\nSeeding in marquee-manager; use torrent stop {} to stop.", created.path.display(),created.magnet,created.info_hash);
        }
        TorrentCmd::Get {
            source,
            into,
            peers,
            timeout_secs,
            review_token,
        } => {
            std::fs::create_dir_all(&into)?;
            let destination = std::fs::canonicalize(&into)?;
            let (hash, result) = if source.starts_with("magnet:") {
                let mut link = source.clone();
                for peer in &peers {
                    link.push_str(&format!("&x.pe={peer}"));
                }
                // Queue first, then get its identity from the durable list.
                let result = client.torrent::<()>(Request::Get {
                    link,
                    destination: Some(destination.clone()),
                });
                let hash = marquee_core::transfer::magnet_hash(&source)?;
                (hash, result)
            } else {
                let path = std::path::Path::new(&source);
                let path = std::fs::canonicalize(path)?;
                let (manifest, token) = marquee_core::transfer::inspect_file(&path)?;
                let result = client.torrent::<()>(Request::AddFile {
                    path,
                    review_token: review_token.unwrap_or(token),
                    peers,
                    destination: Some(destination.clone()),
                });
                (manifest.info_hash, result)
            };
            if let Err(error) = result {
                if error.code != "TORRENT_EXISTS" {
                    return Err(error);
                }
                let state = status()?;
                if !state
                    .items
                    .iter()
                    .any(|e| e.info_hash == hash && e.directory == destination.join(&hash))
                {
                    return Err(AppError::new("TORRENT_CONFLICT", "This torrent already has a different destination; use torrent list and resume"));
                }
                pause(&hash, false)?;
            }
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
            loop {
                let status = status()?;
                if let Some(item) = status.items.iter().find(|e| e.info_hash == hash) {
                    if matches!(item.state.as_str(), "complete" | "sharing" | "seeding") {
                        println!(
                            "Downloaded: {} ({} bytes) in {}. Installation is separate.",
                            item.name,
                            item.total,
                            item.directory.display()
                        );
                        break;
                    }
                    if item.state == "error" || item.state == "source_missing" {
                        return Err(AppError::new(
                            "TORRENT",
                            item.error
                                .clone()
                                .unwrap_or_else(|| "Source files are missing".into()),
                        ));
                    }
                }
                if std::time::Instant::now() >= deadline {
                    pause(&hash, true)?;
                    return Err(AppError::new(
                        "TORRENT_TIMEOUT",
                        "Download paused after timeout; progress and transfer record were kept",
                    ));
                }
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
        }
    }
    Ok(())
}
