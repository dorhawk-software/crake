//! A manager with a fake `openconnect` (a script that calls the real hook
//! binary), for the lifecycle tests.

use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

use crake_proto::TunnelState;
use craked::config::{Config, Protocol, TunnelConfig};
use craked::manager::Manager;

use crate::script::scripted;

pub async fn wait_for(mgr: &Manager, name: &str, want: TunnelState) {
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

pub fn setup(dir: &std::path::Path, script: &str) -> Config {
    let fake = dir.join("openconnect");
    std::fs::write(&fake, scripted(script, dir)).unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    Config {
        hook_path: env!("CARGO_BIN_EXE_crake-hook").into(),
        hook_socket: dir.join("hook.sock"),
        openconnect: fake,
        openvpn: "openvpn".into(),
        charon: "charon".into(),
        charon_unit: None,
        log_path: dir.join("log"),
        state_path: dir.join("hosts.json"),
        learned_expiry_days: 7,
        probing: false,
        race_first_syn: false,
        probe_wait_secs: 3,
        dns_check_secs: craked::config::DNS_CHECK_SECS,
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
            profile: None,
            gateway_key_file: None,
            gateway_id: None,
            managed: None,
            own: None,
        }],
    }
}
