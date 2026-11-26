//! Library-owned private-display launches. No arbitrary command manifests or desktop capture.
use crate::db::now_rfc3339;
use crate::error::{AppError, Result};
use crate::model::{CoreEvent, Session, SessionState};
use crate::sessions::EventSink;
use crate::source_builds::{run, valid_image};
use crate::{runners, Core};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const APP: &str = "Marquee library game";
const XENIA_REVISION: &str = "451f915566d57436555c6eb4ee77bb4717eb01de";
const XENIA_SHA256: &str = "b96b2723b0af55b5c4d837916e12035f172fe9ebbab2e90ba6c64f2f21fd5d8b";
const FILES: &[(&str, &[u8])] = &[
    (
        "Containerfile",
        include_bytes!("../../../../catalog/streaming/library/Containerfile"),
    ),
    (
        "library-entrypoint.py",
        include_bytes!("../../../../catalog/streaming/library/library-entrypoint.py"),
    ),
    (
        "library-game.py",
        include_bytes!("../../../../catalog/streaming/library/library-game.py"),
    ),
    (
        "game.py",
        include_bytes!("../../../../catalog/streaming/sonic/game.py"),
    ),
];
#[derive(Default)]
pub struct State {
    pub busy: bool,
    pub error: Option<AppError>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Plan {
    schema_version: u8,
    launch_id: String,
    #[serde(default)]
    process_lease: bool,
    edition_id: String,
    runner_id: String,
    adapter: String,
    title: String,
    source: PathBuf,
    size: u64,
    mtime: i64,
    serial: Option<String>,
}
impl Plan {
    fn same_input(&self, other: &Self) -> bool {
        self.schema_version == other.schema_version
            && self.edition_id == other.edition_id
            && self.runner_id == other.runner_id
            && self.adapter == other.adapter
            && self.source == other.source
            && self.size == other.size
            && self.mtime == other.mtime
            && self.serial == other.serial
    }
}
#[derive(Serialize, Deserialize)]
struct Owner {
    library: String,
    edition_id: String,
    runner_id: String,
    plan: String,
}
fn err(message: &str) -> AppError {
    AppError::new("HEADLESS", message)
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn key(core: &Core) -> String {
    digest(core.paths.data_dir.to_string_lossy().as_bytes())
}
fn root(data: &Path) -> PathBuf {
    data.join("streaming/library")
}
fn name(data: &Path) -> String {
    format!(
        "marquee-library-{}",
        &digest(data.to_string_lossy().as_bytes())[..12]
    )
}
fn read_json(path: &Path) -> Result<Value> {
    let mut bytes = vec![];
    fs::File::open(path)?.take(65537).read_to_end(&mut bytes)?;
    if bytes.len() > 65536 {
        return Err(err("Streaming receipt exceeds its size limit"));
    }
    Ok(serde_json::from_slice(&bytes)?)
}
fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    fs::create_dir_all(path.parent().unwrap())?;
    let mut temp = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
    temp.write_all(&serde_json::to_vec_pretty(value)?)?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| AppError::from(e.error))?;
    Ok(())
}
fn output(command: &mut Command) -> Result<String> {
    let file = tempfile::NamedTempFile::new()?;
    run(
        command,
        file.path(),
        &AtomicBool::new(false),
        Duration::from_secs(15),
    )?;
    let bytes = fs::read(file.path())?;
    if bytes.len() > 65536 {
        return Err(err("Streaming command returned excessive output"));
    }
    String::from_utf8(bytes).map_err(|_| err("Invalid streaming command output"))
}
fn container(data: &Path) -> Result<Option<Value>> {
    let out = output(Command::new("podman").args([
        "container",
        "inspect",
        "--format",
        "{{json .}}",
        &name(data),
    ]));
    match out {
        Ok(out) => Ok(Some(serde_json::from_str(&out)?)),
        Err(_) => {
            // A failed inspect must not disguise an unavailable engine as a stopped host.
            output(Command::new("podman").args([
                "info",
                "--format",
                "{{.Host.Security.Rootless}} ",
            ]))?;
            Ok(None)
        }
    }
}
fn owner(data: &Path) -> Result<Option<Owner>> {
    let path = root(data).join("owner.json");
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(serde_json::from_value(read_json(&path)?)?))
}
fn owned(core: &Core, data: &Path, container: &Value) -> Result<Owner> {
    let owner =
        owner(data)?.ok_or_else(|| err("This streaming container has no ownership receipt"))?;
    let labels = &container["Config"]["Labels"];
    if owner.library != key(core)
        || labels["io.marquee.integration"] != "library-stream-v1"
        || labels["io.marquee.headless.owner"] != owner.library
        || labels["io.marquee.headless.plan"] != owner.plan
    {
        return Err(err(
            "This streaming host belongs to another library or runtime",
        ));
    }
    Ok(owner)
}

fn watching() -> &'static Mutex<HashSet<String>> {
    static WATCHING: std::sync::OnceLock<Mutex<HashSet<String>>> = std::sync::OnceLock::new();
    WATCHING.get_or_init(|| Mutex::new(HashSet::new()))
}

fn process_lease(data: &Path, library: &str, plan: &Plan) -> PathBuf {
    let profile = root(data)
        .join("profiles")
        .join(library)
        .join(&plan.adapter)
        .join(&plan.edition_id);
    if plan.process_lease {
        profile.join(format!("playing-{}.lock", plan.launch_id))
    } else {
        profile.join("writer.lock")
    }
}

