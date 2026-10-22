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
    Limits { upload: u32, download: u32 },
    /// Make a .torrent for a game file or folder (saved under the data dir)
    Create { path: PathBuf },
    /// Seed a file or folder until interrupted; prints the address and magnet
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

fn run() -> marquee_core::error::Result<()> {
    let args = Args::parse();
    let core = Arc::new(Core::open(args.data_dir, args.allow_fake)?);
    match args.command {
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
        Command::Torrent(command) => torrent(&core, command)?,
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

fn torrent_config(
    core: &Core,
    dir: &std::path::Path,
    listen: std::net::SocketAddr,
) -> marquee_core::error::Result<marquee_torrent::Config> {
    let t = core.torrent_settings()?;
    if !t.enabled {
        return Err(AppError::new(
            "TORRENT_OFF",
            "Library transfer is off. Turn it on with `marquee torrent enable`.",
        ));
    }
    let mut cfg = marquee_torrent::Config::private(dir);
    cfg.listen = listen;
    cfg.seeding = t.seeding;
    cfg.upload_bps = (t.upload_kbps > 0).then(|| t.upload_kbps.saturating_mul(1024));
    cfg.download_bps = (t.download_kbps > 0).then(|| t.download_kbps.saturating_mul(1024));
    Ok(cfg)
}

fn torrent(core: &Core, command: TorrentCmd) -> marquee_core::error::Result<()> {
    let fail = |e: anyhow::Error| AppError::new("TORRENT", format!("{e:#}"));
    let store = core.paths.data_dir.join("torrents");
    match command {
        TorrentCmd::Status => println!(
            "{}",
            serde_json::to_string_pretty(&core.torrent_settings()?)?
        ),
        TorrentCmd::Enable | TorrentCmd::Disable => {
            let mut t = core.torrent_settings()?;
            t.enabled = matches!(command, TorrentCmd::Enable);
            core.set_torrent_settings(&t)?;
            println!("library transfer {}", if t.enabled { "on" } else { "off" });
        }
        TorrentCmd::Seeding { on } => {
            let mut t = core.torrent_settings()?;
            t.seeding = on;
            core.set_torrent_settings(&t)?;
        }
        TorrentCmd::Limits { upload, download } => {
            let mut t = core.torrent_settings()?;
            t.upload_kbps = upload;
            t.download_kbps = download;
            core.set_torrent_settings(&t)?;
        }
        TorrentCmd::Create { path } => {
            let cfg = torrent_config(core, &store, "127.0.0.1:0".parse().unwrap())?;
            let engine = marquee_torrent::Engine::start(&cfg).map_err(fail)?;
            let made = engine.create(&path, vec![]).map_err(fail)?;
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let file = store.join(format!("{name}.torrent"));
            std::fs::write(&file, &made.bytes)?;
            println!("{}\n{}\n{}", file.display(), made.info_hash, made.magnet);
            engine.shutdown();
        }
        TorrentCmd::Seed { path, listen } => {
            let cfg = torrent_config(core, &store, listen)?;
            if !cfg.seeding {
                return Err(AppError::new(
                    "TORRENT_SEEDING_OFF",
                    "Seeding is off. Run `marquee torrent seeding true`.",
                ));
            }
            let engine = marquee_torrent::Engine::start(&cfg).map_err(fail)?;
            let made = engine.create(&path, vec![]).map_err(fail)?;
            let root = path.parent().unwrap_or(std::path::Path::new("."));
            let id = engine.seed(made.bytes.clone(), root).map_err(fail)?;
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            std::fs::create_dir_all(&store)?;
            let file = store.join(format!("{name}.torrent"));
            std::fs::write(&file, &made.bytes)?;
            println!("torrent file: {}", file.display());
            println!("magnet: {}", made.magnet);
            if let Some(a) = engine.listen_addr() {
                println!(
                    "listening on port {} (use --peer <this-host>:{})",
                    a.port(),
                    a.port()
                );
            }
            loop {
                std::thread::sleep(std::time::Duration::from_secs(5));
                if let Ok(p) = engine.progress(id) {
                    println!(
                        "{:?}: {}/{} bytes, uploaded {}",
                        p.state, p.downloaded, p.total, p.uploaded
                    );
                }
            }
        }
        TorrentCmd::Get {
            source,
            into,
            peers,
            timeout_secs,
        } => {
            let cfg = torrent_config(core, &into, "0.0.0.0:0".parse().unwrap())?;
            let engine = marquee_torrent::Engine::start(&cfg).map_err(fail)?;
            let src = if source.starts_with("magnet:") || source.starts_with("http") {
                marquee_torrent::Source::Link(source)
            } else {
                marquee_torrent::Source::TorrentBytes(std::fs::read(&source)?)
            };
            let id = engine.download(src, &into, peers).map_err(fail)?;
            engine
                .wait_finished(id, std::time::Duration::from_secs(timeout_secs))
                .map_err(fail)?;
            let p = engine.progress(id).map_err(fail)?;
            println!("done: {} ({} bytes) in {}", p.name, p.total, into.display());
            engine.shutdown();
        }
    }
    Ok(())
}
