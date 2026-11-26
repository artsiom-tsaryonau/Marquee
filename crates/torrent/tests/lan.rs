//! The transport never enables public discovery or accepts non-local peers.
use marquee_torrent::{check_local_peer, Config, Engine, Source};

#[test]
fn only_private_link_local_and_loopback_peers_are_accepted() {
    for address in [
        "127.0.0.1:1",
        "10.1.2.3:51413",
        "172.16.0.1:2",
        "172.31.255.255:2",
        "192.168.1.20:3",
        "169.254.10.20:4",
        "[::1]:5",
        "[fd12::1]:6",
        "[fe80::1%2]:7",
        "[::ffff:192.168.1.20]:8",
    ] {
        assert!(
            check_local_peer(address.parse().unwrap()).is_ok(),
            "{address}"
        );
    }
    for address in [
        "0.0.0.0:1",
        "192.168.1.20:0",
        "8.8.8.8:51413",
        "172.15.255.255:1",
        "172.32.0.1:1",
        "192.169.1.1:1",
        "100.64.0.1:1",
        "224.0.0.1:1",
        "255.255.255.255:1",
        "[::]:1",
        "[2001:4860::1]:1",
        "[ff02::1]:1",
        "[::ffff:8.8.8.8]:1",
    ] {
        assert!(
            check_local_peer(address.parse().unwrap()).is_err(),
            "{address}"
        );
    }
}

#[test]
fn public_settings_and_listeners_are_rejected_before_creating_data() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("no-network");
    for mode in 0..5 {
        let mut cfg = Config::private(&path);
        match mode {
            0 => cfg.dht = true,
            1 => cfg.trackers = true,
            2 => cfg.proxy = Some("socks5://127.0.0.1:1080".into()),
            3 => cfg.listen = "8.8.8.8:51413".parse().unwrap(),
            _ => cfg.listen = "0.0.0.0:51413".parse().unwrap(),
        }
        assert!(Engine::start(&cfg).is_err());
        assert!(!path.exists());
    }
}

#[test]
fn urls_public_magnets_and_mixed_peer_lists_do_not_create_downloads() {
    let temp = tempfile::tempdir().unwrap();
    let mut cfg = Config::private(temp.path().join("engine"));
    cfg.listen = "127.0.0.1:0".parse().unwrap();
    let engine = Engine::start(&cfg).unwrap();
    let output = temp.path().join("must-not-exist");
    let hash = "4927b4f8762d3727d8be7d48e754485c7f091436";
    for link in [
        "https://example.invalid/game.torrent".into(),
        format!("magnet:?xt=urn:btih:{hash}&tr=http://example.invalid/announce"),
        format!("magnet:?xt=urn:btih:{hash}&x.pe=8.8.8.8:51413"),
    ] {
        assert!(engine
            .download(Source::Link(link), &output, vec![])
            .is_err());
        assert!(!output.exists());
    }
    assert!(engine
        .download(
            Source::Link(format!("magnet:?xt=urn:btih:{hash}")),
            &output,
            vec!["127.0.0.1:1".parse().unwrap(), "8.8.8.8:1".parse().unwrap()]
        )
        .is_err());
    assert!(!output.exists());
    engine.shutdown();
}
