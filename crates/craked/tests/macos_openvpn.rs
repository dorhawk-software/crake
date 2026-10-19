//! OpenVPN on macOS (CLAUDE.md, 7i, slice 5), as root on CI's Mac: craked
//! runs Homebrew's openvpn against an openvpn server on this same Mac
//! (loopback; the server says `dev tun`, which macOS' openvpn makes a
//! utun of: it refuses `--mode server` with `dev utun`), signing in with a
//! certificate: the utun the hook names gets
//! the tunnel's address and its DNS route, the tunnel comes up, and goes
//! with its device. Not the data path (a server on the same Mac answers its
//! own addresses directly): that's 7j's, with a gateway on another machine.
//! Skipped unless CRAKE_MACOS_ROOT=1.
#![cfg(target_os = "macos")]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crake_proto::TunnelState;
use craked::config::Config;
use craked::hook_server;
use craked::manager::Manager;

fn enabled() -> bool {
    if std::env::var("CRAKE_MACOS_ROOT").as_deref() == Ok("1") {
        return true;
    }
    eprintln!("skipped: needs root (CRAKE_MACOS_ROOT=1)");
    false
}

/// Homebrew's openvpn (Apple Silicon's, then Intel's).
fn openvpn() -> PathBuf {
    ["/opt/homebrew/sbin/openvpn", "/usr/local/sbin/openvpn"]
        .iter()
        .map(PathBuf::from)
        .find(|p| p.exists())
        .expect("openvpn from Homebrew (brew install openvpn)")
}

fn sh(cmd: &str, dir: &Path) -> String {
    let out = Command::new("sh")
        .args(["-c", cmd])
        .current_dir(dir)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr)
}

/// A CA, the server's certificate and the client's, in `dir`.
fn pki(dir: &Path) {
    sh(
        "openssl req -x509 -newkey rsa:2048 -nodes -keyout ca.key -out ca.crt -days 2 -subj /CN=Test-CA && \
         for n in server client; do \
           openssl req -newkey rsa:2048 -nodes -keyout $n.key -out $n.csr -subj /CN=$n && \
           if [ $n = server ]; then eku=serverAuth; else eku=clientAuth; fi && \
           printf \"extendedKeyUsage=$eku\\nkeyUsage=digitalSignature,keyEncipherment\\n\" > $n.ext && \
           openssl x509 -req -in $n.csr -CA ca.crt -CAkey ca.key -CAcreateserial -out $n.crt -days 2 -extfile $n.ext; \
         done",
        dir,
    );
    assert!(dir.join("client.crt").exists(), "made the certificates");
}

const SERVER: &str = "mode server\ntls-server\ndev tun\ntopology subnet\nproto udp\n\
    local 127.0.0.1\nport 11940\nifconfig 10.88.0.1 255.255.255.0\nifconfig-pool 10.88.0.10 10.88.0.50\n\
    push \"topology subnet\"\npush \"route-gateway 10.88.0.1\"\npush \"route 10.89.0.0 255.255.0.0\"\n\
    push \"dhcp-option DNS 10.88.0.53\"\npush \"dhcp-option DOMAIN mac.internal\"\n\
    ca ca.crt\ncert server.crt\nkey server.key\ndh none\nverify-client-cert require\nverb 3\n";

async fn until(mgr: &Manager, name: &str, want: TunnelState, dir: &Path) {
    for _ in 0..300 {
        let got = mgr.get(name).unwrap();
        assert!(got.asking.is_none(), "a certificate asks nothing");
        if got.state == want {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let server = std::fs::read_to_string(dir.join("server.log")).unwrap_or_default();
    let got = mgr.get(name).unwrap();
    panic!(
        "never {want:?} (it's {:?}: {:?})\n--- the server said:\n{server}",
        got.state, got.why
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_openvpn_tunnel_on_this_mac() {
    if !enabled() {
        return;
    }
    // craked's own lines (openvpn's output and its management dialogue
    // among them), shown when the test fails.
    let _ = tracing_subscriber::fmt()
        .with_env_filter("debug")
        .with_test_writer()
        .try_init();
    let dir = tempfile::tempdir().unwrap();
    pki(dir.path());
    std::fs::write(dir.path().join("server.conf"), SERVER).unwrap();
    let mut server = Command::new(openvpn())
        .args(["--config", "server.conf"])
        .current_dir(dir.path())
        .stdout(std::fs::File::create(dir.path().join("server.log")).unwrap())
        .stderr(std::fs::File::create(dir.path().join("server.err")).unwrap())
        .spawn()
        .unwrap();
    let read = |f: &str| std::fs::read_to_string(dir.path().join(f)).unwrap();
    let profile = format!(
        "client\ndev tun\nproto udp\nremote 127.0.0.1 11940\nnobind\nremote-cert-tls server\n\
         <ca>\n{}</ca>\n<cert>\n{}</cert>\n<key>\n{}</key>\n",
        read("ca.crt"),
        read("client.crt"),
        read("client.key")
    );
    let cfg = Config {
        hook_path: env!("CARGO_BIN_EXE_crake-hook").into(),
        hook_socket: dir.path().join("hook.sock"),
        openvpn: openvpn(),
        charon: "charon".into(),
        log_path: dir.path().join("log"),
        state_path: dir.path().join("hosts.json"),
        probing: false,
        race_first_syn: false,
        use_resolved: false,
        managed_dir: dir.path().join("managed.d"),
        ..Config::default()
    };
    let socket = cfg.hook_socket.clone();
    let mgr = Manager::new(cfg).unwrap();
    mgr.init().await.unwrap();
    let _hooks = hook_server::serve(mgr.clone(), &socket).unwrap();
    let fields: HashMap<String, String> = [
        ("display_name", "Initech"),
        ("protocol", "openvpn"),
        ("profile", profile.as_str()),
    ]
    .iter()
    .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
    .collect();
    let name = mgr.add_tunnel(&fields).unwrap();
    mgr.connect(&name).unwrap();
    until(&mgr, &name, TunnelState::Connected, dir.path()).await;

    let info = mgr.get(&name).unwrap().info.unwrap();
    assert!(
        info.device.starts_with("utun"),
        "the hook named its utun: {info:?}"
    );
    let address = info.address.unwrap().to_string();
    assert!(address.starts_with("10.88.0."), "{info:?}");
    let shown = sh(&format!("ifconfig {}", info.device), dir.path());
    assert!(shown.contains(&format!("inet {address}")), "{shown}");
    let dns = sh("route -n get 10.88.0.53", dir.path());
    assert!(
        dns.contains(&format!("interface: {}", info.device)),
        "its DNS through it: {dns}"
    );
    assert_eq!(info.domains, ["mac.internal"]);
    assert_eq!(info.split_include, ["10.89.0.0/16"]);

    mgr.disconnect(&name).unwrap();
    until(&mgr, &name, TunnelState::Off, dir.path()).await;
    let gone = sh(&format!("ifconfig {}", info.device), dir.path());
    assert!(gone.contains("does not exist"), "its utun went: {gone}");
    mgr.shutdown().await;
    let _ = server.kill();
    let _ = server.wait();
}
