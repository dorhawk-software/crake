//! Drives the manager with a fake `openconnect` that calls the real hook binary.

#![cfg(target_os = "linux")]

use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

use crake_proto::TunnelState;
use craked::config::{Config, Protocol, TunnelConfig};
use craked::hook_server;
use craked::manager::Manager;

const FAKE: &str = r#"#!/bin/sh
HOOK="$CRAKE_TEST_HOOK"
export TUNDEV=crake-acme INTERNAL_IP4_ADDRESS=10.40.1.7 INTERNAL_IP4_NETMASK=255.255.255.0 \
  INTERNAL_IP4_DNS="10.40.0.2" CISCO_DEF_DOMAIN=acme.internal \
  CISCO_SPLIT_INC=1 CISCO_SPLIT_INC_0_ADDR=10.40.0.0 CISCO_SPLIT_INC_0_MASKLEN=16
trap 'reason=disconnect "$HOOK"; exit 0' TERM
reason=pre-init "$HOOK" || exit 1
reason=connect "$HOOK" || exit 1
while :; do sleep 0.1; done
"#;

async fn wait_for(mgr: &Manager, name: &str, want: TunnelState) {
    for _ in 0..100 {
        if mgr.get(name).unwrap().state == want {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!(
        "{name} never reached {want:?}; is {:?}",
        mgr.get(name).unwrap().state
    );
}

fn setup(dir: &std::path::Path, script: &str) -> Config {
    let fake = dir.join("openconnect");
    std::fs::write(&fake, script).unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    // The fake finds the hook through the environment.
    unsafe { std::env::set_var("CRAKE_TEST_HOOK", env!("CARGO_BIN_EXE_crake-hook")) };
    Config {
        hook_path: env!("CARGO_BIN_EXE_crake-hook").into(),
        hook_socket: dir.join("hook.sock"),
        openconnect: fake,
        log_path: dir.join("log"),
        state_path: dir.join("hosts.json"),
        learned_expiry_days: 7,
        probing: false,
        race_first_syn: false,
        probe_wait_secs: 3,
        never: vec![],
        use_resolved: false,
        apply_network: false,
        managed_dir: dir.join("managed.d"),
        tunnels: vec![TunnelConfig {
            name: "acme".into(),
            display_name: Some("ACME".into()),
            gateway: "vpn.acme.example".into(),
            protocol: Protocol::Anyconnect,
            user: None,
            passwd_file: None,
            dns_domains: vec![],
            auth_args: vec![],
            extra_args: vec![],
            managed: None,
            own: None,
        }],
    }
}

#[tokio::test]
async fn connect_then_disconnect() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = setup(dir.path(), FAKE);
    let socket = cfg.hook_socket.clone();
    let mgr = Manager::new(cfg).unwrap();
    let _srv = hook_server::serve(mgr.clone(), &socket).unwrap();

    assert_eq!(mgr.get("acme").unwrap().state, TunnelState::Off);
    mgr.connect("acme").unwrap();
    assert!(
        mgr.connect("acme").is_err(),
        "second connect must be refused"
    );
    wait_for(&mgr, "acme", TunnelState::Connected).await;

    let snap = mgr.get("acme").unwrap();
    let info = snap.info.unwrap();
    assert_eq!(info.address.unwrap().to_string(), "10.40.1.7");
    assert_eq!(info.prefix, Some(24));
    assert_eq!(info.domains, ["acme.internal"]);
    assert_eq!(info.split_include, ["10.40.0.0/16"]);

    mgr.disconnect("acme").unwrap();
    wait_for(&mgr, "acme", TunnelState::Off).await;
    assert!(mgr.get("acme").unwrap().info.is_none());

    let log = std::fs::read_to_string(dir.path().join("log")).unwrap();
    assert!(log.contains("ACME is up. Routing nothing yet."), "{log}");
    assert!(log.contains("Switched ACME off."), "{log}");
    for line in log.lines() {
        serde_json::from_str::<crake_proto::LogEntry>(line).unwrap();
    }
}

#[tokio::test]
async fn unexpected_exit_is_trouble() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = setup(
        dir.path(),
        "#!/bin/sh\necho 'SSL negotiation failed' >&2\nexit 1\n",
    );
    let socket = cfg.hook_socket.clone();
    let mgr = Manager::new(cfg).unwrap();
    let _srv = hook_server::serve(mgr.clone(), &socket).unwrap();

    mgr.connect("acme").unwrap();
    wait_for(&mgr, "acme", TunnelState::Trouble).await;
    let log = std::fs::read_to_string(dir.path().join("log")).unwrap();
    assert!(log.contains("ACME stopped unexpectedly."), "{log}");
    assert!(log.contains("SSL negotiation failed"), "{log}");
    // Trouble can be retried.
    mgr.connect("acme").unwrap();
}

