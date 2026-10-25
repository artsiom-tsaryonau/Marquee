//! Library-owned private-display launches. No arbitrary command manifests or desktop capture.
use crate::error::{AppError, Result};
use crate::source_builds::{run, valid_image};
use crate::{runners, Core};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

const APP: &str = "Marquee library game";
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
fn base_image(data: &Path) -> Result<String> {
    let receipt = read_json(&data.join("streaming/xenia-pilot/current.json")).map_err(|_| {
        err("Build the reviewed private-display runtime first; see Streaming setup")
    })?;
    let image = receipt["image"].as_str().unwrap_or_default();
    if !valid_image(image)
        || receipt["xenia_revision"] != "451f915566d57436555c6eb4ee77bb4717eb01de"
        || receipt["xenia_sha256"]
            != "b96b2723b0af55b5c4d837916e12035f172fe9ebbab2e90ba6c64f2f21fd5d8b"
    {
        return Err(err(
            "The private-display base image receipt is incompatible",
        ));
    }
    Ok(image.into())
}
fn build(data: &Path) -> Result<String> {
    let base = base_image(data)?;
    let mut hash = Sha256::new();
    hash.update(base.as_bytes());
    for (name, bytes) in FILES {
        hash.update(name);
        hash.update([0]);
        hash.update(bytes);
    }
    let hash = format!("{:x}", hash.finalize());
    let receipt_path = root(data).join("image.json");
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
            base_image(&c.data_dir)?;
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
        let _gate = self.headless_gate.lock().unwrap_or_else(|e| e.into_inner());
        if self.with_db(|db| db.running_session())?.is_some() {
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
                                return Ok(());
                            }
                            return Err(err(
                                "Close the other viewer before reconnecting this game",
                            ));
                        }
                        return self.streaming_open_viewer(app.id.clone());
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
            } else {
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
        let image = build(&c.data_dir)?;
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
                edition_id: plan.edition_id,
                runner_id: plan.runner_id,
                plan: plan_id,
            },
        )?;
        output(&mut command)?;
        for _ in 0..60 {
            if let Ok(host) = super::wolf::WolfClient::new(&c.socket)?.inspect() {
                if let Some(app) = host.apps.into_iter().find(|a| a.title == APP) {
                    return self.streaming_open_viewer(app.id);
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
        Ok(())
    }
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
}
