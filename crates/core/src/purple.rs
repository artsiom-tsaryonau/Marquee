//! A deliberately narrow, managed Windows recipe. Purple is the launcher;
//! successful setup is never evidence that Aion 2 itself works under Proton.
use crate::{
    error::{AppError, Result},
    http,
    paths::Paths,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

const UMU_URL: &str = "https://github.com/Open-Wine-Components/umu-launcher/releases/download/1.4.4/umu-launcher-1.4.4-zipapp.tar";
const UMU_SHA: &str = "eb590691841f7fad3fc3ad8fd5db4ccb87849fe7948e62b28ece7a4ee48cc851";
const INSTALLER_URL: &str =
    "https://gs-purple-inst.download.ncupdate.com/Purple/PURPLE_Installer_2_26_1019_7.exe";
const INSTALLER_SHA: &str = "99b882e490e6fa773eaf60cdfcf73956d0eecbc54435ae35e056062eeac0c52f";
const EXE: &str = "drive_c/Program Files (x86)/NCSOFT/Purple/PurpleLauncher.exe";
const VERBS: &[&str] = &[
    "vcrun2010",
    "d3dx10",
    "dotnet48",
    "vcrun2022",
    "corefonts",
    "arial",
];
const RECIPE: &str = "purple-2.26.1019.7-umu1.4.4-v1";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Receipt {
    recipe: String,
    runtime: Option<PathBuf>,
    completed: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub installed: bool,
    pub ready: bool,
    pub busy: bool,
    pub running: bool,
    pub message: String,
    pub error: Option<String>,
    pub prefix: String,
    pub log_path: String,
    pub runtime: Option<String>,
    pub aion2_verified: bool,
}

pub struct Manager {
    root: PathBuf,
    busy: AtomicBool,
    running: AtomicBool,
    cancel: AtomicBool,
    progress: Mutex<(String, Option<String>)>,
    operation: Mutex<()>,
}

impl Manager {
    pub fn new(paths: &Paths) -> Self {
        Self {
            root: if paths.data_dir.is_absolute() {
                paths.data_dir.clone()
            } else {
                std::env::current_dir()
                    .unwrap_or_default()
                    .join(&paths.data_dir)
            }
            .join("windows/purple"),
            busy: AtomicBool::new(false),
            running: AtomicBool::new(false),
            cancel: AtomicBool::new(false),
            operation: Mutex::new(()),
            progress: Mutex::new(("Install Purple to open it for Aion 2.".into(), None)),
        }
    }
    fn prefix(&self) -> PathBuf {
        self.root.join("prefix")
    }
    fn helper(&self) -> PathBuf {
        self.root.join("helpers/umu-1.4.4")
    }
    fn receipt(&self) -> Result<Receipt> {
        let path = self.root.join("receipt.json");
        if !path.exists() {
            return Ok(Receipt::default());
        }
        let file = fs::File::open(path)?;
        Ok(serde_json::from_reader(file.take(64 * 1024))?)
    }
    fn save(&self, receipt: &Receipt) -> Result<()> {
        let mut tmp = tempfile::NamedTempFile::new_in(&self.root)?;
        tmp.write_all(&serde_json::to_vec_pretty(receipt)?)?;
        tmp.as_file().sync_all()?;
        tmp.persist(self.root.join("receipt.json"))
            .map_err(|e| AppError::from(e.error))?;
        Ok(())
    }
    fn progress(&self, text: impl Into<String>) {
        *self.progress.lock().unwrap_or_else(|e| e.into_inner()) = (text.into(), None);
    }
    pub fn status(&self) -> Result<Status> {
        let receipt = self.receipt()?;
        let installed = valid_pe(&self.prefix().join(EXE));
        let runtime = receipt.runtime.as_ref().filter(|p| valid_runtime(p));
        let (message, error) = self
            .progress
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        Ok(Status {
            installed,
            ready: installed
                && receipt.recipe == RECIPE
                && receipt.completed.iter().any(|s| s == "installer")
                && runtime.is_some()
                && self.helper().is_file()
                && python().is_ok(),
            busy: self.busy.load(Ordering::Acquire),
            running: self.running.load(Ordering::Acquire),
            message,
            error,
            prefix: self.prefix().to_string_lossy().into_owned(),
            log_path: self.root.join("setup.log").to_string_lossy().into_owned(),
            runtime: runtime.map(|p| p.to_string_lossy().into_owned()),
            aion2_verified: false,
        })
    }
    /// The caller retains Core's instance lease until the worker finishes.
    pub fn start(self: &Arc<Self>, lease: Arc<crate::Core>) -> Result<Status> {
        let _operation = self.operation.lock().unwrap_or_else(|e| e.into_inner());
        supported_host()?;
        let version = Command::new(python()?)
            .args(["-c", "import sys; sys.exit(sys.version_info < (3,11))"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        if !version.success() {
            return Err(AppError::new(
                "RUNTIME",
                "Python 3.11 or newer is required for Purple setup.",
            ));
        }
        if self
            .busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(AppError::new("BUSY", "Purple setup is already running."));
        }
        if self.running.load(Ordering::Acquire) {
            self.busy.store(false, Ordering::Release);
            return Err(AppError::new(
                "BUSY",
                "Close Purple before changing its installation.",
            ));
        }
        self.cancel.store(false, Ordering::Release);
        self.progress("Preparing Purple setup…");
        let manager = Arc::clone(self);
        std::thread::spawn(move || {
            let _lease = lease;
            let result =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| manager.install()));
            let result = result.unwrap_or_else(|_| {
                Err(AppError::new(
                    "INSTALL",
                    "Purple setup was interrupted. Retry to resume.",
                ))
            });
            match result {
                Ok(()) => manager.progress(
                    "Purple is installed. Open it, sign in, then choose Aion 2 in Store.",
                ),
                Err(e) => {
                    *manager.progress.lock().unwrap_or_else(|e| e.into_inner()) = (
                        "Setup needs attention. Retry resumes completed steps.".into(),
                        Some(e.to_string()),
                    )
                }
            }
            manager.busy.store(false, Ordering::Release);
        });
        self.status()
    }
    pub fn cancel(&self) -> Result<Status> {
        let _operation = self.operation.lock().unwrap_or_else(|e| e.into_inner());
        if !self.busy.load(Ordering::Acquire) {
            return self.status();
        }
        self.cancel.store(true, Ordering::Release);
        self.progress("Stopping setup… A current download must finish first.");
        self.status()
    }
    fn check_cancel(&self) -> Result<()> {
        if self.cancel.load(Ordering::Acquire) {
            return Err(AppError::new(
                "CANCELLED",
                "Purple setup cancelled; existing files were kept.",
            ));
        }
        Ok(())
    }
    fn install(&self) -> Result<()> {
        fs::create_dir_all(&self.root)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.root, fs::Permissions::from_mode(0o700))?;
        }
        let mut receipt = self.receipt()?;
        // Keep an existing prefix. New recipe revisions rerun prerequisites.
        if receipt.recipe != RECIPE {
            receipt.completed.clear();
            receipt.recipe = RECIPE.into();
        }
        let temp = tempfile::tempdir_in(&self.root)?;
        self.progress("Downloading verified umu-launcher 1.4.4…");
        let tar = temp.path().join("umu.tar");
        download(UMU_URL, UMU_SHA, &tar, 1024 * 1024, |_| {})?;
        self.check_cancel()?;
        extract_helper(&tar, &self.helper())?;
        fs::create_dir_all(self.prefix())?;
        if receipt.runtime.is_none() {
            self.progress("Preparing GE-Proton and Steam runtime. This first download can take several minutes…");
            // A prior interrupted setup may have finished the runtime download.
            // Never update or delete a previously downloaded concrete build.
            if resolve_runtime(&self.root).is_err() {
                self.execute(None, &[""], "runtime")?;
            }
            receipt.runtime = Some(resolve_runtime(&self.root)?);
            self.save(&receipt)?;
        }
        let runtime = receipt.runtime.as_ref().unwrap().clone();
        if !valid_runtime(&runtime) {
            return Err(AppError::new("RUNTIME", "The pinned Purple runtime is missing. Restore the runtime shown in setup details before retrying."));
        }
        self.dependencies(&mut receipt, &runtime)?;
        if !valid_pe(&self.prefix().join(EXE))
            || !receipt.completed.iter().any(|s| s == "installer")
        {
            self.check_cancel()?;
            self.progress("Downloading NCSOFT Purple (388 MiB)…");
            let installer = temp.path().join("PurpleInstaller.exe");
            let mut last = 0;
            download(
                INSTALLER_URL,
                INSTALLER_SHA,
                &installer,
                512 * 1024 * 1024,
                |bytes| {
                    let mib = bytes / (1024 * 1024);
                    if mib >= last + 8 {
                        last = mib;
                        self.progress(format!("Downloading NCSOFT Purple: {mib} MiB / 388 MiB…"));
                    }
                },
            )?;
            self.check_cancel()?;
            self.progress("Complete the Purple installer window, keeping its default folder. Close Purple if it opens so setup can finish.");
            self.execute(
                Some(&runtime),
                &[installer
                    .to_str()
                    .ok_or_else(|| AppError::new("PATH", "Installer path is not UTF-8"))?],
                "installer",
            )?;
            if !valid_pe(&self.prefix().join(EXE)) {
                return Err(AppError::new("INSTALL", "The installer exited without PurpleLauncher.exe in its default folder. Retry setup and keep the default installation folder."));
            }
            if !receipt.completed.iter().any(|s| s == "installer") {
                receipt.completed.push("installer".into());
            }
            self.save(&receipt)?;
        }
        Ok(())
    }
    fn dependencies(&self, receipt: &mut Receipt, runtime: &Path) -> Result<()> {
        for verb in VERBS {
            self.check_cancel()?;
            if receipt.completed.iter().any(|s| s == verb) {
                continue;
            }
            // umu returns nonzero for already installed verbs. Recover a verb
            // that completed immediately before an interrupted receipt write.
            let installed = fs::read_to_string(self.prefix().join("winetricks.log"))
                .is_ok_and(|log| log.lines().any(|line| line.trim() == *verb));
            if !installed {
                self.progress(format!(
                    "Installing {verb}. Complete any dependency installer windows that appear…"
                ));
                self.execute(Some(runtime), &["winetricks", verb], verb)?;
            }
            receipt.completed.push((*verb).into());
            self.save(receipt)?;
        }
        Ok(())
    }
    fn command(&self, runtime: Option<&Path>, args: &[&str]) -> Result<Command> {
        let mut cmd = Command::new(python()?);
        cmd.arg(self.helper())
            .args(args)
            .env("WINEPREFIX", self.prefix())
            .env("WINEARCH", "win64")
            .env("UMU_FOLDERS_PATH", self.root.join("runtime"))
            .env(
                "PROTONPATH",
                runtime
                    .map(|p| p.as_os_str())
                    .unwrap_or(std::ffi::OsStr::new("GE-Proton")),
            )
            .env("GAMEID", "marquee-purple")
            .env("STORE", "ncsoft")
            .env("PROTON_VERB", "waitforexitandrun")
            .env_remove("WINEDLLOVERRIDES")
            .env_remove("WINESERVER")
            .env_remove("WINE")
            .env_remove("UMU_NO_RUNTIME")
            .env_remove("UMU_NO_PROTON")
            .env("PROTON_LOG", "0")
            .env("UMU_LOG", "warn")
            .current_dir(&self.root)
            .stdin(Stdio::null());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }
        Ok(cmd)
    }
    fn execute(&self, runtime: Option<&Path>, args: &[&str], stage: &str) -> Result<()> {
        self.check_cancel()?;
        let log = private_log(&self.root.join("setup.log"))?;
        writeln!(&log, "\n--- {stage} ---")?;
        let mut cmd = self.command(runtime, args)?;
        cmd.stdout(log.try_clone()?).stderr(log);
        let mut child = cmd.spawn().map_err(|e| {
            AppError::new("SPAWN_FAILED", format!("Could not start Purple setup: {e}"))
        })?;
        let started = Instant::now();
        loop {
            if let Some(status) = child.try_wait()? {
                return if status.success() {
                    Ok(())
                } else {
                    Err(AppError::new(
                        "INSTALL",
                        format!(
                            "{stage} exited with {status}. See the local setup log in Details."
                        ),
                    ))
                };
            }
            if fs::metadata(self.root.join("setup.log")).is_ok_and(|m| m.len() > 32 * 1024 * 1024) {
                #[cfg(unix)]
                unsafe {
                    libc::kill(-(child.id() as i32), libc::SIGKILL);
                }
                let _ = child.kill();
                let _ = child.wait();
                return Err(AppError::new("INSTALL", "Setup stopped because its diagnostic log exceeded 32 MiB. Inspect the log before retrying."));
            }
            if self.cancel.load(Ordering::Acquire)
                || started.elapsed() > Duration::from_secs(2 * 60 * 60)
            {
                #[cfg(unix)]
                unsafe {
                    libc::kill(-(child.id() as i32), libc::SIGKILL);
                }
                let _ = child.kill();
                let _ = child.wait();
                self.check_cancel()?;
                return Err(AppError::new(
                    "TIMEOUT",
                    format!("{stage} exceeded two hours. Retry to resume."),
                ));
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }
    pub fn open(self: &Arc<Self>, lease: Arc<crate::Core>) -> Result<()> {
        let _operation = self.operation.lock().unwrap_or_else(|e| e.into_inner());
        supported_host()?;
        if self.busy.load(Ordering::Acquire) {
            return Err(AppError::new("BUSY", "Wait for Purple setup to finish."));
        }
        if !self.status()?.ready {
            return Err(AppError::new(
                "NOT_READY",
                "Install or repair Purple first.",
            ));
        }
        if self
            .running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(AppError::new("BUSY", "Purple is already open."));
        }
        let result = (|| {
            let receipt = self.receipt()?;
            let exe = self.prefix().join(EXE);
            let mut cmd = self.command(
                receipt.runtime.as_deref(),
                &[exe
                    .to_str()
                    .ok_or_else(|| AppError::new("PATH", "Purple path is not UTF-8"))?],
            )?;
            // Account/session output is not captured into Marquee's setup log.
            cmd.stdout(Stdio::null()).stderr(Stdio::null());
            cmd.spawn().map_err(AppError::from)
        })();
        match result {
            Ok(mut child) => {
                let manager = Arc::clone(self);
                std::thread::spawn(move || {
                    let _lease = lease;
                    let result = child.wait();
                    manager.running.store(false, Ordering::Release);
                    if !matches!(result, Ok(s) if s.success()) {
                        *manager.progress.lock().unwrap_or_else(|e| e.into_inner()) = (
                            "Purple exited.".into(),
                            Some(
                                "Purple could not stay open. Linux compatibility is unverified."
                                    .into(),
                            ),
                        );
                    }
                });
                Ok(())
            }
            Err(e) => {
                self.running.store(false, Ordering::Release);
                Err(e)
            }
        }
    }
}