#[tokio::test]
async fn hook_from_a_stopped_tunnel_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = setup(dir.path(), FAKE);
    let socket = cfg.hook_socket.clone();
    let mgr = Manager::new(cfg).unwrap();
    let _srv = hook_server::serve(mgr.clone(), &socket).unwrap();

    let msg = crake_proto::HookMessage {
        tunnel: "acme".into(),
        reason: "connect".into(),
        env: Default::default(),
    };
    assert!(mgr.handle_hook(msg).await.is_err());
}

const FAKE_COOKIE: &str = r#"#!/bin/sh
case "$*" in *--cookie-on-stdin*--servercert=pin-sha256:abc*" -- https://vpn.acme.example/"*) ;; *) echo "bad args: $*" >&2; exit 2 ;; esac
read -r cookie
[ "$cookie" = "s3cret" ] || { echo "bad cookie" >&2; exit 3; }
trap 'exit 0' TERM
export TUNDEV=crake-acme INTERNAL_IP4_ADDRESS=10.40.1.7
reason=connect "$CRAKE_TEST_HOOK" || exit 1
while :; do sleep 0.1; done
"#;

#[tokio::test]
async fn connects_with_a_signed_in_session() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = setup(dir.path(), FAKE_COOKIE);
    let socket = cfg.hook_socket.clone();
    let mgr = Manager::new(cfg).unwrap();
    let _srv = hook_server::serve(mgr.clone(), &socket).unwrap();

    let session = craked::auth::parse(
        "COOKIE='s3cret'\nCONNECT_URL='https://vpn.acme.example/'\nFINGERPRINT='pin-sha256:abc'\n",
    )
    .unwrap();
    mgr.connect_with("acme", Some(session)).unwrap();
    wait_for(&mgr, "acme", TunnelState::Connected).await;
    mgr.disconnect("acme").unwrap();
    wait_for(&mgr, "acme", TunnelState::Off).await;
    let log = std::fs::read_to_string(dir.path().join("log")).unwrap();
    assert!(!log.contains("s3cret"), "cookie leaked into the log");
}

/// UDP doesn't get through: DTLS never comes up unless told not to try.
const FAKE_NO_UDP: &str = r#"#!/bin/sh
trap 'exit 0' TERM HUP
echo "$*" >> "$CRAKE_TEST_CALLS"
read -r cookie
export TUNDEV=crake-acme INTERNAL_IP4_ADDRESS=10.40.1.7
reason=connect "$CRAKE_TEST_HOOK" || exit 1
case "$*" in *--no-dtls*) ;; *) echo "DTLS handshake timed out" >&2 ;; esac
while :; do sleep 0.1; done
"#;

