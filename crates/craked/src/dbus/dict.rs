//! A tunnel's snapshot as the dictionary `ListTunnels` and `GetTunnel` give.

use std::collections::HashMap;

use zbus::{OwnedVariant, Variant};

use crate::manager::Snapshot;

pub(super) fn to_dict(s: &Snapshot) -> HashMap<String, OwnedVariant> {
    let info = s.info.as_ref();
    let strings = |v: Vec<String>| OwnedVariant::try_from(Variant::from(v)).unwrap();
    let text = |v: &str| OwnedVariant::try_from(Variant::from(v)).unwrap();
    let mut d = HashMap::new();
    d.insert("name".into(), text(&s.config.name));
    d.insert("display_name".into(), text(s.config.display()));
    d.insert("state".into(), text(s.state.as_str()));
    d.insert("interface".into(), text(&s.interface));
    d.insert("gateway".into(), text(&s.config.gateway));
    d.insert("protocol".into(), text(s.config.protocol.as_str()));
    d.insert("hosts".into(), OwnedVariant::from(s.hosts as u32));
    d.insert("signed_in".into(), OwnedVariant::from(s.signed_in));
    d.insert("user".into(), text(s.config.user.as_deref().unwrap_or("")));
    // What sign-in needs, so the user's session can run it.
    d.insert("auth_args".into(), strings(s.config.auth_args.clone()));
    d.insert("extra_args".into(), strings(s.config.extra_args.clone()));
    // Who decides (Managed profiles): the panel says it in plain words.
    let managed = s.config.managed.as_ref();
    d.insert("managed".into(), OwnedVariant::from(managed.is_some()));
    d.insert(
        "policy".into(),
        text(
            &managed
                .map(|m| m.describe(s.config.display()))
                .unwrap_or_default(),
        ),
    );
    d.insert(
        "full_tunnel".into(),
        OwnedVariant::from(s.config.learning() == crate::policy::Learning::FullTunnel),
    );
    // What a managed tunnel may learn: names and networks (Settings, Managed by IT).
    let learn = match s.config.learning() {
        crate::policy::Learning::Only { names, nets } => names
            .into_iter()
            .chain(nets.iter().map(|n| n.to_string()))
            .collect(),
        _ => Vec::new(),
    };
    d.insert("learn".into(), strings(learn));
    // Where it comes from: what the app may change.
    let source = match (&s.config.managed, &s.config.own) {
        (Some(_), _) => "managed",
        (_, Some(_)) => "app",
        _ => "config",
    };
    d.insert("source".into(), text(source));
    d.insert(
        "organisation".into(),
        text(managed.map(|m| m.organisation.as_str()).unwrap_or("")),
    );
    // An app tunnel's fields, for editing (keys as AddTunnel's).
    let own: HashMap<String, String> = s
        .config
        .own
        .as_ref()
        .map(|o| o.to_map().into_iter().collect())
        .unwrap_or_default();
    d.insert(
        "own".into(),
        OwnedVariant::try_from(Variant::from(own)).unwrap(),
    );
    // Names that go to its DNS: pushed by the gateway, plus the config's.
    let mut dns_domains: Vec<String> = info.map(|i| i.domains.clone()).unwrap_or_default();
    for d in &s.config.dns_domains {
        if !dns_domains.contains(d) {
            dns_domains.push(d.clone());
        }
    }
    d.insert("dns_domains".into(), strings(dns_domains));
    d.insert(
        "managed_file".into(),
        text(
            &managed
                .map(|m| m.file.display().to_string())
                .unwrap_or_default(),
        ),
    );
    d.insert("since".into(), OwnedVariant::from(s.since.unwrap_or(0)));
    d.insert("clash".into(), text(s.clash.as_deref().unwrap_or("")));
    d.insert("dns_down".into(), OwnedVariant::from(s.dns_down));
    d.insert("why".into(), text(s.why.as_deref().unwrap_or("")));
    // OpenVPN: its sign-in asks the user something (`Asking` says what).
    d.insert("asking".into(), OwnedVariant::from(s.asking.unwrap_or(0)));
    d.insert(
        "transport".into(),
        text(s.transport.as_deref().unwrap_or("")),
    );
    d.insert(
        "address".into(),
        text(
            &info
                .and_then(|i| {
                    i.address
                        .map(|a| a.to_string())
                        .or_else(|| i.address6.map(|(a, _)| a.to_string()))
                })
                .unwrap_or_default(),
        ),
    );
    d.insert(
        "dns".into(),
        strings(
            info.map(|i| {
                i.dns
                    .iter()
                    .map(ToString::to_string)
                    .chain(i.dns6.iter().map(ToString::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        ),
    );
    d.insert(
        "domains".into(),
        strings(info.map(|i| i.domains.clone()).unwrap_or_default()),
    );
    d.insert(
        "split_include".into(),
        strings(info.map(|i| i.split_include.clone()).unwrap_or_default()),
    );
    d.insert(
        "split_exclude".into(),
        strings(info.map(|i| i.split_exclude.clone()).unwrap_or_default()),
    );
    d
}