fn playing(lease: &Path) -> Result<bool> {
    let file = match fs::File::open(lease) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    match file.try_lock_shared() {
        Ok(()) => Ok(false),
        Err(fs::TryLockError::WouldBlock) => Ok(true),
        Err(fs::TryLockError::Error(error)) => Err(error.into()),
    }
}

fn finish_stream_row(
    db: &Arc<Mutex<crate::db::Db>>,
    sink: Arc<dyn crate::sessions::EventSink>,
    row: crate::db::SessionRow,
    error: Option<&str>,
) -> Result<()> {
    let guard = db.lock().unwrap_or_else(|e| e.into_inner());
    let Some(row) = guard
        .session_by_id(&row.id)?
        .filter(|current| current.state == "running")
    else {
        return Ok(());
    };
    let elapsed = row.duration_seconds.unwrap_or(0);
    let (state, error, credit) = if elapsed < crate::sessions::EARLY_EXIT_SECS {
        (
            "failed",
            error.or(Some("Streamed session ended before it stayed open")),
            false,
        )
    } else {
        ("ended", error, true)
    };
    guard.complete_session(&row.id, elapsed, None, state, error, credit)?;
    if let Ok(Some(row)) = guard.session_by_id(&row.id) {
        if let Ok(session) = crate::sessions::row_to_session(row) {
            if session.state == SessionState::Failed {
                sink.emit(CoreEvent::SessionFailed { session });
            } else {
                sink.emit(CoreEvent::SessionEnded { session });
            }
        }
    }
    Ok(())
}

fn spawn_stream_watch(
    db: Arc<Mutex<crate::db::Db>>,
    sink: Arc<dyn crate::sessions::EventSink>,
    lease: PathBuf,
    session_id: String,
    started_unix: i64,
) {
    {
        let mut watching = watching().lock().unwrap_or_else(|e| e.into_inner());
        if !watching.insert(session_id.clone()) {
            return;
        }
    }
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(250));
            let guard = db.lock().unwrap_or_else(|e| e.into_inner());
            let row = match guard.session_by_id(&session_id) {
                Ok(Some(row)) if row.state == "running" => row,
                Ok(_) => break,
                Err(_) => continue,
            };
            match playing(&lease) {
                Ok(true) => {
                    let seconds =
                        (crate::unix_now() - started_unix).max(row.duration_seconds.unwrap_or(0));
                    if Some(seconds) != row.duration_seconds {
                        let _ = guard.checkpoint_session(&session_id, seconds);
                    }
                }
                Ok(false) => {
                    drop(guard);
                    if finish_stream_row(&db, Arc::clone(&sink), row, None).is_ok() {
                        break;
                    }
                }
                Err(_) => continue,
            }
        }
        watching()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&session_id);
    });
}

fn digest_image(receipt: &Value) -> Result<String> {
    let image = receipt["image"].as_str().unwrap_or_default();
    if !valid_image(image) {
        return Err(err(
            "The private-display base image receipt is incompatible",
        ));
    }
    Ok(image.into())
}

fn library_base(data: &Path) -> Result<Option<String>> {
    let path = root(data).join("base.json");
    if !path.exists() {
        return Ok(None);
    }
    let receipt = read_json(&path)?;
    if receipt["schema_version"] != 1 {
        return Err(err("The library streaming base receipt is incompatible"));
    }
    Ok(Some(digest_image(&receipt)?))
}

fn sonic_pilot_image(data: &Path) -> Result<Option<String>> {
    let path = data.join("streaming/sonic-pilot/current.json");
    if !path.exists() {
        return Ok(None);
    }
    let receipt = read_json(&path)?;
    if receipt["schema_version"] != 1 {
        return Ok(None);
    }
    Ok(digest_image(&receipt).ok())
}

fn wolf_image(data: &Path) -> Result<Option<String>> {
    Ok(super::runtime::status(data)?
        .map(|receipt| receipt.image)
        .filter(|image| valid_image(image)))
}

fn xenia_pilot_image(data: &Path) -> Result<String> {
    let receipt = read_json(&data.join("streaming/xenia-pilot/current.json")).map_err(|_| {
        err("Build the reviewed private-display runtime first; see Streaming setup")
    })?;
    if receipt["xenia_revision"] != XENIA_REVISION || receipt["xenia_sha256"] != XENIA_SHA256 {
        return Err(err(
            "The private-display base image receipt is incompatible",
        ));
    }
    digest_image(&receipt)
}

#[derive(Debug, PartialEq)]
enum Base {
    Ready(String),
    ComposeBlastem,
}

fn adapter_base(data: &Path, adapter: &str, program: &Path) -> Result<Base> {
    resolve_base(data, adapter, program, |image| {
        output(Command::new("podman").args(["image", "exists", image])).is_ok()
    })
}