#[tokio::test]
async fn where_udp_is_blocked_it_carries_on_over_tcp() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = setup(dir.path(), FAKE_NO_UDP);
    let calls = dir.path().join("calls");
    // SAFETY: only this test reads it.
    unsafe { std::env::set_var("CRAKE_TEST_CALLS", &calls) };
    let socket = cfg.hook_socket.clone();
    let mgr = Manager::new(cfg).unwrap();
    let _srv = hook_server::serve(mgr.clone(), &socket).unwrap();

    let session =
        craked::auth::parse("COOKIE='s3cret'\nCONNECT_URL='https://vpn.acme.example/'\n").unwrap();
    mgr.connect_with("acme", Some(session)).unwrap();
    // Up, then restarted without DTLS on the same sign-in.
    for _ in 0..100 {
        let n = std::fs::read_to_string(&calls)
            .unwrap_or_default()
            .lines()
            .count();
        if n >= 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    wait_for(&mgr, "acme", TunnelState::Connected).await;
    let runs = std::fs::read_to_string(&calls).unwrap();
    let runs: Vec<&str> = runs.lines().collect();
    assert_eq!(runs.len(), 2, "{runs:?}");
    assert!(!runs[0].contains("--no-dtls"));
    assert!(runs[1].contains("--no-dtls") && runs[1].contains("--cookie-on-stdin"));
    tokio::time::sleep(Duration::from_millis(300)).await;
    let log = std::fs::read_to_string(dir.path().join("log")).unwrap();
    assert!(
        log.contains("UDP is blocked here. ACME runs over TCP on this network."),
        "{log}"
    );
    assert_eq!(log.matches("ACME is up").count(), 1, "{log}");
    assert!(log.contains("ACME is back."), "{log}");

    // Next time on this network it starts over TCP straight away.
    mgr.disconnect("acme").unwrap();
    wait_for(&mgr, "acme", TunnelState::Off).await;
    if craked::networks::current().is_some() {
        mgr.connect("acme").unwrap();
        wait_for(&mgr, "acme", TunnelState::Connected).await;
        let runs = std::fs::read_to_string(&calls).unwrap();
        assert!(runs.lines().nth(2).unwrap().contains("--no-dtls"), "{runs}");
        mgr.disconnect("acme").unwrap();
        wait_for(&mgr, "acme", TunnelState::Off).await;
    }
}

#[tokio::test]
async fn a_managed_tunnel_keeps_to_its_scope() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = setup(dir.path(), FAKE);
    std::fs::create_dir_all(&cfg.managed_dir).unwrap();
    std::fs::write(
        cfg.managed_dir.join("corp.toml"),
        "display_name = \"Corp\"\ngateway = \"vpn.corp.example\"\nlearn = [\"*.corp.internal\", \"10.40.0.0/16\"]\n",
    )
    .unwrap();
    // The user's own "corp" gives way to IT's.
    let mut own = cfg.tunnels[0].clone();
    own.name = "corp".into();
    cfg.tunnels.push(own);
    cfg.add_managed();
    cfg.validate().unwrap();
    assert_eq!(cfg.tunnels[0].name, "corp", "IT's tunnels come first");
    assert!(cfg.tunnels[0].managed.is_some());
    assert_eq!(cfg.tunnels.iter().filter(|t| t.name == "corp").count(), 1);

    let mgr = Manager::new(cfg).unwrap();
    let refused = mgr.add_host("corp", "10.99.0.1").await.unwrap_err();
    assert_eq!(
        refused.to_string(),
        "Corp IT only allows *.corp.internal and 10.40.0.0/16 through Corp."
    );
    assert!(mgr.pin("corp", "10.0.0.0/8").await.is_err(), "no widening");
    mgr.add_host("corp", "10.40.0.5").await.unwrap();
    mgr.pin("corp", "10.40.8.0/24").await.unwrap();
    // The user's own tunnel learns anything.
    mgr.add_host("acme", "10.99.0.1").await.unwrap();
}

