//! Read-only view of Xenia's content folders for one Xbox 360 title.
//! This lists folders so a save can be confirmed. It does not copy or interpret them.

use crate::error::{AppError, Result};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub title_id: Option<String>,
    pub folders: Vec<Folder>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Folder {
    pub relative: String,
    pub modified_at: String,
}

pub fn list(content_root: &Path, title_id: &str) -> Result<Report> {
    if title_id.len() != 8 || !title_id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(AppError::new(
            "BAD_REQUEST",
            "Xbox 360 title id must be eight hexadecimal characters",
        ));
    }
    let mut title_dirs = Vec::new();
    find_title_dirs(content_root, title_id, 0, &mut title_dirs);
    let mut folders = Vec::new();
    for title_dir in title_dirs {
        let mut children = false;
        if let Ok(entries) = fs::read_dir(&title_dir) {
            for entry in entries.flatten() {
                if !is_real_dir(&entry.path()) {
                    continue;
                }
                children = true;
                push(content_root, &entry.path(), &mut folders);
            }
        }
        if !children {
            push(content_root, &title_dir, &mut folders);
        }
    }
    folders.sort_by(|left, right| right.modified_at.cmp(&left.modified_at));
    folders.truncate(50);
    Ok(Report {
        title_id: Some(title_id.to_ascii_uppercase()),
        folders,
    })
}

fn find_title_dirs(dir: &Path, title_id: &str, depth: u8, out: &mut Vec<PathBuf>) {
    if depth > 2 || !dir.is_dir() {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !is_real_dir(&path) {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.eq_ignore_ascii_case(title_id) {
            out.push(path);
        } else if depth < 2 {
            find_title_dirs(&path, title_id, depth + 1, out);
        }
    }
}

fn is_real_dir(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|meta| meta.file_type().is_dir())
        .unwrap_or(false)
}

fn push(root: &Path, path: &Path, out: &mut Vec<Folder>) {
    let Ok(root) = root.canonicalize() else {
        return;
    };
    let Ok(path) = path.canonicalize() else {
        return;
    };
    if !path.starts_with(&root) {
        return;
    }
    let Ok(relative) = path.strip_prefix(&root) else {
        return;
    };
    let modified_at = fs::metadata(&path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|time| marquee_media::time::format_unix(time.as_secs() as i64))
        .unwrap_or_else(|| "unknown".into());
    out.push(Folder {
        relative: relative.to_string_lossy().replace('\\', "/"),
        modified_at,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_folders_are_listed_and_links_outside_the_content_root_are_ignored() {
        let root = tempfile::tempdir().unwrap();
        let save = root.path().join("53450841/00000001");
        fs::create_dir_all(&save).unwrap();
        fs::write(save.join("save.bin"), b"saved").unwrap();
        let outside = root.path().join("elsewhere");
        fs::create_dir(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, root.path().join("53450841/escape")).unwrap();
        let report = list(root.path(), "53450841").unwrap();
        assert_eq!(report.folders.len(), 1);
        assert_eq!(report.folders[0].relative, "53450841/00000001");
        assert!(list(root.path(), "../secret").is_err());
    }
}
