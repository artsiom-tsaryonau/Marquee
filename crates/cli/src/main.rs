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
    Library(LibraryCmd),
    Scan {
        #[arg(long)]
        root: Option<String>,
    },
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
    Launch {
        edition_id: String,
    },
    /// Serve the UI and API on 127.0.0.1 so a browser shows this same library
    Serve {
        #[arg(long)]
        port: Option<u16>,
        /// Built UI directory
        #[arg(long, default_value = "apps/desktop/dist")]
        web_dir: PathBuf,
    },
    Health,
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
        Command::Library(LibraryCmd::AddRoot { system, path }) => {
            if system != "ps2" {
                return Err(AppError::new(
                    "ROOT_OFFLINE",
                    "only --system ps2 is supported in this milestone",
                ));
            }
            let root = core.add_root(path, None)?;
            println!("{}", serde_json::to_string_pretty(&root)?);
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
        Command::Serve { port, web_dir } => {
            let assets = marquee_core::server::dir_assets(web_dir);
            let handle = marquee_core::server::start(Arc::clone(&core), Some(assets), port)?;
            println!("Marquee web UI: {}", handle.url());
            println!("Press Ctrl+C to stop.");
            loop {
                thread::sleep(Duration::from_secs(3600));
            }
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