#[tokio::test]
async fn tunnels_added_in_the_app_can_be_changed_and_removed() {
    use std::collections::HashMap;
    let dir = tempfile::tempdir().unwrap();
    let cfg = setup(dir.path(), FAKE);
    let fields = |pairs: &[(&str, &str)]| -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    };
    let mgr = Manager::new(cfg.clone()).unwrap();
    let name = mgr
        .add_tunnel(&fields(&[
            ("display_name", "Home lab"),
            ("gateway", "203.0.113.7"),
            ("protocol", "anyconnect"),
            ("sign_in", "password"),
            ("user", "me"),
        ]))
        .unwrap();
    assert_eq!(name, "home-lab");
    assert_eq!(
        mgr.get("home-lab").unwrap().config.user.as_deref(),
        Some("me")
    );
    assert!(
        mgr.add_tunnel(&fields(&[("display_name", "X")])).is_err(),
        "needs a gateway"
    );

    // Kept across a restart.
    drop(mgr);
    let mgr = Manager::new(cfg).unwrap();
    assert!(mgr.get("home-lab").is_some());
    mgr.update_tunnel(
        "home-lab",
        &fields(&[
            ("display_name", "Lab"),
            ("gateway", "203.0.113.7"),
            ("mode", "full"),
        ]),
    )
    .await
    .unwrap();
    let snap = mgr.get("home-lab").unwrap();
    assert_eq!(snap.config.display(), "Lab");
    assert_eq!(snap.config.learning(), craked::policy::Learning::FullTunnel);

    // The config file's tunnels aren't the app's to change.
    assert!(mgr.remove_tunnel("acme").await.is_err());
    mgr.add_host("home-lab", "10.9.9.9").await.unwrap();
    // Tunnel detail: when and how it was learned.
    let rows = mgr.hosts("home-lab").await.unwrap();
    let row = rows
        .iter()
        .find(|r| r.target.to_string() == "10.9.9.9")
        .unwrap();
    assert_eq!(row.learned.as_ref().map(|l| l.1.as_str()), Some("you"));
    // A name needs the tunnel up, to ask its DNS.
    let off = mgr
        .add_host("home-lab", "wiki.lab.example")
        .await
        .unwrap_err();
    assert!(
        format!("{off:#}").contains("Switch Lab on first"),
        "{off:#}"
    );
    mgr.remove_tunnel("home-lab").await.unwrap();
    assert!(mgr.get("home-lab").is_none());
    assert!(mgr.hosts("home-lab").await.unwrap_or_default().is_empty());
    let saved = std::fs::read_to_string(dir.path().join("tunnels.json")).unwrap();
    assert!(!saved.contains("home-lab"), "{saved}");
    let log = std::fs::read_to_string(dir.path().join("log")).unwrap();
    assert!(
        log.contains("Added Home lab.") && log.contains("Removed Lab."),
        "{log}"
    );
}

/// OpenVPN 3: the app runs the session; craked routes through its device.
#[tokio::test]
async fn an_openvpn3_session_attaches_and_detaches() {
    use std::collections::HashMap;
    let dir = tempfile::tempdir().unwrap();
    let cfg = setup(dir.path(), FAKE);
    let mgr = Manager::new(cfg).unwrap();
    let name = mgr
        .add_tunnel(&HashMap::from([
            ("display_name".to_owned(), "GitLab".to_owned()),
            ("gateway".to_owned(), "initech".to_owned()),
            ("protocol".to_owned(), "openvpn3".to_owned()),
        ]))
        .unwrap();
    assert!(mgr.connect(&name).is_err(), "the app switches it on");
    mgr.attach(
        &name,
        &HashMap::from([
            ("device".to_owned(), "tun0".to_owned()),
            ("dns".to_owned(), "10.8.0.1".to_owned()),
            ("domains".to_owned(), "gitlab.initech.internal".to_owned()),
            ("gateway".to_owned(), "203.0.113.50".to_owned()),
            ("transport".to_owned(), "udp".to_owned()),
        ]),
    )
    .await
    .unwrap();
    let snap = mgr.get(&name).unwrap();
    assert_eq!(snap.state, TunnelState::Connected);
    assert!(snap.since.is_some(), "tunnel detail's \"connected for\"");
    assert_eq!(snap.transport.as_deref(), Some("UDP"));
    // Its DNS answers arrive on tun0: they're this tunnel's, not public.
    assert_eq!(mgr.tunnel_of_device("tun0").as_deref(), Some(name.as_str()));
    assert_eq!(mgr.tunnel_of_device("crake-acme").as_deref(), Some("acme"));
    assert_eq!(mgr.tunnel_of_device("wlp0s20f3"), None);
    let info = snap.info.unwrap();
    assert_eq!(info.device, "tun0");
    assert_eq!(info.domains, ["gitlab.initech.internal"]);
    assert!(
        mgr.attach(
            &name,
            &HashMap::from([("device".to_owned(), "../x".to_owned())])
        )
        .await
        .is_err()
    );
    mgr.detach(&name, None).await.unwrap();
    assert_eq!(mgr.get(&name).unwrap().state, TunnelState::Off);
    let log = std::fs::read_to_string(dir.path().join("log")).unwrap();
    assert!(
        log.contains("GitLab is up.") && log.contains("Switched GitLab off."),
        "{log}"
    );
}

