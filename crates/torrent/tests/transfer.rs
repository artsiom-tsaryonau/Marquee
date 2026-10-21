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
    assert!(a.progress(seed_id).unwrap().uploaded > 0, "seeder uploaded nothing");

    a.shutdown();
    b.shutdown();
}