fn supported_host() -> Result<()> {
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Ok(())
    } else {
        Err(AppError::new(
            "UNSUPPORTED",
            "Managed Purple currently requires x86-64 Linux.",
        ))
    }
}
fn python() -> Result<PathBuf> {
    let path = crate::launchers::resolve("python3").ok_or_else(|| {
        AppError::new(
            "RUNTIME",
            "Install Python 3.11 or newer, then retry Purple setup.",
        )
    })?;
    Ok(path)
}
fn valid_pe(path: &Path) -> bool {
    let mut magic = [0; 2];
    fs::File::open(path)
        .and_then(|mut f| f.read_exact(&mut magic))
        .is_ok()
        && magic == *b"MZ"
}
fn valid_runtime(path: &Path) -> bool {
    path.is_absolute() && path.join("proton").is_file() && path.join("toolmanifest.vdf").is_file()
}
fn resolve_runtime(root: &Path) -> Result<PathBuf> {
    let base = root.join("runtime/Steam/compatibilitytools.d");
    let candidates: Vec<PathBuf> = fs::read_dir(&base)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("GE-Proton"))
                && valid_runtime(p)
        })
        .collect();
    if candidates.len() != 1 {
        return Err(AppError::new(
            "RUNTIME",
            "Could not identify the Purple runtime downloaded by umu. Check the local setup log.",
        ));
    }
    Ok(fs::canonicalize(&candidates[0])?)
}
fn private_log(path: &Path) -> Result<fs::File> {
    // Bound retained diagnostics. No Purple sign-in output is written here.
    let mut options = fs::OpenOptions::new();
    options.create(true).write(true);
    if fs::metadata(path).is_ok_and(|m| m.len() > 8 * 1024 * 1024) {
        options.truncate(true);
    } else {
        options.append(true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    Ok(options.open(path)?)
}
fn download(url: &str, sha: &str, path: &Path, max: u64, progress: impl FnMut(u64)) -> Result<()> {
    let actual = http::download_file(url, path, max, Duration::from_secs(30 * 60), progress)?;
    if actual != sha {
        return Err(AppError::new("CHECKSUM", "Download checksum did not match the pinned Purple recipe. No downloaded program was run."));
    }
    Ok(())
}
fn extract_helper(tar: &Path, dest: &Path) -> Result<()> {
    let mut archive = tar::Archive::new(fs::File::open(tar)?);
    let mut helper = None;
    for entry in archive.entries()? {
        let mut entry = entry?;
        if entry.path()?.as_ref() != Path::new("umu/umu-run") {
            continue;
        }
        if helper.is_some() || !entry.header().entry_type().is_file() || entry.size() > 1024 * 1024
        {
            return Err(AppError::new("ARCHIVE", "Unexpected umu helper entry"));
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        helper = Some(bytes);
    }
    let bytes = helper.ok_or_else(|| AppError::new("ARCHIVE", "umu archive has no helper"))?;
    fs::create_dir_all(dest.parent().unwrap())?;
    let mut tmp = tempfile::NamedTempFile::new_in(dest.parent().unwrap())?;
    tmp.write_all(&bytes)?;
    tmp.as_file().sync_all()?;
    tmp.persist(dest).map_err(|e| AppError::from(e.error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    fn manager(dir: &Path) -> Manager {
        let paths = Paths::resolve(Some(dir.join("Marquee with spaces"))).unwrap();
        let manager = Manager::new(&paths);
        fs::create_dir_all(manager.helper().parent().unwrap()).unwrap();
        fs::create_dir_all(manager.prefix()).unwrap();
        manager
    }
    fn runtime(manager: &Manager) -> PathBuf {
        let path = manager
            .root
            .join("runtime/Steam/compatibilitytools.d/GE-Proton-test");
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("proton"), "test").unwrap();
        fs::write(path.join("toolmanifest.vdf"), "test").unwrap();
        path
    }
    fn fake_helper(manager: &Manager) {
        fs::write(manager.helper(), r#"import os,sys,json,pathlib
root=pathlib.Path.cwd()
with open(root/'calls.jsonl','a') as f: f.write(json.dumps({'args':sys.argv[1:],'prefix':os.environ['WINEPREFIX'],'runtime':os.environ['PROTONPATH'],'folders':os.environ['UMU_FOLDERS_PATH']})+'\n')
if (root/'fail').exists() and sys.argv[-1] == 'dotnet48': sys.exit(7)
"#).unwrap();
    }
    #[test]
    fn failed_dependency_is_not_checkpointed_and_retry_resumes_in_order() {
        let temp = tempfile::tempdir().unwrap();
        let manager = manager(temp.path());
        let runtime = runtime(&manager);
        fake_helper(&manager);
        fs::write(manager.root.join("fail"), "").unwrap();
        let mut receipt = Receipt {
            recipe: RECIPE.into(),
            runtime: Some(runtime.clone()),
            completed: vec![],
        };
        assert!(manager.dependencies(&mut receipt, &runtime).is_err());
        assert_eq!(
            manager.receipt().unwrap().completed,
            vec!["vcrun2010", "d3dx10"]
        );
        fs::remove_file(manager.root.join("fail")).unwrap();
        let mut resumed = manager.receipt().unwrap();
        manager.dependencies(&mut resumed, &runtime).unwrap();
        assert_eq!(resumed.completed, VERBS);
        let calls: Vec<Value> = fs::read_to_string(manager.root.join("calls.jsonl"))
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(calls.len(), 7);
        assert_eq!(
            calls[0]["args"],
            serde_json::json!(["winetricks", "vcrun2010"])
        );
        assert_eq!(
            calls[3]["args"],
            serde_json::json!(["winetricks", "dotnet48"])
        );
        assert_eq!(calls[0]["prefix"], manager.prefix().to_str().unwrap());
        assert_eq!(calls[0]["runtime"], runtime.to_str().unwrap());
        assert!(!manager.status().unwrap().ready);
        assert!(!manager.status().unwrap().aion2_verified);
    }
    #[test]
    fn interrupted_receipt_recovers_from_exact_winetricks_log_entries() {
        let temp = tempfile::tempdir().unwrap();
        let manager = manager(temp.path());
        let runtime = runtime(&manager);
        fake_helper(&manager);
        fs::write(
            manager.prefix().join("winetricks.log"),
            "vcrun2010\nd3dx10\ndotnet48\nvcrun2022\ncorefonts\narial\n",
        )
        .unwrap();
        let mut receipt = Receipt::default();
        manager.dependencies(&mut receipt, &runtime).unwrap();
        assert_eq!(receipt.completed, VERBS);
        assert!(!manager.root.join("calls.jsonl").exists());
    }
    #[test]
    fn exe_alone_is_not_ready_and_runtime_never_falls_back() {
        let temp = tempfile::tempdir().unwrap();
        let manager = manager(temp.path());
        let runtime = runtime(&manager);
        fake_helper(&manager);
        fs::create_dir_all(manager.prefix().join(EXE).parent().unwrap()).unwrap();
        fs::write(manager.prefix().join(EXE), b"MZtest").unwrap();
        assert!(manager.status().unwrap().installed);
        assert!(!manager.status().unwrap().ready);
        manager
            .save(&Receipt {
                recipe: RECIPE.into(),
                runtime: Some(runtime.clone()),
                completed: vec!["installer".into()],
            })
            .unwrap();
        assert!(manager.status().unwrap().ready);
        assert!(!manager.status().unwrap().aion2_verified);
        fs::remove_file(runtime.join("proton")).unwrap();
        assert!(!manager.status().unwrap().ready);
    }
    #[test]
    fn pinned_command_preserves_metacharacters_as_one_argument() {
        let temp = tempfile::tempdir().unwrap();
        let manager = manager(temp.path());
        let runtime = runtime(&manager);
        fake_helper(&manager);
        let arg = "setup space; $(touch NO) `false`.exe";
        manager.execute(Some(&runtime), &[arg], "test").unwrap();
        let call: Value = serde_json::from_str(
            fs::read_to_string(manager.root.join("calls.jsonl"))
                .unwrap()
                .trim(),
        )
        .unwrap();
        assert_eq!(call["args"], serde_json::json!([arg]));
        assert!(!manager.root.join("NO").exists());
    }
    #[test]
    fn cancellation_stops_the_owned_process_without_marking_success() {
        let temp = tempfile::tempdir().unwrap();
        let manager = Arc::new(manager(temp.path()));
        let runtime = runtime(&manager);
        fs::write(manager.helper(), "import time\ntime.sleep(30)\n").unwrap();
        let other = Arc::clone(&manager);
        let worker = std::thread::spawn(move || other.execute(Some(&runtime), &[""], "test"));
        std::thread::sleep(Duration::from_millis(250));
        manager.cancel.store(true, Ordering::Release);
        assert_eq!(worker.join().unwrap().unwrap_err().code, "CANCELLED");
        assert!(!manager.status().unwrap().ready);
    }
    #[test]
    fn archive_rejects_link_helper_and_leaves_existing_helper_intact() {
        let temp = tempfile::tempdir().unwrap();
        let archive = temp.path().join("bad.tar");
        let dest = temp.path().join("helper");
        fs::write(&dest, "existing").unwrap();
        let mut builder = tar::Builder::new(fs::File::create(&archive).unwrap());
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Symlink);
        header.set_size(0);
        header.set_mode(0o777);
        header.set_cksum();
        builder
            .append_link(&mut header, "umu/umu-run", "/tmp/unrelated")
            .unwrap();
        builder.finish().unwrap();
        assert!(extract_helper(&archive, &dest).is_err());
        assert_eq!(fs::read_to_string(&dest).unwrap(), "existing");
    }
    #[test]
    fn archive_extracts_only_the_known_helper_without_executing_it() {
        let temp = tempfile::tempdir().unwrap();
        let archive = temp.path().join("good.tar");
        let dest = temp.path().join("helper");
        let mut builder = tar::Builder::new(fs::File::create(&archive).unwrap());
        let mut header = tar::Header::new_gnu();
        header.set_size(6);
        header.set_mode(0o755);
        header.set_cksum();
        builder
            .append_data(&mut header, "umu/umu-run", &b"helper"[..])
            .unwrap();
        builder.finish().unwrap();
        extract_helper(&archive, &dest).unwrap();
        assert_eq!(fs::read_to_string(&dest).unwrap(), "helper");
    }

    #[test]
    fn shared_api_opens_launcher_without_creating_a_game_session() {
        let temp = tempfile::tempdir().unwrap();
        let core =
            Arc::new(crate::Core::open_isolated(Some(temp.path().join("library")), false).unwrap());
        let manager = &core.purple;
        fs::create_dir_all(manager.helper().parent().unwrap()).unwrap();
        let runtime = runtime(manager);
        fake_helper(manager);
        let exe = manager.prefix().join(EXE);
        fs::create_dir_all(exe.parent().unwrap()).unwrap();
        fs::write(&exe, b"MZtest").unwrap();
        manager
            .save(&Receipt {
                recipe: RECIPE.into(),
                runtime: Some(runtime),
                completed: vec!["installer".into()],
            })
            .unwrap();
        crate::api::dispatch(&core, "open_purple", serde_json::json!({})).unwrap();
        for _ in 0..60 {
            if !manager.running.load(Ordering::Acquire) {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(!manager.running.load(Ordering::Acquire));
        let call: Value = serde_json::from_str(
            fs::read_to_string(manager.root.join("calls.jsonl"))
                .unwrap()
                .trim(),
        )
        .unwrap();
        assert_eq!(call["args"], serde_json::json!([exe]));
        assert!(core.snapshot().unwrap().sessions.is_empty());
        let status = crate::api::dispatch(&core, "purple_status", serde_json::json!({})).unwrap();
        assert_eq!(status["ready"], true);
        assert_eq!(status["aion2_verified"], false);
    }
}
