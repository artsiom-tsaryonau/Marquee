//! A proxied engine reaches peers only through its SOCKS5 proxy, has no listener, and downloads
//! nothing when the proxy is gone (a VPN container that is down).

use std::io::{Read, Write};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use marquee_torrent::{check_proxy, Config, Engine, Source};

fn sample(path: &std::path::Path, mib: usize) {
    let mut f = std::fs::File::create(path).unwrap();
    let mut x: u32 = 0x9e37_79b9;
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

/// Minimal SOCKS5 server (no auth, CONNECT only) counting the connections it relays.
fn socks5() -> (SocketAddr, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let relayed = Arc::new(AtomicUsize::new(0));
    let count = relayed.clone();
    std::thread::spawn(move || {
        for client in listener.incoming().flatten() {
            let count = count.clone();
            std::thread::spawn(move || {
                let _ = relay(client, &count);
            });
        }
    });
    (addr, relayed)
}

fn relay(mut client: TcpStream, count: &AtomicUsize) -> std::io::Result<()> {
    let mut head = [0u8; 2];
    client.read_exact(&mut head)?;
    let mut methods = vec![0u8; head[1] as usize];
    client.read_exact(&mut methods)?;
    client.write_all(&[5, 0])?;
    let mut req = [0u8; 4];
    client.read_exact(&mut req)?;
    let ip: std::net::IpAddr = match req[3] {
        1 => {
            let mut b = [0u8; 4];
            client.read_exact(&mut b)?;
            Ipv4Addr::from(b).into()
        }
        4 => {
            let mut b = [0u8; 16];
            client.read_exact(&mut b)?;
            Ipv6Addr::from(b).into()
        }
        _ => return Ok(()),
    };
    let mut port = [0u8; 2];
    client.read_exact(&mut port)?;
    let target = TcpStream::connect(SocketAddr::new(ip, u16::from_be_bytes(port)))?;
    count.fetch_add(1, Ordering::SeqCst);
    client.write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0])?;
    let (mut c2, mut t2) = (client.try_clone()?, target.try_clone()?);
    std::thread::spawn(move || {
        let _ = std::io::copy(&mut c2, &mut t2);
    });
    let (mut c, mut t) = (client, target);
    let _ = std::io::copy(&mut t, &mut c);
    Ok(())
}

fn seeder(dir: &std::path::Path) -> (Engine, Vec<u8>, std::path::PathBuf) {
    std::fs::create_dir_all(dir).unwrap();
    let file = dir.join("Proxy Test (USA).iso");
    sample(&file, 4);
    let a = Engine::start(&local(dir)).unwrap();
    let created = a.create(&file, vec![]).unwrap();
    let id = a.seed(created.bytes.clone(), dir).unwrap();
    a.wait_finished(id, Duration::from_secs(60)).unwrap();
    (a, created.bytes, file)
}

#[test]
fn proxied_engine_downloads_only_through_the_proxy() {
    let tmp = tempfile::tempdir().unwrap();
    let (a, torrent, file) = seeder(&tmp.path().join("a"));
    let (proxy, relayed) = socks5();

    let b_dir = tmp.path().join("b");
    let mut cfg = local(&b_dir);
    cfg.proxy = Some(format!("socks5://{proxy}"));
    let b = Engine::start(&cfg).unwrap();
    assert!(
        b.listen_addr().is_none(),
        "a proxied engine must not accept inbound peers"
    );

    let id = b
        .download(
            Source::TorrentBytes(torrent),
            &b_dir,
            vec![a.listen_addr().unwrap()],
        )
        .unwrap();
    b.wait_finished(id, Duration::from_secs(120)).unwrap();
    assert_eq!(
        std::fs::read(b_dir.join("Proxy Test (USA).iso")).unwrap(),
        std::fs::read(&file).unwrap()
    );
    assert!(
        relayed.load(Ordering::SeqCst) > 0,
        "the peer was not reached through the proxy"
    );
    a.shutdown();
    b.shutdown();
}

#[test]
fn proxied_engine_fails_closed_when_the_proxy_is_down() {
    let tmp = tempfile::tempdir().unwrap();
    let (a, torrent, _file) = seeder(&tmp.path().join("a"));
    // A port with nothing listening: the VPN container is stopped.
    let closed = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();

    let b_dir = tmp.path().join("b");
    let mut cfg = local(&b_dir);
    cfg.proxy = Some(format!("socks5://{closed}"));
    let b = Engine::start(&cfg).unwrap();
    let id = b
        .download(
            Source::TorrentBytes(torrent),
            &b_dir,
            vec![a.listen_addr().unwrap()],
        )
        .unwrap();
    assert!(
        b.wait_finished(id, Duration::from_secs(8)).is_err(),
        "downloaded without the proxy"
    );
    assert_eq!(b.progress(id).unwrap().downloaded, 0);
    a.shutdown();
    b.shutdown();
}

#[test]
fn only_loopback_socks5_proxies_are_accepted() {
    assert!(check_proxy("socks5://127.0.0.1:1080").is_ok());
    assert!(check_proxy("socks5://localhost:1080").is_ok());
    assert!(check_proxy("socks5://[::1]:1080").is_ok());
    for bad in [
        "socks5://10.0.0.5:1080",
        "socks5://vpn.example.com:1080",
        "http://127.0.0.1:1080",
        "socks4://127.0.0.1:1080",
        "socks5://127.0.0.1",
        "not a url",
    ] {
        assert!(check_proxy(bad).is_err(), "{bad} should be rejected");
    }
}
