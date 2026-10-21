//! Conservative preflight for arcade ZIP sets. Never unpack or rename a set.
use std::path::Path;

pub fn issue(path: &Path) -> Option<String> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(err) => return Some(format!("Cannot read Neo Geo set: {err}")),
    };
    let archive = match zip::ZipArchive::new(file) {
        Ok(archive) => archive,
        Err(err) => return Some(format!("Invalid Neo Geo ZIP: {err}")),
    };
    if archive.is_empty() || archive.len() > 4096 {
        return Some("Neo Geo set is empty or has too many entries".into());
    }
    // This historical split clone is not a driver name in the pinned FBNeo
    // source. The supplied archive contains graphics/audio CPU data only; do
    // not silently relabel it as a modern fully decrypted set.
    if path
        .file_stem()
        .is_some_and(|s| s.eq_ignore_ascii_case("samsh5nd"))
    {
        return Some("Legacy samsh5nd split set: main program and sound sample ROMs are absent from this archive. Supply a complete ROM set matching current FBNeo plus neogeo.zip; renaming this ZIP is not a repair.".into());
    }
    if !path.parent().is_some_and(|dir| {
        std::fs::read_dir(dir).is_ok_and(|entries| {
            entries
                .flatten()
                .any(|e| e.file_name().eq_ignore_ascii_case("neogeo.zip") && e.path().is_file())
        })
    }) {
        return Some("Neo Geo BIOS missing: place a matching neogeo.zip beside the game ZIP. Split sets may also need their parent archive. File presence does not verify ROM-set compatibility.".into());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn split_sets_and_missing_bios_have_actionable_diagnostics() {
        let temp = tempfile::tempdir().unwrap();
        let set = temp.path().join("samsh5nd.zip");
        let mut archive = zip::ZipWriter::new(std::fs::File::create(&set).unwrap());
        archive
            .start_file("ss5_c1.rom", zip::write::SimpleFileOptions::default())
            .unwrap();
        archive.write_all(b"fixture").unwrap();
        archive.finish().unwrap();
        assert!(issue(&set).unwrap().contains("main program"));
        let other = temp.path().join("other.zip");
        std::fs::rename(set, &other).unwrap();
        assert!(issue(&other).unwrap().contains("BIOS missing"));
        std::fs::write(temp.path().join("neogeo.zip"), b"fixture").unwrap();
        assert!(issue(&other).is_none()); // Presence only, never a compatibility claim.
    }
}
