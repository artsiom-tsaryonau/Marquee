use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const SCHEMA_VERSION: i64 = 4;
pub const SYSTEM_PS2: &str = "ps2";
pub const SYSTEM_PS1: &str = "ps1";
pub const KIND_PS2_ISO: &str = "ps2_iso";
pub const KIND_PS1_ISO: &str = "ps1_iso";
pub const KIND_PS1_CHD: &str = "ps1_chd";
pub const KIND_PS1_CUE: &str = "ps1_cue";

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
    /// System id this folder holds images for (`ps2`, `ps1`).
    pub system: String,
    pub label: Option<String>,
    pub status: RootStatus,
    pub last_scan_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    /// `.iso` (PS2 and, for now, the catch-all for discs whose system has no
    /// own kind yet).
    Iso,
    /// `.iso` bound to a PS1 root.
    Ps1Iso,
    /// `.chd` bound to a PS1 root (DuckStation reads CHD directly).
    Ps1Chd,
    /// `.cue` sheet bound to a PS1 root. The sheet names a `.bin` track next to
    /// it; the emulator is handed the sheet, the serial is read from the track.
    Ps1Cue,
}

impl MediaKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Iso => KIND_PS2_ISO,
            Self::Ps1Iso => KIND_PS1_ISO,
            Self::Ps1Chd => KIND_PS1_CHD,
            Self::Ps1Cue => KIND_PS1_CUE,
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            KIND_PS2_ISO => Some(Self::Iso),
            KIND_PS1_ISO => Some(Self::Ps1Iso),
            KIND_PS1_CHD => Some(Self::Ps1Chd),
            KIND_PS1_CUE => Some(Self::Ps1Cue),
            _ => None,
        }
    }

    /// True when the file itself is ISO9660 and can be read for a serial. `.cue`
    /// points at a sibling track and `.chd` is compressed, so both resolve their
    /// serial another way.
    pub fn is_readable_image(&self) -> bool {
        matches!(self, Self::Iso | Self::Ps1Iso)
    }

    /// Extension-to-kind within a known system. `.iso` is a supported image for
    /// every system Marquee ships; `.chd` and `.cue` only for systems whose
    /// runner reads them (PS1/DuckStation).
    ///
    /// A `.bin` track is deliberately *not* a kind: it is part of a `.cue` pair
    /// and importing it separately would double-count one disc.
    pub fn for_system_extension(system: &str, ext: &str) -> Option<Self> {
        match ext.to_ascii_lowercase().as_str() {
            "iso" => Some(if system == SYSTEM_PS1 {
                Self::Ps1Iso
            } else {
                Self::Iso
            }),
            "chd" if system == SYSTEM_PS1 => Some(Self::Ps1Chd),
            "cue" if system == SYSTEM_PS1 => Some(Self::Ps1Cue),
            _ => None,
        }
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
    pub cover_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunnerKind {
    Pcsx2,
    Duckstation,
    Fake,
}

impl RunnerKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pcsx2 => "pcsx2",
            Self::Duckstation => "duckstation",
            Self::Fake => "fake",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "duckstation" => Self::Duckstation,
            "fake" => Self::Fake,
            _ => Self::Pcsx2,
        }
    }

    /// True for emulators that boot from a BIOS, which is what `bios_state`
    /// tracks. The fake test runner never is.
    pub fn needs_bios(&self) -> bool {
        matches!(self, Self::Pcsx2 | Self::Duckstation)
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
    pub warnings: Vec<String>,
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
    #[serde(default)]
    pub selected_runner_ids: std::collections::BTreeMap<String, String>,
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
    ScanStarted {
        root_id: String,
    },
    ScanProgress {
        scanned: u32,
        added: u32,
    },
    ScanFinished {
        report: ScanReport,
    },
    SessionStarted {
        session: Session,
    },
    SessionEnded {
        session: Session,
    },
    SessionFailed {
        session: Session,
    },
    LibraryChanged,
    LaunchersChanged,
    LutrisChanged,
    ToolInstallStarted {
        tool_id: String,
        channel: String,
    },
    ToolInstallProgress {
        tool_id: String,
        message: String,
    },
    ToolInstallFinished {
        tool_id: String,
        runner_id: Option<String>,
    },
    ToolInstallFailed {
        tool_id: String,
        message: String,
    },
    PortProgress {
        port_id: String,
        message: String,
    },
    ArtworkFinished {
        fetched: u32,
        skipped: u32,
        failed: u32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ArtworkReport {
    pub fetched: u32,
    pub skipped: u32,
    pub failed: u32,
}