/// Two tunnels that share a DNS server can't both work: the second is
/// refused with a sentence saying which to switch off, and craked warns
/// before it's switched on again, until the first is off.
#[tokio::test]
async fn tunnels_that_clash_are_refused_and_say_why() {
    use std::collections::HashMap;
    let dir = tempfile::tempdir().unwrap();
    let cfg = setup(dir.path(), FAKE);
    let mgr = Manager::new(cfg).unwrap();
    let add = |display: &str, profile: &str| {
        mgr.add_tunnel(&HashMap::from([
            ("display_name".to_owned(), display.to_owned()),
            ("gateway".to_owned(), profile.to_owned()),
            ("protocol".to_owned(), "openvpn3".to_owned()),
        ]))
        .unwrap()
    };
    let (acme, nord) = (add("Initech", "initech"), add("Nordbank", "nordbank"));
    let attach = |device: &str| {
        HashMap::from([
            ("device".to_owned(), device.to_owned()),
            ("dns".to_owned(), "10.0.0.2".to_owned()),
            ("domains".to_owned(), "corp.internal".to_owned()),
        ])
    };
    mgr.attach(&acme, &attach("tun0")).await.unwrap();
    let refused = mgr.attach(&nord, &attach("tun1")).await.unwrap_err();
    assert_eq!(
        format!("{refused:#}"),
        "Turn Initech off to use Nordbank. Both use 10.0.0.2 for DNS."
    );
    assert_eq!(mgr.get(&nord).unwrap().state, TunnelState::Off);
    assert_eq!(mgr.get(&acme).unwrap().state, TunnelState::Connected);
    // Ending the refused session changes nothing and logs nothing more.
    mgr.detach(&nord, None).await.unwrap();
    // Said before switching on again, from what it got last time.
    assert_eq!(
        mgr.get(&nord).unwrap().clash.as_deref(),
        Some("Turn Initech off to use Nordbank. Both use 10.0.0.2 for DNS.")
    );
    mgr.detach(&acme, None).await.unwrap();
    assert_eq!(
        mgr.get(&nord).unwrap().clash,
        None,
        "nothing in the way now"
    );
    mgr.attach(&nord, &attach("tun1")).await.unwrap();
    let log = std::fs::read_to_string(dir.path().join("log")).unwrap();
    assert_eq!(
        log.matches("Turn Initech off to use Nordbank.").count(),
        1,
        "{log}"
    );
    assert!(!log.contains("Switched Nordbank off."), "{log}");
}