fn resolve_base(
    data: &Path,
    adapter: &str,
    program: &Path,
    installed: impl Fn(&str) -> bool,
) -> Result<Base> {
    if adapter == "xenia" {
        let image = xenia_pilot_image(data)?;
        if installed(&image) {
            return Ok(Base::Ready(image));
        }
        return Err(err(
            "Rebuild the Xenia pilot image in the selected streaming folder",
        ));
    }
    if let Some(image) = sonic_pilot_image(data)?.filter(|image| installed(image)) {
        return Ok(Base::Ready(image));
    }
    if wolf_image(data)?.is_some_and(|image| installed(&image))
        && super::game_pilot::emulator_image(program).is_ok_and(|image| installed(&image))
    {
        return Ok(Base::ComposeBlastem);
    }
    if let Ok(image) = xenia_pilot_image(data) {
        if installed(&image) {
            return Ok(Base::Ready(image));
        }
    }
    // Older installs may retain only the composed library image. Early receipts
    // could point to a plain Wolf image, so check its emulator before reuse.
    if let Some(image) = library_base(data)?.filter(|image| installed(image)) {
        output(Command::new("podman").args([
            "run",
            "--rm",
            "--pull=never",
            "--network=none",
            "--entrypoint",
            "/opt/blastem/blastem",
            &image,
            "-v",
        ]))?;
        return Ok(Base::Ready(image));
    }
    Err(err(
        "Build the managed Wolf host and the reviewed BlastEm emulator before streaming Genesis",
    ))
}

