use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const SCHEMA_VERSION: i64 = 1;
pub const SYSTEM_PS2: &str = "ps2";
pub const KIND_PS2_ISO: &str = "ps2_iso";
pub const KIND_PS2_CUE: &str = "ps2_cue";
pub const KIND_PS2_BIN: &str = "ps2_bin";
pub const KIND_PS2_CHD: &str = "ps2_chd";
pub const KIND_PS2_GZ: &str = "ps2_gz";
pub const KIND_PS2_CSO: &str = "ps2_cso";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RootKind {
    Folder,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RootStatus {
    Online,
    Offline,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryRoot {
    pub id: String,
    pub path: PathBuf,
    pub kind: RootKind,
    pub label: Option<String>,
    pub status: RootStatus,
    pub last_scan_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    Ps2Iso,
    Ps2Cue,
    Ps2Bin,
    Ps2Chd,
    Ps2Gz,
    Ps2Cso,
}

impl MediaKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ps2Iso => KIND_PS2_ISO,
            Self::Ps2Cue => KIND_PS2_CUE,
            Self::Ps2Bin => KIND_PS2_BIN,
            Self::Ps2Chd => KIND_PS2_CHD,
            Self::Ps2Gz => KIND_PS2_GZ,
            Self::Ps2Cso => KIND_PS2_CSO,
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            KIND_PS2_ISO => Some(Self::Ps2Iso),
            KIND_PS2_CUE => Some(Self::Ps2Cue),
            KIND_PS2_BIN => Some(Self::Ps2Bin),
            KIND_PS2_CHD => Some(Self::Ps2Chd),
            KIND_PS2_GZ => Some(Self::Ps2Gz),
            KIND_PS2_CSO => Some(Self::Ps2Cso),
            _ => None,
        }
    }

    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_ascii_lowercase().as_str() {
            "iso" => Some(Self::Ps2Iso),
            "cue" => Some(Self::Ps2Cue),
            "bin" => Some(Self::Ps2Bin),
            "chd" => Some(Self::Ps2Chd),
            "gz" => Some(Self::Ps2Gz),
            "cso" => Some(Self::Ps2Cso),
            _ => None,
        }
    }

    pub fn is_supported(&self) -> bool {
        matches!(self, Self::Ps2Iso)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaFile {
    pub id: String,
    pub edition_id: String,
    pub kind: MediaKind,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub mtime: i64,
    pub serial: Option<String>,
    pub serial_raw: Option<String>,
    pub region: Option<String>,
    pub available: bool,
    pub skip_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edition {
    pub id: String,
    pub system: String,
    pub title: String,
    pub scan_title: String,
    pub display_title: Option<String>,
    pub favorite: bool,
    pub media_id: Option<String>,
    pub last_played_at: Option<String>,
    pub playtime_seconds: i64,
    pub play_count: i64,
    pub imported_at: String,
    pub missing: bool,
    pub serial: Option<String>,
    pub region: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunnerKind {
    Pcsx2,
    Fake,
}

impl RunnerKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pcsx2 => "pcsx2",
            Self::Fake => "fake",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BiosState {
    Missing,
    Unverified,
    LaunchTested,
}

impl BiosState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Unverified => "unverified",
            Self::LaunchTested => "launch_tested",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "launch_tested" => Self::LaunchTested,
            "unverified" => Self::Unverified,
            _ => Self::Missing,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Runner {
    pub id: String,
    pub kind: RunnerKind,
    pub display_name: String,
    pub command: Vec<String>,
    pub bios_state: BiosState,
    pub bios_hint: Option<String>,
    pub source: String,
    pub available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchProfile {
    pub edition_id: String,
    pub runner_id: String,
    pub extra_args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Running,
    Ended,
    Failed,
}

impl SessionState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Ended => "ended",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub edition_id: String,
    pub runner_id: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_seconds: Option<i64>,
    pub exit_code: Option<i32>,
    pub state: SessionState,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScanReport {
    pub scanned: u32,
    pub added: u32,
    pub updated: u32,
    pub skipped_unsupported: u32,
    pub skipped_by_kind: Vec<SkipCount>,
    pub unresolved_serial: u32,
    pub missing: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkipCount {
    pub kind: String,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub last_root: Option<PathBuf>,
    pub last_collection: Option<String>,
    pub last_focus: Option<String>,
    pub selected_runner_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibrarySnapshot {
    pub roots: Vec<LibraryRoot>,
    pub editions: Vec<Edition>,
    pub media: Vec<MediaFile>,
    pub runners: Vec<Runner>,
    pub sessions: Vec<Session>,
    pub settings: Settings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchRequest {
    pub edition_id: String,
    pub extra_args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchEvent {
    pub session: Session,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CoreEvent {
    ScanStarted { root_id: String },
    ScanProgress { scanned: u32, added: u32 },
    ScanFinished { report: ScanReport },
    SessionStarted { session: Session },
    SessionEnded { session: Session },
    SessionFailed { session: Session },
    LibraryChanged,
}