/// Two companies using the same private address: it stays with the tunnel
/// that has it, Crake says so once and offers the other tunnel.
#[tokio::test]
async fn an_address_another_tunnel_has_stays_there_and_says_so() {
    use std::collections::HashMap;
    let dir = tempfile::tempdir().unwrap();
    let cfg = setup(dir.path(), FAKE);
    let mgr = Manager::new(cfg).unwrap();
    let add = |display: &str, profile: &str| {
        mgr.add_tunnel(&HashMap::from([
            ("display_name".to_owned(), display.to_owned()),
            ("gateway".to_owned(), profile.to_owned()),
            ("protocol".to_owned(), "openvpn3".to_owned()),
        ]))
        .unwrap()
    };
    let (a, b) = (add("Initech", "initech"), add("Nordbank", "nordbank"));
    let up = |device: &str, dns: &str, domain: &str| {
        HashMap::from([
            ("device".to_owned(), device.to_owned()),
            ("dns".to_owned(), dns.to_owned()),
            ("domains".to_owned(), domain.to_owned()),
        ])
    };
    mgr.attach(&a, &up("tun0", "10.8.0.1", "initech.internal"))
        .await
        .unwrap();
    mgr.attach(&b, &up("tun1", "10.9.0.1", "nordbank.internal"))
        .await
        .unwrap();
    mgr.add_host(&a, "10.0.0.1").await.unwrap();

    // Nordbank's DNS says xyz.nordbank.internal is 10.0.0.1 too.
    let answer = || craked::dns::Answer {
        name: "xyz.nordbank.internal".into(),
        addrs: vec!["10.0.0.1".parse().unwrap()],
    };
    mgr.learn_from_dns(&b, answer()).await;
    mgr.learn_from_dns(&b, answer()).await;
    assert!(sends(&mgr, &a, "10.0.0.1").await, "it stays with Initech");
    assert!(
        !sends(&mgr, &b, "10.0.0.1").await,
        "Nordbank doesn't take it as well"
    );
    let log = std::fs::read_to_string(dir.path().join("log")).unwrap();
    assert_eq!(
        log.matches("xyz.nordbank.internal can't go through Nordbank.")
            .count(),
        1,
        "said once, not on every lookup: {log}"
    );
    assert!(log.contains("already goes through Initech"), "{log}");
    // Adding it by hand is refused the same way.
    let refused = mgr.add_host(&b, "10.0.0.1").await.unwrap_err();
    assert!(format!("{refused:#}").contains("already goes through Initech"));
    // Moving it is the user's choice.
    mgr.move_hosts(&a, &b, &["10.0.0.1"]).await.unwrap();
    assert!(sends(&mgr, &b, "10.0.0.1").await && !sends(&mgr, &a, "10.0.0.1").await);

    // The same for an IPv6 address both companies use.
    mgr.add_host(&a, "fd00::1").await.unwrap();
    mgr.learn_from_dns(
        &b,
        craked::dns::Answer {
            name: "v6.nordbank.internal".into(),
            addrs: vec!["fd00::1".parse().unwrap()],
        },
    )
    .await;
    assert!(sends(&mgr, &a, "fd00::1").await && !sends(&mgr, &b, "fd00::1").await);
    let log = std::fs::read_to_string(dir.path().join("log")).unwrap();
    assert!(
        log.contains("v6.nordbank.internal can't go through Nordbank."),
        "{log}"
    );
}

/// The tunnel sends this host through itself (learned or pinned).
async fn sends(mgr: &Manager, tunnel: &str, host: &str) -> bool {
    mgr.hosts(tunnel)
        .await
        .unwrap()
        .iter()
        .any(|r| r.target.to_string() == host && !r.direct)
}

