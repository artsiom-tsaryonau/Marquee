//! Length-bounded JSON frames, one request per connection. Snapshot polling is
//! advertised; event replay and arbitrary command execution are not supported.
use crate::{AppError, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::io::{Read, Write};
use std::path::PathBuf;

pub const VERSION: u32 = 1;
pub const MAX_FRAME: usize = 32 * 1024 * 1024;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub protocol_version: u32,
    pub request_id: String,
    pub operation: Operation,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Hello,
    Browser {
        client_id: String,
        request: crate::browser::Request,
    },
    BrowserList,
    BrowserCancel {
        id: String,
    },
    List,
    Get {
        id: String,
    },
    Inspect {
        source: PathBuf,
        system: String,
    },
    Start {
        id: String,
        review_token: String,
    },
    Cancel {
        id: String,
    },
    Acknowledge {
        id: String,
        receipt_fingerprint: String,
        edition_ids: Vec<String>,
    },
    PublicationFailed {
        id: String,
        receipt_fingerprint: String,
        code: String,
        message: String,
    },
    Stop,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub protocol_version: u32,
    pub request_id: String,
    pub result: std::result::Result<Value, AppError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Status {
    pub running: bool,
    pub pid: Option<u32>,
    pub stopping: bool,
    pub protocol_version: u32,
    pub snapshot_polling: bool,
    pub event_replay: bool,
    pub active_cancel: bool,
}

pub fn write_frame(stream: &mut impl Write, value: &impl Serialize) -> Result<()> {
    let bytes = serde_json::to_vec(value)?;
    if bytes.len() > MAX_FRAME {
        return Err(AppError::new(
            "MANAGER_FRAME",
            "Manager message exceeds 32 MiB",
        ));
    }
    stream.write_all(&(bytes.len() as u32).to_le_bytes())?;
    stream.write_all(&bytes)?;
    stream.flush()?;
    Ok(())
}

pub fn read_frame<T: DeserializeOwned>(stream: &mut impl Read) -> Result<T> {
    let mut size = [0; 4];
    stream.read_exact(&mut size)?;
    let size = u32::from_le_bytes(size) as usize;
    if size == 0 || size > MAX_FRAME {
        return Err(AppError::new("MANAGER_FRAME", "Invalid manager frame size"));
    }
    let mut bytes = vec![0; size];
    stream.read_exact(&mut bytes)?;
    Ok(serde_json::from_slice(&bytes)?)
}