fn build(data: &Path, adapter: &str, program: &Path) -> Result<String> {
    let base = match adapter_base(data, adapter, program)? {
        Base::Ready(image) => image,
        Base::ComposeBlastem => digest_image(&super::game_pilot::build(
            data,
            program,
            &AtomicBool::new(false),
        )?)?,
    };
    let mut hash = Sha256::new();
    hash.update(base.as_bytes());
    for (name, bytes) in FILES {
        hash.update(name);
        hash.update([0]);
        hash.update(bytes);
    }
    let hash = format!("{:x}", hash.finalize());
    let receipt_path = root(data).join(format!("image-{adapter}.json"));
    if let Ok(receipt) = read_json(&receipt_path) {
        if receipt["recipe"] == hash {
            if let Some(image) = receipt["image"].as_str().filter(|v| valid_image(v)) {
                if output(Command::new("podman").args(["image", "exists", image])).is_ok() {
                    return Ok(image.into());
                }
            }
        }
    }
    fs::create_dir_all(root(data))?;
    let context = tempfile::Builder::new()
        .prefix(".build-")
        .tempdir_in(root(data))?;
    for (name, bytes) in FILES {
        fs::write(context.path().join(name), bytes)?;
    }
    let image_file = context.path().join("image-id");
    run(
        Command::new("podman")
            .args([
                "build",
                "--pull=never",
                "--force-rm",
                "--layers",
                "--iidfile",
            ])
            .arg(&image_file)
            .arg("--build-arg")
            .arg(format!("BASE_IMAGE={base}"))
            .arg(context.path()),
        &root(data).join("build.log"),
        &AtomicBool::new(false),
        Duration::from_secs(300),
    )?;
    let image = fs::read_to_string(image_file)?.trim().to_string();
    if !valid_image(&image) {
        return Err(err("Invalid private-display image identity"));
    }
    write_json(
        &receipt_path,
        &json!({"image":image,"recipe":hash,"base":base}),
    )?;
    Ok(image)
}
fn render_device() -> Result<(PathBuf, bool)> {
    let mut nodes = vec![];
    for entry in fs::read_dir("/sys/class/drm")
        .map_err(|_| err("No render devices found"))?
        .flatten()
    {
        let node = entry.file_name().to_string_lossy().to_string();
        if !node.starts_with("renderD") {
            continue;
        }
        let nvidia = fs::read_link(entry.path().join("device/driver"))
            .is_ok_and(|p| p.file_name().is_some_and(|v| v == "nvidia"));
        let path = Path::new("/dev/dri").join(node);
        if path.exists() {
            nodes.push((path, nvidia));
        }
    }
    nodes.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    nodes
        .into_iter()
        .next()
        .ok_or_else(|| err("No accessible render device found"))
}
impl Core {
    fn headless_plan(&self, id: &str) -> Result<Plan> {
        self.with_db(|db| {
            let edition = db
                .edition_by_id(id)?
                .ok_or_else(|| err("Game is not in the library"))?;
            let media = crate::library::find_media_for_edition(db, id)?
                .ok_or_else(|| err("Game files are missing"))?;
            if !media.available || media.skip_reason.is_some() {
                return Err(err("Review the game files before streaming"));
            }
            let list = runners::snapshot(db)?;
            let legacy = db.get_setting("selected_runner_id")?;
            let selected = db
                .get_setting(&format!("selected_runner.{}", edition.system))?
                .or_else(|| {
                    runners::selection_for(&list, legacy.as_deref(), &edition.system)
                        .filter(|id| list.iter().any(|r| r.id == *id && r.available))
                        .map(str::to_owned)
                });
            let profile = db.profile_runner(id)?;
            let runner = runners::resolve(
                &list,
                &edition.system,
                profile.as_deref(),
                selected.as_deref(),
            )?;
            let adapter = match (edition.system.as_str(), runner.id.as_str()) {
                ("genesis", "runner.blastem.marquee") => "blastem",
                ("xbox360", "runner.xenia.marquee") => "xenia",
                _ => {
                    return Err(err(
                        "The selected emulator has no private-display adapter yet",
                    ))
                }
            };
            if !crate::emulators::config(db, adapter)?.args.is_empty() {
                return Err(err(
                    "Custom emulator arguments need a separate reviewed streaming profile",
                ));
            }
            let metadata = fs::symlink_metadata(&media.path)?;
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                return Err(err("Choose a regular game file for headless streaming"));
            }
            if adapter == "blastem" && !(512..=16 * 1024 * 1024).contains(&metadata.len()) {
                return Err(err(
                    "The Genesis adapter accepts cartridges from 512 bytes to 16 MiB",
                ));
            }
            let mtime = metadata
                .modified()?
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| err("The game modification time is unsupported"))?
                .as_secs() as i64;
            if adapter == "xenia" {
                let identity = marquee_media::xbox360::inspect(&media.path)?;
                if media.serial.as_deref() != Some(identity.title_id.as_str()) {
                    return Err(err("Disc identity changed; rescan before streaming"));
                }
            }
            Ok(Plan {
                schema_version: 1,
                launch_id: uuid::Uuid::new_v4().to_string(),
                process_lease: true,
                edition_id: id.into(),
                runner_id: runner.id.clone(),
                adapter: adapter.into(),
                title: edition.display_title.unwrap_or(edition.scan_title),
                source: media.path,
                size: metadata.len(),
                mtime,
                serial: media.serial,
            })
        })
    }
    fn headless_connection(&self) -> Result<super::controls::Connection> {
        let config = self.streaming_connection()?;
        config.validate()?;
        if config.socket
            != config
                .data_dir
                .join("streaming/wolf/state/runtime/wolf.sock")
        {
            return Err(err("Automatic headless launch needs the owned Wolf socket in the selected streaming folder"));
        }
        Ok(config)
    }
    pub(crate) fn headless_active(&self) -> Result<bool> {
        if self.headless.lock().unwrap_or_else(|e| e.into_inner()).busy {
            return Ok(true);
        }
        let c = self.streaming_connection()?;
        if owner(&c.data_dir)?.is_none() {
            return Ok(false);
        }
        if let Some(container) = container(&c.data_dir)? {
            owned(self, &c.data_dir, &container)?;
            if container["State"]["Running"] == true {
                return Ok(super::wolf::WolfClient::new(&c.socket)?
                    .inspect()
                    .map_or(true, |s| !s.sessions.is_empty()));
            }
        }
        Ok(false)
    }
    pub fn headless_status(&self, id: Option<&str>) -> Result<Value> {
        let state = self.headless.lock().unwrap_or_else(|e| e.into_inner());
        let busy = state.busy;
        let error = state.error.clone();
        drop(state);
        let plan = id.map(|id| self.headless_plan(id));
        let supported = plan.as_ref().is_none_or(|p| p.is_ok());
        let c = self.headless_connection();
        let setup = c.as_ref().map_err(|e| e.clone()).and_then(|c| {
            let adapter = plan
                .as_ref()
                .and_then(|result| result.as_ref().ok())
                .map_or("blastem", |plan| plan.adapter.as_str());
            adapter_base(
                &c.data_dir,
                adapter,
                &crate::emulators::program_dir(&self.paths, "blastem"),
            )?;
            if super::receiver::status(&c.data_dir)?.is_none() {
                return Err(err("Install the streaming viewer in Streaming setup"));
            }
            if !c
                .data_dir
                .join("streaming/wolf/state/config.toml")
                .is_file()
            {
                return Err(err("Set up and pair the private Wolf host first"));
            }
            Ok(())
        });
        let mut host_running = false;
        let mut game_running = false;
        let mut active = None;
        let mut app_id = None;
        let mut host_error = None;
        if let Ok(c) = &c {
            if root(&c.data_dir).join("owner.json").exists() {
                match container(&c.data_dir).and_then(|info| {
                    info.map(|i| owned(self, &c.data_dir, &i).map(|o| (i, o)))
                        .transpose()
                }) {
                    Ok(Some((info, o))) if info["State"]["Running"] == true => {
                        host_running = true;
                        active = Some(o.edition_id);
                        match super::wolf::WolfClient::new(&c.socket)?.inspect() {
                            Ok(host) => {
                                game_running = !host.sessions.is_empty();
                                app_id =
                                    host.apps.into_iter().find(|a| a.title == APP).map(|a| a.id);
                            }
                            Err(e) => {
                                host_error = Some(e.message);
                                game_running = true;
                            }
                        }
                    }
                    Err(e) => host_error = Some(e.message),
                    _ => {}
                }
            }
        }
        let reason = plan
            .and_then(|p| p.err())
            .map(|e| e.message)
            .or_else(|| setup.as_ref().err().map(|e| e.message.clone()));
        Ok(
            json!({"supported":supported,"ready":supported && setup.is_ok(),"reason":reason,"busy":busy,"error":error,
            "host_running":host_running,"game_running":game_running,"active_edition_id":active,"app_id":app_id,"host_error":host_error}),
        )
    }
    pub fn headless_launch(self: &Arc<Self>, id: &str) -> Result<()> {
        self.storage_move_ready()?;
        let _gate = self.headless_gate.lock().unwrap_or_else(|e| e.into_inner());
        self.storage_move_ready()?;
        if self.with_db(|db| {
            Ok(db.conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM sessions WHERE state='running' AND mode='local')",
                [],
                |row| row.get::<_, bool>(0),
            )?)
        })? || self.with_db(|db| crate::port_sessions::has_running(&db.conn))?
        {
            return Err(err("Close the local game before starting a stream"));
        }
        let plan = self.headless_plan(id)?;
        self.headless_connection()?;
        let mut state = self.headless.lock().unwrap_or_else(|e| e.into_inner());
        if state.busy {
            return Err(err("A headless operation is already running"));
        }
        state.busy = true;
        state.error = None;
        let core = Arc::clone(self);
        if let Err(e) = std::thread::Builder::new()
            .name("marquee-headless".into())
            .spawn(move || {
                let result = core.start_headless(plan);
                let mut state = core.headless.lock().unwrap_or_else(|e| e.into_inner());
                state.busy = false;
                state.error = result.err();
            })
        {
            state.busy = false;
            return Err(e.into());
        }
        Ok(())
    }
    fn start_headless(&self, plan: Plan) -> Result<()> {
        let c = self.headless_connection()?;
        fs::create_dir_all(root(&c.data_dir))?;
        let _lease =
            crate::instance::InstanceLock::acquire(&root(&c.data_dir).join("operation.lock"))?;
        let info: Value = serde_json::from_str(&output(
            Command::new("podman").args(["info", "--format=json"]),
        )?)?;
        if info.pointer("/host/security/rootless") != Some(&json!(true)) {
            return Err(err("Headless launch requires rootless Podman"));
        }
        if super::receiver::status(&c.data_dir)?.is_none() {
            return Err(err("Install the reference viewer first"));
        }
        let viewer = super::viewer::status(&c.data_dir)?;
        if let Some(info) = container(&c.data_dir)? {
            let o = owned(self, &c.data_dir, &info)?;
            if info["State"]["Running"] == true {
                let host = super::wolf::WolfClient::new(&c.socket)?.inspect()?;
                if o.edition_id == plan.edition_id && o.runner_id == plan.runner_id {
                    let old: Plan = serde_json::from_value(read_json(
                        &root(&c.data_dir)
                            .join("launches")
                            .join(&o.plan)
                            .join("launch.json"),
                    )?)?;
                    if plan.same_input(&old) {
                        let app = host
                            .apps
                            .iter()
                            .find(|a| a.title == APP)
                            .ok_or_else(|| err("Owned streaming app is missing"))?;
                        if viewer["running"] == true {
                            if viewer["app_id"] == app.id {
                                return self.track_stream_start(&c.data_dir, &old);
                            }
                            return Err(err(
                                "Close the other viewer before reconnecting this game",
                            ));
                        }
                        self.streaming_open_viewer(app.id.clone())?;
                        return self.track_stream_start(&c.data_dir, &old);
                    }
                }
                if viewer["running"] == true {
                    return Err(err(
                        "Close the current viewer before switching streamed games",
                    ));
                }
                if !host.sessions.is_empty() {
                    return Err(err(
                        "End the running streamed game before switching games or emulators",
                    ));
                }
                output(Command::new("podman").args(["stop", "--time", "10", &name(&c.data_dir)]))?;
                self.complete_stream_sessions(None)?;
            } else {
                self.complete_stream_sessions(Some("Streaming host is no longer running"))?;
                output(Command::new("podman").args(["rm", &name(&c.data_dir)]))?;
            }
        } else if super::wolf::WolfClient::new(&c.socket)?.inspect().is_ok() {
            return Err(err(
                "Another Wolf host is running; stop that host before using library headless mode",
            ));
        }
        if viewer["running"] == true {
            return Err(err(
                "Close the current viewer before starting a headless host",
            ));
        }
        let image = build(
            &c.data_dir,
            &plan.adapter,
            &crate::emulators::program_dir(&self.paths, "blastem"),
        )?;
        // Revalidate after image preparation, before any runtime publication.
        let fresh = self.headless_plan(&plan.edition_id)?;
        if fresh.runner_id != plan.runner_id
            || fresh.source != plan.source
            || fresh.size != plan.size
            || fresh.mtime != plan.mtime
            || fresh.serial != plan.serial
        {
            return Err(err(
                "The game or selected emulator changed while preparing streaming; try again",
            ));
        }
        let plan_id = uuid::Uuid::new_v4().to_string();
        let manifest = root(&c.data_dir)
            .join("launches")
            .join(&plan_id)
            .join("launch.json");
        write_json(&manifest, &plan)?;
        let profile = root(&c.data_dir)
            .join("profiles")
            .join(key(self))
            .join(&plan.adapter)
            .join(&plan.edition_id);
        fs::create_dir_all(&profile)?;
        let state = c.data_dir.join("streaming/wolf/state");
        if !state.join("config.toml").is_file() {
            return Err(err("Set up and pair the private Wolf host first"));
        }
        let (render, nvidia) = render_device()?;
        let mut command = Command::new("podman");
        command.args([
            "run",
            "--detach",
            "--rm",
            "--pull=never",
            "--name",
            &name(&c.data_dir),
            "--userns=keep-id",
            "--cap-drop=all",
            "--security-opt=no-new-privileges",
            "--security-opt=label=disable",
        ]);
        if nvidia {
            command.arg("--device=nvidia.com/gpu=all");
        }
        command.arg(format!("--device={}", render.display()));
        command
            .arg("--label")
            .arg(format!("io.marquee.headless.owner={}", key(self)));
        command
            .arg("--label")
            .arg(format!("io.marquee.headless.plan={plan_id}"));
        for (source, target, mode) in [
            (&state, "/state", "rw"),
            (&manifest, "/launch.json", "ro"),
            (&profile, "/profile", "rw"),
            (
                &plan.source,
                if plan.adapter == "xenia" {
                    "/games/game.iso"
                } else {
                    "/games/game.md"
                },
                "ro",
            ),
        ] {
            if source.to_string_lossy().contains([':', ',']) {
                return Err(err("Streaming mount paths cannot contain ':' or ','"));
            }
            command
                .arg("--volume")
                .arg(format!("{}:{target}:{mode}", source.display()));
        }
        for var in ["WOLF_RENDER_NODE", "WOLF_ENCODER_NODE"] {
            command
                .arg("--env")
                .arg(format!("{var}={}", render.display()));
        }
        command.args(["--env", "WOLF_USE_ZERO_COPY=FALSE"]);
        for (port, proto) in [
            (47984, "tcp"),
            (47989, "tcp"),
            (48010, "tcp"),
            (47999, "udp"),
            (48100, "udp"),
            (48200, "udp"),
        ] {
            command
                .arg("--publish")
                .arg(format!("127.0.0.1:{port}:{port}/{proto}"));
        }
        command.arg(image);
        write_json(
            &root(&c.data_dir).join("owner.json"),
            &Owner {
                library: key(self),
                edition_id: plan.edition_id.clone(),
                runner_id: plan.runner_id.clone(),
                plan: plan_id,
            },
        )?;
        output(&mut command)?;
        for _ in 0..60 {
            if let Ok(host) = super::wolf::WolfClient::new(&c.socket)?.inspect() {
                if let Some(app) = host.apps.into_iter().find(|a| a.title == APP) {
                    self.streaming_open_viewer(app.id)?;
                    return self.track_stream_start(&c.data_dir, &plan);
                }
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        Err(err(
            "The owned host did not become ready; inspect its container log before retrying",
        ))
    }
    pub fn headless_stop(&self) -> Result<()> {
        let _gate = self.headless_gate.lock().unwrap_or_else(|e| e.into_inner());
        if self.headless.lock().unwrap_or_else(|e| e.into_inner()).busy {
            return Err(err("Wait for headless setup to finish before stopping it"));
        }
        let c = self.headless_connection()?;
        if let Some(info) = container(&c.data_dir)? {
            owned(self, &c.data_dir, &info)?;
            if super::viewer::status(&c.data_dir)?["running"] == true {
                super::viewer::disconnect(&c.data_dir)?;
            }
            output(Command::new("podman").args(["stop", "--time", "10", &name(&c.data_dir)]))?;
        }
        self.complete_stream_sessions(None)
    }

    fn track_stream_start(&self, data: &Path, plan: &Plan) -> Result<()> {
        let lease = process_lease(data, &key(self), plan);
        for _ in 0..120 {
            if playing(&lease)? {
                return self.start_stream_session(&plan.edition_id, &plan.runner_id, lease);
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        Err(err("The emulator did not start within 30 seconds. Check the viewer and reconnect to retry."))
    }

    fn start_stream_session(
        &self,
        edition_id: &str,
        runner_id: &str,
        lease: PathBuf,
    ) -> Result<()> {
        let sessions = self.with_db(crate::sessions::list)?;
        if let Some(existing) = sessions.iter().find(|session| {
            session.state == SessionState::Running
                && session.mode == "stream"
                && session.edition_id == edition_id
                && session.runner_id == runner_id
        }) {
            spawn_stream_watch(
                self.db_arc(),
                self.hub(),
                lease,
                existing.id.clone(),
                crate::history::unix_of(&existing.started_at).unwrap_or(crate::unix_now()),
            );
            return Ok(());
        }
        self.complete_stream_sessions(None)?;
        let session_id = uuid::Uuid::new_v4().to_string();
        let started = now_rfc3339();
        self.with_db(|db| {
            db.insert_session_with_mode(
                &session_id,
                edition_id,
                runner_id,
                &started,
                "running",
                "stream",
            )
        })?;
        let session = Session {
            id: session_id.clone(),
            edition_id: edition_id.to_string(),
            runner_id: runner_id.to_string(),
            started_at: started.clone(),
            ended_at: None,
            duration_seconds: None,
            exit_code: None,
            state: SessionState::Running,
            error: None,
            port_id: None,
            mode: "stream".into(),
        };
        self.hub().emit(CoreEvent::SessionStarted { session });
        spawn_stream_watch(
            self.db_arc(),
            self.hub(),
            lease,
            session_id,
            crate::history::unix_of(&started).unwrap_or(crate::unix_now()),
        );
        Ok(())
    }

    fn complete_stream_sessions(&self, error: Option<&str>) -> Result<()> {
        let rows = self.with_db(|db| db.list_sessions())?;
        for row in rows
            .into_iter()
            .filter(|row| row.state == "running" && row.mode == "stream")
        {
            finish_stream_row(&self.db_arc(), self.hub(), row, error)?;
        }
        Ok(())
    }
}

// Receipts and profile locks live in the configured streaming directory.
fn recovered_lease(core: &Core, row: &crate::db::SessionRow) -> Result<Option<PathBuf>> {
    let data = core.streaming_connection()?.data_dir;
    let Some(owner) = owner(&data)?.filter(|owner| {
        owner.library == key(core)
            && owner.edition_id == row.edition_id
            && owner.runner_id == row.runner_id
    }) else {
        return Ok(None);
    };
    let manifest = root(&data)
        .join("launches")
        .join(owner.plan)
        .join("launch.json");
    let plan: Plan = serde_json::from_value(read_json(&manifest)?)?;
    if plan.edition_id != row.edition_id || plan.runner_id != row.runner_id {
        return Ok(None);
    }
    let lease = process_lease(&data, &owner.library, &plan);
    Ok(playing(&lease)?.then_some(lease))
}

pub fn recover_stream(core: &Core) -> Result<()> {
    let rows = core.with_db(|db| db.list_sessions())?;
    for row in rows
        .into_iter()
        .filter(|row| row.state == "running" && row.mode == "stream")
    {
        let active_lease = recovered_lease(core, &row);
        let reason = match &active_lease {
            Err(error) => format!(
                "Stream recovery could not verify the emulator: {}",
                error.message
            ),
            _ => "The streamed game ended while Marquee was closed".into(),
        };
        if let Ok(Some(lease)) = active_lease {
            spawn_stream_watch(
                core.db_arc(),
                core.hub(),
                lease,
                row.id.clone(),
                crate::history::unix_of(&row.started_at).unwrap_or(crate::unix_now()),
            );
        } else {
            finish_stream_row(&core.db_arc(), core.hub(), row, Some(&reason))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plans_follow_saved_runner_and_revalidate_identity_without_running_code() {
        let temp = tempfile::tempdir().unwrap();
        let core = Arc::new(Core::open_isolated(Some(temp.path().join("data")), false).unwrap());
        let games = temp.path().join("games");
        fs::create_dir(&games).unwrap();
        let iso = games.join("Game's $(name).iso");
        fs::write(&iso, marquee_media::xbox360::test_disc()).unwrap();
        core.add_root("xbox360", games, None).unwrap();
        core.scan(None, None).unwrap();
        core.with_db(|db| {
            db.upsert_runner(
                "runner.xenia.marquee",
                "emulator",
                "Xenia",
                &["/usr/bin/true".into()],
                crate::model::BiosState::Unverified,
                None,
                "marquee",
            )
        })
        .unwrap();
        let id = core.snapshot().unwrap().editions[0].id.clone();
        let plan = core.headless_plan(&id).unwrap();
        assert_eq!(plan.adapter, "xenia");
        assert_eq!(plan.source, iso);
        let mut renamed = plan.clone();
        renamed.title = "New display title".into();
        renamed.launch_id = "new-request".into();
        assert!(plan.same_input(&renamed));
        renamed.runner_id = "another-runner".into();
        assert!(!plan.same_input(&renamed));
        assert_eq!(plan.serial.as_deref(), Some("53450841"));
        core.with_db(|db| {
            db.set_setting(
                &crate::emulators::config_key("xenia"),
                r#"{"args":["--config=/outside"]}"#,
            )
        })
        .unwrap();
        assert!(core
            .headless_plan(&id)
            .unwrap_err()
            .message
            .contains("Custom emulator arguments"));
        core.with_db(|db| db.set_setting(&crate::emulators::config_key("xenia"), "{}"))
            .unwrap();
        fs::write(&iso, b"damaged").unwrap();
        assert!(core.headless_plan(&id).is_err());
        assert_eq!(
            crate::api::dispatch(&core, "headless_stop", json!({}))
                .unwrap_err()
                .code,
            "STREAMING_CONFIRM"
        );
    }
    #[test]
    fn ownership_never_adopts_an_unrelated_container() {
        let temp = tempfile::tempdir().unwrap();
        let core = Core::open_isolated(Some(temp.path().join("data")), false).unwrap();
        let data = temp.path().join("streaming");
        write_json(
            &root(&data).join("owner.json"),
            &Owner {
                library: key(&core),
                edition_id: "game".into(),
                runner_id: "runner".into(),
                plan: "plan".into(),
            },
        )
        .unwrap();
        let info = json!({"Config":{"Labels":{"io.marquee.integration":"library-stream-v1","io.marquee.headless.owner":key(&core),"io.marquee.headless.plan":"plan"}}});
        assert!(owned(&core, &data, &info).is_ok());
        let mut foreign = info.clone();
        foreign["Config"]["Labels"]["io.marquee.headless.owner"] = json!("another-library");
        assert!(owned(&core, &data, &foreign).is_err());
        let mut wrong = info;
        wrong["Config"]["Labels"]["io.marquee.headless.plan"] = json!("another-plan");
        assert!(owned(&core, &data, &wrong).is_err());
    }
    fn sha(digit: char) -> String {
        format!("sha256:{}", digit.to_string().repeat(64))
    }

    #[test]
    fn genesis_and_xenia_use_adapter_capable_bases() {
        let temp = tempfile::tempdir().unwrap();
        let data = temp.path();
        assert!(resolve_base(data, "blastem", data, |_| true).is_err());
        write_json(&data.join("streaming/wolf/current.json"), &json!({
            "schema_version":1,"image":sha('d'),"wolf_revision":"w","compositor_revision":"c",
            "recipe_sha256":"r","built_at":"","baseline_probe":null,"license_inventory":"","live_streaming_verified":false
        })).unwrap();
        assert!(resolve_base(data, "blastem", data, |_| true).is_err());
        fs::write(data.join("image-id"), sha('e')).unwrap();
        write_json(&data.join("source-build.json"), &json!({"image":sha('e'),
            "repository":"https://github.com/libretro/blastem","commit":"ab743653ed110eeb7b1fe64d00cc5f9279c2bfec","recipe":"blastem-standalone-sdl-v1"})).unwrap();
        assert_eq!(
            resolve_base(data, "blastem", data, |_| true).unwrap(),
            Base::ComposeBlastem
        );
        write_json(
            &data.join("streaming/sonic-pilot/current.json"),
            &json!({"schema_version":1,"image":sha('a')}),
        )
        .unwrap();
        assert_eq!(
            resolve_base(data, "blastem", data, |_| true).unwrap(),
            Base::Ready(sha('a'))
        );
        assert!(resolve_base(data, "xenia", data, |_| true).is_err());
        write_json(&data.join("streaming/xenia-pilot/current.json"), &json!({"image":sha('b'), "xenia_revision": XENIA_REVISION, "xenia_sha256": XENIA_SHA256})).unwrap();
        assert_eq!(
            resolve_base(data, "xenia", data, |_| true).unwrap(),
            Base::Ready(sha('b'))
        );
        assert!(resolve_base(data, "blastem", data, |_| false).is_err());
        assert!(resolve_base(data, "xenia", data, |_| false).is_err());
    }

    #[test]
    fn busy_headless_start_blocks_local_launch_without_contacting_a_host() {
        let temp = tempfile::tempdir().unwrap();
        let core = Core::open_isolated(Some(temp.path().join("data")), true).unwrap();
        core.headless.lock().unwrap().busy = true;
        assert_eq!(
            core.launch("missing", &[], core.hub()).unwrap_err().code,
            "LAUNCH_BUSY"
        );
        assert!(core.headless_stop().is_err());
    }

    #[test]
    fn streamed_session_lifecycle_credits_playtime() {
        let temp = tempfile::tempdir().unwrap();
        let core = Core::open_isolated(Some(temp.path().join("data")), true).unwrap();
        core.with_db(|db| {
            db.conn.execute(
                "INSERT INTO editions(id,system,scan_title,imported_at) VALUES('edition','ps2','Game','2026-01-01')",
                [],
            )?;
            db.insert_session_with_mode(
                "stream-1",
                "edition",
                "runner.fake",
                "2026-01-01T00:00:00Z",
                "running",
                "stream",
            )?;
            db.checkpoint_session("stream-1", 120)
        })
        .unwrap();
        recover_stream(&core).unwrap();
        let snap = core.snapshot().unwrap();
        let session = snap
            .sessions
            .iter()
            .find(|session| session.id == "stream-1")
            .unwrap();
        assert_eq!(session.mode, "stream");
        assert_eq!(session.state, SessionState::Ended);
        assert_eq!(
            snap.editions
                .iter()
                .find(|edition| edition.id == "edition")
                .unwrap()
                .playtime_seconds,
            120
        );
        assert_eq!(core.game_sessions("edition").unwrap().stream_seconds, 120);
    }

    #[test]
    fn refused_headless_launch_does_not_insert_a_stream_session() {
        let temp = tempfile::tempdir().unwrap();
        let core = Arc::new(Core::open_isolated(Some(temp.path().join("data")), true).unwrap());
        core.with_db(|db| {
            db.conn.execute(
                "INSERT INTO editions(id,system,scan_title,imported_at) VALUES('edition','ps2','Game','2026-01-01')",
                [],
            )?;
            db.insert_session(
                "session",
                "edition",
                "runner.fake",
                "2026-01-01T00:00:00Z",
                "running",
            )
        })
        .unwrap();
        let err = core.headless_launch("edition").unwrap_err();
        assert!(err.message.contains("Close the local game"));
        let sessions = core.with_db(crate::sessions::list).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].mode, "local");
    }

    #[test]
    fn recovery_and_reconnect_use_the_active_games_streaming_folder() {
        let temp = tempfile::tempdir().unwrap();
        let core = Arc::new(Core::open_isolated(Some(temp.path().join("library")), true).unwrap());
        let data = temp.path().join("stream");
        let plan: Plan = serde_json::from_value(json!({
            "schema_version":1, "launch_id":"launch", "process_lease":true,
            "edition_id":"edition", "runner_id":"runner.fake", "adapter":"blastem",
            "title":"Game", "source":"/fixture.md", "size":512, "mtime":0, "serial":null
        }))
        .unwrap();
        write_json(&root(&data).join("launches/test/launch.json"), &plan).unwrap();
        write_json(
            &root(&data).join("owner.json"),
            &Owner {
                library: key(&core),
                edition_id: plan.edition_id.clone(),
                runner_id: plan.runner_id.clone(),
                plan: "test".into(),
            },
        )
        .unwrap();
        let lease = process_lease(&data, &key(&core), &plan);
        fs::create_dir_all(lease.parent().unwrap()).unwrap();
        let holder = fs::File::create(&lease).unwrap();
        holder.lock().unwrap();
        core.with_db(|db| {
            db.set_setting("streaming.local_connection", &json!({"data_dir":data,"socket":data.join("streaming/wolf/state/runtime/wolf.sock")}).to_string())?;
            db.conn.execute("INSERT INTO editions(id,system,scan_title,imported_at) VALUES('edition','genesis','Game','2026-01-01')", [])?;
            db.insert_session_with_mode("session", "edition", "runner.fake", &now_rfc3339(), "running", "stream")
        }).unwrap();
        recover_stream(&core).unwrap();
        core.track_stream_start(&data, &plan).unwrap();
        let rows = core.with_db(|db| db.list_sessions()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].state, "running");
        // The public entry point now reaches media validation past the local-session gate.
        assert_eq!(
            core.headless_launch("edition").unwrap_err().message,
            "Game files are missing"
        );
        drop(holder);
        for _ in 0..40 {
            if core.with_db(|db| db.running_session()).unwrap().is_none() {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("stream watcher did not finish");
    }

    #[test]
    fn session_tracking_requires_a_live_emulator_lock() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("playing.lock");
        assert!(!playing(&path).unwrap());
        let holder = fs::File::create(&path).unwrap();
        holder.lock().unwrap();
        assert!(playing(&path).unwrap());
        drop(holder);
        assert!(!playing(&path).unwrap());
    }
}
