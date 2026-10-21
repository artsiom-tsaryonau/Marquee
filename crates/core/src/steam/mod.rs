//! Read-only installed Steam discovery. Client handoff, persistent inventory,
//! account ownership and verified game lifecycle are separate increments.
mod keyvalues;
use crate::error::{AppError, Result};
use keyvalues::Value;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientKind {
    Native,
    Flatpak,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Client {
    pub id: String,
    pub kind: ClientKind,
    pub root: PathBuf,
    pub executable: Option<PathBuf>,
    pub available: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Library {
    pub path: PathBuf,
    pub clients: Vec<String>,
    pub offline: bool,
    pub scan_complete: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Installation {
    pub id: String,
    pub appid: u32,
    pub name: String,
    pub library: PathBuf,
    pub install_dir: PathBuf,
    pub build_id: Option<u64>,
    pub size_bytes: Option<u64>,
    pub state_flags: u64,
    pub files_present: bool,
    pub manifest_complete: bool,
    pub clients: Vec<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Inventory {
    pub clients: Vec<Client>,
    pub libraries: Vec<Library>,
    pub installations: Vec<Installation>,
    pub warnings: Vec<String>,
}
fn invalid(message: &str) -> AppError {
    AppError::new("STEAM_MANIFEST", message)
}
fn fingerprint(path: &Path) -> String {
    format!("{:x}", Sha256::digest(crate::paths::path_bytes(path)))[..24].into()
}
fn location(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
fn read(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    if file.metadata()?.len() > 4 * 1024 * 1024 {
        return Err(invalid("Steam text file exceeds 4 MiB"));
    }
    let mut text = String::new();
    file.by_ref()
        .take(4 * 1024 * 1024 + 1)
        .read_to_string(&mut text)?;
    if text.len() > 4 * 1024 * 1024 {
        return Err(invalid("Steam text file grew beyond 4 MiB"));
    }
    Ok(text)
}
fn libraries(text: &str, root: &Path) -> Result<Vec<PathBuf>> {
    let parsed = keyvalues::parse(text)?;
    let folders = parsed
        .get("libraryfolders")?
        .ok_or_else(|| invalid("Missing libraryfolders object"))?
        .object()?;
    let mut paths = BTreeSet::from([location(root)]);
    for (index, value) in folders {
        if !index.chars().all(|c| c.is_ascii_digit()) || index.is_empty() {
            continue;
        }
        let path = match value {
            Value::Text(path) => path.as_str(),
            Value::Object(_) => value.required("path")?,
        };
        let path = Path::new(path);
        if !path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir)) {
            return Err(invalid(
                "Steam library path must be absolute without parent traversal",
            ));
        }
        paths.insert(location(path));
        if paths.len() > 1000 {
            return Err(invalid("Steam library count exceeds the limit"));
        }
    }
    Ok(paths.into_iter().collect())
}
fn optional_number(root: &Value, key: &str) -> Result<Option<u64>> {
    root.get(key)?
        .map(|v| {
            v.text()?
                .parse::<u64>()
                .map_err(|_| invalid("Invalid numeric manifest field"))
        })
        .transpose()
}
fn manifest(text: &str, path: &Path, appid: u32, clients: &[String]) -> Result<Installation> {
    let root = keyvalues::parse(text)?;
    let app = root
        .get("AppState")?
        .ok_or_else(|| invalid("Missing AppState object"))?;
    let actual = app
        .required("appid")?
        .parse::<u32>()
        .map_err(|_| invalid("Invalid app ID"))?;
    if actual == 0 || actual != appid {
        return Err(invalid("Manifest app ID does not match its filename"));
    }
    let name = app.required("name")?.trim();
    if name.is_empty() || name.len() > 1024 || name.contains(['\n', '\r']) {
        return Err(invalid("Invalid application name"));
    }
    let directory = app.required("installdir")?;
    if directory.is_empty()
        || directory.contains(['/', '\\'])
        || directory == "."
        || directory == ".."
    {
        return Err(invalid(
            "Installation directory must be a single safe folder name",
        ));
    }
    let install_dir = path.join("steamapps/common").join(directory);
    let state_flags = optional_number(app, "StateFlags")?
        .ok_or_else(|| invalid("Missing installation state flags"))?;
    Ok(Installation {
        id: format!("steam-install:{}:{appid}", fingerprint(path)),
        appid,
        name: name.into(),
        library: path.into(),
        files_present: install_dir.is_dir(),
        install_dir,
        build_id: optional_number(app, "buildid")?,
        size_bytes: optional_number(app, "SizeOnDisk")?,
        state_flags,
        manifest_complete: state_flags == 4,
        clients: clients.to_vec(),
    })
}

/// Path suggestions are canonicalized so ~/.steam/steam and ~/.local/share/Steam
/// do not duplicate a native client. Native and Flatpak owners remain distinct.
pub fn discover() -> Inventory {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return Inventory {
            warnings: vec!["HOME unavailable; Steam discovery was skipped".into()],
            ..Inventory::default()
        };
    };
    let data = std::env::var_os("XDG_DATA_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local/share"));
    let candidates = [
        (ClientKind::Native, data.join("Steam")),
        (ClientKind::Native, home.join(".local/share/Steam")),
        (ClientKind::Native, home.join(".steam/steam")),
        (ClientKind::Native, home.join(".steam/root")),
        (
            ClientKind::Flatpak,
            home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
        ),
    ];
    let mut seen = BTreeSet::new();
    let mut clients = vec![];
    for (kind, root) in candidates {
        if !root.is_dir() {
            continue;
        }
        let root = location(&root);
        let label = if kind == ClientKind::Native {
            "native"
        } else {
            "flatpak"
        };
        let id = format!("steam-client:{label}:{}", fingerprint(&root));
        if !seen.insert(id.clone()) {
            continue;
        }
        let executable = if kind == ClientKind::Native {
            crate::launchers::resolve("steam")
                .or_else(|| crate::launchers::resolve(&root.join("steam.sh").to_string_lossy()))
        } else {
            crate::launchers::resolve("flatpak")
        };
        let available = executable.is_some()
            && (kind == ClientKind::Native
                || [
                    data.join("flatpak/app/com.valvesoftware.Steam/current/active/metadata"),
                    PathBuf::from(
                        "/var/lib/flatpak/app/com.valvesoftware.Steam/current/active/metadata",
                    ),
                ]
                .iter()
                .any(|p| p.is_file()));
        clients.push(Client {
            id,
            kind,
            root,
            executable,
            available,
        });
    }
    scan(clients)
}

pub fn scan(clients: Vec<Client>) -> Inventory {
    let mut result = Inventory {
        clients,
        ..Inventory::default()
    };
    let mut roots: BTreeMap<PathBuf, BTreeSet<String>> = BTreeMap::new();
    for client in &result.clients {
        let config = client.root.join("steamapps/libraryfolders.vdf");
        let paths = match read(&config).and_then(|s| libraries(&s, &client.root)) {
            Ok(paths) => paths,
            Err(error) => {
                result.warnings.push(format!(
                    "Could not read all libraries for {}: {}. Only the default root was scanned.",
                    client.root.display(),
                    error.message
                ));
                vec![location(&client.root)]
            }
        };
        for path in paths {
            roots.entry(path).or_default().insert(client.id.clone());
        }
    }
    for (path, owners) in roots {
        let owners: Vec<_> = owners.into_iter().collect();
        let steamapps = path.join("steamapps");
        let mut library = Library {
            offline: !steamapps.is_dir(),
            scan_complete: true,
            path: path.clone(),
            clients: owners.clone(),
        };
        let entries = match fs::read_dir(&steamapps) {
            Ok(entries) => entries,
            Err(_) => {
                library.scan_complete = false;
                result
                    .warnings
                    .push(format!("Steam library unavailable: {}", path.display()));
                result.libraries.push(library);
                continue;
            }
        };
        let mut manifests = vec![];
        for (index, entry) in entries.take(50_001).enumerate() {
            if index == 50_000 {
                library.scan_complete = false;
                result
                    .warnings
                    .push("Steam library directory-entry limit exceeded".into());
                break;
            }
            let Ok(entry) = entry else {
                library.scan_complete = false;
                continue;
            };
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(id) = name
                .strip_prefix("appmanifest_")
                .and_then(|s| s.strip_suffix(".acf"))
            {
                if let Ok(appid) = id.parse::<u32>() {
                    manifests.push((appid, entry.path()));
                }
            }
        }
        manifests.sort_by(|a, b| a.1.cmp(&b.1));
        if manifests.len() > 10_000 {
            library.scan_complete = false;
            result
                .warnings
                .push("Steam library manifest limit exceeded".into());
            result.libraries.push(library);
            continue;
        }
        let mut installed = BTreeMap::new();
        let mut ambiguous = BTreeSet::new();
        for (appid, file) in manifests {
            match read(&file).and_then(|s| manifest(&s, &path, appid, &owners)) {
                Ok(entry) => {
                    if installed.insert(appid, entry).is_some() {
                        ambiguous.insert(appid);
                    }
                }
                Err(error) => {
                    library.scan_complete = false;
                    result
                        .warnings
                        .push(format!("Skipped {}: {}", file.display(), error.message));
                }
            }
        }
        for appid in ambiguous {
            installed.remove(&appid);
            library.scan_complete = false;
            result
                .warnings
                .push(format!("Ambiguous Steam app {appid} in {}", path.display()));
        }
        result.installations.extend(installed.into_values());
        result.libraries.push(library);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn client(root: &Path, id: &str, kind: ClientKind) -> Client {
        Client {
            id: id.into(),
            kind,
            root: root.into(),
            executable: None,
            available: false,
        }
    }
    fn write_manifest(root: &Path, id: u32, directory: &str, flags: u64) -> PathBuf {
        fs::create_dir_all(root.join("steamapps")).unwrap();
        let file = root.join(format!("steamapps/appmanifest_{id}.acf"));
        fs::write(&file,format!(r#""AppState" {{ "appid" "{id}" "name" "Game {id}" "installdir" "{directory}" "StateFlags" "{flags}" "buildid" "123" "SizeOnDisk" "9876543210" }}"#)).unwrap();
        file
    }
    #[test]
    fn reads_modern_and_legacy_library_config_without_merging_different_locations() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("root");
        let other = temp.path().join("other");
        let paths = libraries(
            &format!(
                r#""LibraryFolders" {{ "0" {{ "path" "{}" "apps" {{ "1" "100" }} }} "1" "{}" }}"#,
                root.display(),
                other.display()
            ),
            &root,
        )
        .unwrap();
        assert_eq!(paths.len(), 2);
        assert!(paths.contains(&other));
        assert!(libraries(r#"libraryfolders { 0 { path "../outside" } }"#, &root).is_err());
        assert!(libraries(
            r#"libraryfolders { 0 { path "/first" path "/second" } }"#,
            &root
        )
        .is_err());
    }
    #[test]
    fn shared_library_has_one_installation_with_native_and_flatpak_choices() {
        let temp = tempfile::tempdir().unwrap();
        let native = temp.path().join("native");
        let flatpak = temp.path().join("flatpak");
        let shared = temp.path().join("shared");
        for root in [&native, &flatpak] {
            fs::create_dir_all(root.join("steamapps")).unwrap();
            fs::write(
                root.join("steamapps/libraryfolders.vdf"),
                format!(
                    r#"libraryfolders {{ 0 {{ path "{}" }} }}"#,
                    shared.display()
                ),
            )
            .unwrap();
        }
        let manifest = write_manifest(&shared, 42, "Game Folder", 4);
        fs::create_dir_all(shared.join("steamapps/common/Game Folder")).unwrap();
        let before = fs::read(&manifest).unwrap();
        let clients = vec![
            client(&native, "native", ClientKind::Native),
            client(&flatpak, "flatpak", ClientKind::Flatpak),
        ];
        let inventory = scan(clients.clone());
        assert_eq!(inventory.installations.len(), 1);
        let entry = &inventory.installations[0];
        assert_eq!(entry.clients, vec!["flatpak", "native"]);
        assert!(entry.files_present && entry.manifest_complete);
        assert_eq!(entry.size_bytes, Some(9876543210));
        assert_eq!(fs::read(manifest).unwrap(), before);
        assert_eq!(scan(clients).installations[0].id, entry.id);
    }
    #[test]
    fn malformed_partial_and_missing_installations_do_not_hide_other_games() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        write_manifest(root, 1, "Installed", 4);
        fs::create_dir_all(root.join("steamapps/common/Installed")).unwrap();
        write_manifest(root, 2, "Partial", 1024);
        fs::create_dir_all(root.join("steamapps/common/Partial")).unwrap();
        write_manifest(root, 3, "Missing", 4);
        fs::write(root.join("steamapps/appmanifest_4.acf"), b"AppState {").unwrap();
        let inventory = scan(vec![client(root, "native", ClientKind::Native)]);
        assert_eq!(inventory.installations.len(), 3);
        assert!(!inventory.libraries[0].scan_complete);
        assert!(
            inventory
                .installations
                .iter()
                .find(|i| i.appid == 1)
                .unwrap()
                .manifest_complete
        );
        assert!(
            !inventory
                .installations
                .iter()
                .find(|i| i.appid == 2)
                .unwrap()
                .manifest_complete
        );
        assert!(
            !inventory
                .installations
                .iter()
                .find(|i| i.appid == 3)
                .unwrap()
                .files_present
        );
        assert!(inventory
            .warnings
            .iter()
            .any(|w| w.contains("appmanifest_4.acf")));
    }
    #[test]
    fn offline_libraries_are_retained_as_unavailable_not_reported_empty() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("root");
        let offline = temp.path().join("offline");
        fs::create_dir_all(root.join("steamapps")).unwrap();
        fs::write(
            root.join("steamapps/libraryfolders.vdf"),
            format!(r#"libraryfolders {{ 1 "{}" }}"#, offline.display()),
        )
        .unwrap();
        let inventory = scan(vec![client(&root, "native", ClientKind::Native)]);
        let lib = inventory
            .libraries
            .iter()
            .find(|l| l.path == offline)
            .unwrap();
        assert!(lib.offline);
        assert!(!lib.scan_complete);
        assert!(inventory.warnings.iter().any(|w| w.contains("unavailable")));
    }
    #[test]
    fn conflicting_identity_and_traversal_are_refused() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path();
        for text in [
            r#"AppState { appid 2 name "Game" installdir "Game" StateFlags 4 }"#,
            r#"AppState { appid 1 name "Game" installdir "../escape" StateFlags 4 }"#,
            r#"AppState { appid 1 name "Game" installdir "Game" StateFlags "bad" }"#,
            r#"AppState { appid 1 appid 1 name "Game" installdir "Game" StateFlags 4 }"#,
        ] {
            assert!(manifest(text, path, 1, &[]).is_err());
        }
    }
    #[cfg(unix)]
    #[test]
    fn symlink_library_aliases_are_deduplicated_but_separate_game_copies_keep_ids() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("root");
        let other = temp.path().join("other");
        let alias = temp.path().join("alias");
        fs::create_dir_all(&root).unwrap();
        std::os::unix::fs::symlink(&root, &alias).unwrap();
        let paths = libraries(
            &format!(
                r#"libraryfolders {{ 0 "{}" 1 "{}" }}"#,
                root.display(),
                alias.display()
            ),
            &root,
        )
        .unwrap();
        assert_eq!(paths.len(), 1);
        write_manifest(&root, 1, "Game", 4);
        write_manifest(&other, 1, "Game", 4);
        let a = manifest(
            &read(&root.join("steamapps/appmanifest_1.acf")).unwrap(),
            &root,
            1,
            &[],
        )
        .unwrap();
        let b = manifest(
            &read(&other.join("steamapps/appmanifest_1.acf")).unwrap(),
            &other,
            1,
            &[],
        )
        .unwrap();
        assert_ne!(a.id, b.id);
    }
}