/// A tunnel's DNS that stops answering (7h): said once, known names keep
/// working from memory, unknown ones fail honestly, and it's said again
/// when the DNS is back.
#[tokio::test]
async fn known_names_keep_working_while_a_tunnels_dns_is_silent() {
    use hickory_proto::op::{Message, ResponseCode};
    use hickory_proto::rr::RecordType;
    use std::collections::HashMap;
    let dir = tempfile::tempdir().unwrap();
    let cfg = setup(dir.path(), FAKE);
    let mgr = Manager::new(cfg).unwrap();
    let t = mgr
        .add_tunnel(&HashMap::from([
            ("display_name".to_owned(), "Initech".to_owned()),
            ("gateway".to_owned(), "initech".to_owned()),
            ("protocol".to_owned(), "openvpn3".to_owned()),
        ]))
        .unwrap();
    mgr.attach(
        &t,
        &HashMap::from([
            ("device".to_owned(), "tun0".to_owned()),
            ("dns".to_owned(), "10.8.0.1".to_owned()),
            ("domains".to_owned(), "initech.internal".to_owned()),
        ]),
    )
    .await
    .unwrap();
    // Its DNS said where GitLab is, while it still answered: an A answer
    // and an AAAA answer (each keeps the other), and Docs only by IPv4.
    for (name, addr) in [
        ("gitlab.initech.internal", "10.81.1.4"),
        ("gitlab.initech.internal", "fd81:1::4"),
        ("docs.initech.internal", "10.81.1.5"),
    ] {
        mgr.learn_from_dns(
            &t,
            craked::dns::Answer {
                name: name.into(),
                addrs: vec![addr.parse().unwrap()],
            },
        )
        .await;
    }

    mgr.dns_checked(&t, false).await;
    mgr.dns_checked(&t, false).await;
    assert!(mgr.get(&t).unwrap().dns_down);
    let ask = |name: &str, qtype| craked::dns::query(7, name, qtype);
    let reply = |bytes: Option<Vec<u8>>| Message::from_vec(&bytes.unwrap()).unwrap();

    let known = reply(
        mgr.keep_answer(&ask("gitlab.initech.internal", RecordType::A))
            .await,
    );
    assert_eq!(known.metadata.response_code, ResponseCode::NoError);
    assert_eq!(
        craked::dns::a_records(&known),
        vec!["10.81.1.4".parse::<std::net::Ipv4Addr>().unwrap()]
    );
    let again = reply(
        mgr.keep_answer(&ask("gitlab.initech.internal", RecordType::A))
            .await,
    );
    assert_eq!(craked::dns::a_records(&again).len(), 1);
    let v6 = reply(
        mgr.keep_answer(&ask("gitlab.initech.internal", RecordType::AAAA))
            .await,
    );
    assert_eq!(
        craked::dns::records(&v6),
        vec!["fd81:1::4".parse::<std::net::IpAddr>().unwrap()],
        "its IPv6 address, remembered too"
    );
    let no_v6 = reply(
        mgr.keep_answer(&ask("docs.initech.internal", RecordType::AAAA))
            .await,
    );
    assert_eq!(no_v6.metadata.response_code, ResponseCode::NoError);
    assert!(
        no_v6.answers.is_empty(),
        "it exists, just no IPv6 we know of"
    );
    let unknown = reply(
        mgr.keep_answer(&ask("wiki.initech.internal", RecordType::A))
            .await,
    );
    assert_eq!(unknown.metadata.response_code, ResponseCode::ServFail);
    assert!(sends(&mgr, &t, "10.81.1.4").await, "routed through Initech");

    mgr.dns_checked(&t, true).await;
    assert!(!mgr.get(&t).unwrap().dns_down);
    let log = std::fs::read_to_string(dir.path().join("log")).unwrap();
    assert_eq!(
        log.matches("Initech's DNS (10.8.0.1) doesn't answer.")
            .count(),
        1,
        "{log}"
    );
    assert_eq!(
        log.matches("Using the address gitlab.initech.internal had last time.")
            .count(),
        1,
        "said once per name: {log}"
    );
    assert!(log.contains("Initech's DNS answers again."), "{log}");
}

/// A host learned under a name before answers were remembered (an older
/// craked, or a name not looked up since) still works while the DNS is silent.
#[tokio::test]
async fn hosts_learned_under_a_name_are_answered_too() {
    use hickory_proto::op::{Message, ResponseCode};
    use hickory_proto::rr::RecordType;
    use std::collections::HashMap;
    let dir = tempfile::tempdir().unwrap();
    let cfg = setup(dir.path(), FAKE);
    let until = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 86_400;
    std::fs::write(
        dir.path().join("hosts.json"),
        format!(
            r#"{{"initech": {{"learned": {{"10.81.1.9": {until}}}, "names": {{"10.81.1.9": "jira.initech.internal"}}}}}}"#
        ),
    )
    .unwrap();
    let mgr = Manager::new(cfg).unwrap();
    let t = mgr
        .add_tunnel(&HashMap::from([
            ("display_name".to_owned(), "Initech".to_owned()),
            ("gateway".to_owned(), "initech".to_owned()),
            ("protocol".to_owned(), "openvpn3".to_owned()),
        ]))
        .unwrap();
    assert_eq!(t, "initech");
    mgr.attach(
        &t,
        &HashMap::from([
            ("device".to_owned(), "tun0".to_owned()),
            ("dns".to_owned(), "10.8.0.1".to_owned()),
            ("domains".to_owned(), "initech.internal".to_owned()),
        ]),
    )
    .await
    .unwrap();
    mgr.dns_checked(&t, false).await;
    let q = craked::dns::query(9, "jira.initech.internal", RecordType::A);
    let msg = Message::from_vec(&mgr.keep_answer(&q).await.unwrap()).unwrap();
    assert_eq!(msg.metadata.response_code, ResponseCode::NoError);
    assert_eq!(
        craked::dns::a_records(&msg),
        vec!["10.81.1.9".parse::<std::net::Ipv4Addr>().unwrap()]
    );
}

