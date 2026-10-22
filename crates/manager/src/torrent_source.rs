//! Reviewed torrent artifacts. Transport completion never grants install readiness.
use crate::service::{Job, Manager, State};
use crate::torrent::{FileCandidate, Host};
use crate::{AppError, Result};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Source {
    pub info_hash: String,
    pub name: String,
    pub relative_path: PathBuf,
    pub download_directory: PathBuf,
    pub artifact: crate::browser::Artifact,
    pub piece_verified: bool,
}
impl Source {
    pub fn valid(&self) -> bool {
        self.info_hash.len() == 40
            && self.info_hash.bytes().all(|b| b.is_ascii_hexdigit())
            && self.download_directory.is_absolute()
            && !self.relative_path.as_os_str().is_empty()
            && self
                .relative_path
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_)))
            && self.artifact.filename == self.download_directory.join(&self.relative_path)
            && self.artifact.bytes > 0
            && self.artifact.sha256.len() == 64
            && self.artifact.sha256.bytes().all(|b| b.is_ascii_hexdigit())
            && !self.artifact.content_verified
            && self.piece_verified
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub path: PathBuf,
    pub size: u64,
    pub supported: bool,
    pub reason: Option<String>,
    pub preparation_id: Option<String>,
}
impl From<FileCandidate> for Artifact {
    fn from(file: FileCandidate) -> Self {
        Self {
            path: file.path,
            size: file.size,
            supported: file.supported,
            reason: file.reason,
            preparation_id: None,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Download {
    pub info_hash: String,
    pub name: String,
    pub directory: PathBuf,
    pub available: bool,
    pub error: Option<String>,
    pub files: Vec<Artifact>,
}
impl Manager {
    fn torrent_database(&self) -> Result<rusqlite::Connection> {
        let conn = self.database()?;
        conn.execute_batch("CREATE TABLE IF NOT EXISTS torrent_preparations(info_hash TEXT NOT NULL,relative_path TEXT NOT NULL,preparation_id TEXT NOT NULL UNIQUE,source_payload TEXT NOT NULL,PRIMARY KEY(info_hash,relative_path));")?;
        Ok(conn)
    }
    pub(crate) fn torrent_downloads(&self, host: &Host) -> Result<Vec<Download>> {
        let conn = self.torrent_database()?;
        let mut downloads = host.completed_downloads()?;
        for download in &mut downloads {
            for file in &mut download.files {
                file.preparation_id=conn.query_row("SELECT preparation_id FROM torrent_preparations WHERE info_hash=?1 AND relative_path=?2",params![download.info_hash,file.path.to_string_lossy()],|r| r.get(0)).optional()?;
            }
        }
        Ok(downloads)
    }
    pub(crate) fn inspect_torrent(
        &self,
        host: &Host,
        hash: &str,
        path: &Path,
        system: &str,
    ) -> Result<Job> {
        let _guard = self.claim()?;
        let mut conn = self.torrent_database()?;
        let linked:Option<(String,String)>=conn.query_row("SELECT preparation_id,source_payload FROM torrent_preparations WHERE info_hash=?1 AND relative_path=?2",params![hash,path.to_string_lossy()],|r| Ok((r.get(0)?,r.get(1)?))).optional()?;
        if let Some((id, payload)) = &linked {
            let job = self.get(id)?;
            let source: Source = serde_json::from_str(payload)?;
            if job.plan.system != system || job.torrent_source.as_ref() != Some(&source) {
                return Err(AppError::new("TORRENT_PREPARATION","This file already has a different preparation review; existing records were kept"));
            }
            // Immutable receipts survive source removal and worker restarts.
            if job.receipt.is_some() || job.state == State::Preparing {
                return Ok(job);
            }
        }
        let source = host.preparation_source(hash, path)?;
        if let Some((_, payload)) = &linked {
            if serde_json::from_str::<Source>(payload)? != source {
                return Err(AppError::new("TORRENT_SOURCE_CHANGED","Torrent artifact changed since its first review; original job and source were kept"));
            }
        }
        let mut job = self.build_review(
            &source.artifact.filename,
            system,
            None,
            Some(source.clone()),
        )?;
        if job.source_sha256 != source.artifact.sha256 || job.source_size != source.artifact.bytes {
            return Err(AppError::new(
                "TORRENT_SOURCE_CHANGED",
                "Torrent artifact changed during inspection",
            ));
        }
        if let Some((id, _)) = &linked {
            if id != &job.id {
                return Err(AppError::new(
                    "TORRENT_PREPARATION",
                    "Saved review differs; existing records were kept",
                ));
            }
        }
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let prior: Option<String> = tx
            .query_row("SELECT payload FROM jobs WHERE id=?1", [&job.id], |r| {
                r.get(0)
            })
            .optional()?;
        if let Some(prior) = prior {
            job = serde_json::from_str(&prior)?;
            if job.torrent_source.as_ref() != Some(&source) {
                return Err(AppError::new(
                    "TORRENT_PREPARATION",
                    "Preparation evidence does not match this torrent",
                ));
            }
            if job.state == State::Cancelled {
                job.state = State::AwaitingReview;
                job.error = None;
                job.error_code = None;
            }
        }
        tx.execute("INSERT INTO jobs(id,payload) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload",params![job.id,serde_json::to_string(&job)?])?;
        tx.execute("INSERT INTO torrent_preparations(info_hash,relative_path,preparation_id,source_payload) VALUES(?1,?2,?3,?4) ON CONFLICT(info_hash,relative_path) DO NOTHING",params![hash,path.to_string_lossy(),job.id,serde_json::to_string(&source)?])?;
        tx.commit()?;
        Ok(job)
    }
}
