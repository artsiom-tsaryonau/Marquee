//! Two engines on loopback: one seeds a file, the other downloads it by a
//! `.torrent` made on the seeder, with DHT and trackers off.

use std::time::Duration;

use marquee_torrent::{Config, Engine, Source, State};

fn sample(path: &std::path::Path, mib: usize) {
    use std::io::Write;
    let mut f = std::fs::File::create(path).unwrap();
    let mut x: u32 = 0x1234_5678;
    let mut buf = vec![0u8; 1 << 20];
    for _ in 0..mib {
        for b in buf.iter_mut() {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            *b = x as u8;
        }
        f.write_all(&buf).unwrap();
    }
}

fn local(dir: &std::path::Path) -> Config {
    let mut c = Config::private(dir);
    c.listen = "127.0.0.1:0".parse().unwrap();
    c
}

#[test]
fn private_transfer_between_two_engines() {
    let tmp = tempfile::tempdir().unwrap();
    let a_dir = tmp.path().join("a");
    let b_dir = tmp.path().join("b");
    std::fs::create_dir_all(&a_dir).unwrap();
    let file = a_dir.join("Test Game (USA).iso");
    sample(&file, 12);

    let a = Engine::start(&local(&a_dir)).unwrap();
    let created = a.create(&file, vec![]).unwrap();
    assert_eq!(created.info_hash.len(), 40);
    let seed_id = a.seed(created.bytes.clone(), &a_dir).unwrap();
    a.wait_finished(seed_id, Duration::from_secs(60)).unwrap();
    assert_eq!(a.progress(seed_id).unwrap().state, State::Seeding);

    let b = Engine::start(&local(&b_dir)).unwrap();
    let peer = a.listen_addr().expect("seeder listens");
    let id = b
        .download(Source::TorrentBytes(created.bytes), &b_dir, vec![peer])
        .unwrap();
    b.wait_finished(id, Duration::from_secs(120)).unwrap();

    let got = std::fs::read(b_dir.join("Test Game (USA).iso")).unwrap();
    let want = std::fs::read(&file).unwrap();
    assert_eq!(got.len(), want.len());
    assert!(got == want, "downloaded bytes differ");
    assert!(
        a.progress(seed_id).unwrap().uploaded > 0,
        "seeder uploaded nothing"
    );

    a.shutdown();
    b.shutdown();
}

#[test]
fn selected_file_verification_checks_neighboring_piece_bytes_and_rejects_changes() {
    let tmp = tempfile::tempdir().unwrap();
    let bundle = tmp.path().join("bundle");
    std::fs::create_dir(&bundle).unwrap();
    for (name, bytes) in [
        ("a.bin", vec![1; 73]),
        ("game.zip", vec![2; 512]),
        ("z.bin", vec![3; 89]),
    ] {
        std::fs::write(bundle.join(name), bytes).unwrap();
    }
    let engine = Engine::start(&local(tmp.path())).unwrap();
    let made = engine.create(&bundle, vec![]).unwrap();
    let manifest = marquee_torrent::inspect(&made.bytes).unwrap();
    let selected = &manifest
        .files
        .iter()
        .find(|f| f.path.ends_with("game.zip"))
        .unwrap()
        .path;
    let base = if tmp.path().join(selected).exists() {
        tmp.path()
    } else {
        &bundle
    };
    let mut opened = Vec::new();
    marquee_torrent::verify_file(&made.bytes, selected, |path, _| {
        opened.push(path.to_string());
        Ok(std::fs::File::open(base.join(path))?)
    })
    .unwrap();
    assert_eq!(
        opened.len(),
        3,
        "all three files intersect the selected file's boundary piece"
    );
    std::fs::write(bundle.join("a.bin"), vec![4; 73]).unwrap();
    assert!(
        marquee_torrent::verify_file(&made.bytes, selected, |path, _| Ok(std::fs::File::open(
            base.join(path)
        )?))
        .unwrap_err()
        .to_string()
        .contains("piece")
    );
    assert!(
        marquee_torrent::verify_file(&made.bytes, "../game.zip", |path, _| Ok(
            std::fs::File::open(base.join(path))?
        ))
        .is_err()
    );
    engine.shutdown();
}

#[test]
fn folder_seed_uses_its_top_directory_and_missing_sources_never_create_files() {
    let temp = tempfile::tempdir().unwrap();
    let folder = temp.path().join("Bundle");
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join("one.zip"), b"first synthetic source").unwrap();
    std::fs::write(folder.join("two.pkg"), b"second synthetic source").unwrap();
    let a = Engine::start(&local(temp.path())).unwrap();
    let made = a.create(&folder, vec![]).unwrap();
    let id = a.seed(made.bytes.clone(), temp.path()).unwrap();
    a.wait_finished(id, Duration::from_secs(5)).unwrap();
    assert!(!temp.path().join("one.zip").exists());
    let destination = temp.path().join("download");
    let b = Engine::start(&local(&destination)).unwrap();
    let id = b
        .download_owned(
            Source::TorrentBytes(made.bytes.clone()),
            &destination,
            vec![a.listen_addr().unwrap()],
        )
        .unwrap();
    b.wait_finished(id, Duration::from_secs(10)).unwrap();
    assert_eq!(
        std::fs::read(destination.join("one.zip")).unwrap(),
        b"first synthetic source"
    );
    let missing = temp.path().join("missing");
    std::fs::create_dir(&missing).unwrap();
    assert!(b.seed(made.bytes, &missing).is_err());
    assert!(std::fs::read_dir(missing).unwrap().next().is_none());
    a.shutdown();
    b.shutdown();
}