/// Records each run (the cookie it was given) and how it was stopped.
// The traps come first: craked may stop it the moment the hook says
// it's connected.
const FAKE_HANDOVER: &str = r#"#!/bin/sh
trap 'echo hup >> "$CRAKE_TEST_HANDOVER"; exit 0' HUP
trap 'echo term >> "$CRAKE_TEST_HANDOVER"; exit 0' TERM
read -r cookie
echo "run cookie=$cookie" >> "$CRAKE_TEST_HANDOVER"
export TUNDEV=crake-acme INTERNAL_IP4_ADDRESS=10.40.1.7
reason=connect "$CRAKE_TEST_HOOK" || exit 1
while :; do sleep 0.1; done
"#;

/// A package upgrade: the old craked hands its tunnels to the new one
/// (handover.rs). They stay signed in and come back without a sign-in; the
/// log says so once, instead of "Switched off" and "Switched on".
#[tokio::test]
async fn an_upgrade_hands_the_tunnels_to_the_new_craked() {
    use std::collections::HashMap;
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let cfg = setup(dir.path(), FAKE_HANDOVER);
    let calls = dir.path().join("calls");
    // SAFETY: only this test reads it.
    unsafe { std::env::set_var("CRAKE_TEST_HANDOVER", &calls) };
    let socket = cfg.hook_socket.clone();
    let log = || std::fs::read_to_string(dir.path().join("log")).unwrap();

    let old = Manager::new(cfg.clone()).unwrap();
    old.init().await.unwrap();
    let srv = hook_server::serve(old.clone(), &socket).unwrap();
    let session =
        craked::auth::parse("COOKIE='s3cret'\nCONNECT_URL='https://vpn.acme.example/'\n").unwrap();
    old.connect_with("acme", Some(session)).unwrap();
    wait_for(&old, "acme", TunnelState::Connected).await;
    let initech = old
        .add_tunnel(&HashMap::from([
            ("display_name".to_owned(), "Initech".to_owned()),
            ("gateway".to_owned(), "initech".to_owned()),
            ("protocol".to_owned(), "openvpn3".to_owned()),
        ]))
        .unwrap();
    let tun0 = HashMap::from([
        ("device".to_owned(), "tun0".to_owned()),
        ("dns".to_owned(), "10.8.0.1".to_owned()),
    ]);
    old.attach(&initech, &tun0).await.unwrap();

    // postinst's flag, then the restart.
    std::fs::write(dir.path().join("handover"), "").unwrap();
    old.shutdown().await;
    srv.abort();
    let resume = dir.path().join("resume.json");
    let mode = std::fs::metadata(&resume).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600, "the sessions are root's alone");
    assert!(!dir.path().join("handover").exists(), "the flag is taken");
    let runs = std::fs::read_to_string(&calls).unwrap();
    assert!(
        runs.contains("hup") && !runs.contains("term"),
        "kept signed in: {runs}"
    );

    let new = Manager::new(cfg).unwrap();
    new.init().await.unwrap();
    let _srv = hook_server::serve(new.clone(), &socket).unwrap();
    new.resume_after_update().await;
    wait_for(&new, "acme", TunnelState::Connected).await;
    wait_for(&new, &initech, TunnelState::Connected).await;
    assert!(!resume.exists(), "read once");
    let runs = std::fs::read_to_string(&calls).unwrap();
    assert_eq!(runs.matches("run cookie=s3cret").count(), 2, "{runs}");
    let said = log();
    assert!(
        said.contains("Crake was updated. Your tunnels are reconnecting with your sign-ins."),
        "{said}"
    );
    assert_eq!(said.matches("Switched ACME on.").count(), 1, "{said}");
    assert!(!said.contains("Switched ACME off."), "{said}");
    assert!(!said.contains("s3cret"), "the cookie stays out of the log");

    // An ordinary stop (no flag) logs off, as always, and leaves nothing.
    new.shutdown().await;
    // The fake openconnect writes "term" as it goes: give it a moment (on a
    // busy machine shutdown can return first).
    let mut termed = false;
    for _ in 0..50 {
        termed = std::fs::read_to_string(&calls).unwrap().contains("term");
        if termed {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(termed, "openconnect was told to log off");
    assert!(!resume.exists());
    assert!(log().contains("Switched ACME off."));
}
