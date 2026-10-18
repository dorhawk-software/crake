//! A profile (.ovpn) read before craked keeps it. craked runs openvpn as
//! root, so only client options known to be harmless get in: nothing that
//! runs a program, loads a library, or reads or writes a file outside the
//! profile. What Crake does itself (the device, scripts, DNS, dropping
//! privileges) is left out. Anything else is refused by name, so a real
//! profile says what's missing here.

use anyhow::{Result, bail};

/// Larger than any real profile.
const MAX_SIZE: usize = 512 * 1024;

/// Copied as they are.
const KEEP: &[&str] = &[
    "allow-compression",
    "allow-pull-fqdn",
    "auth",
    "auth-nocache",
    "bind",
    "block-ipv6",
    "cipher",
    "client",
    "comp-lzo",
    "compress",
    "connect-retry",
    "connect-retry-max",
    "connect-timeout",
    "data-ciphers",
    "data-ciphers-fallback",
    "dhcp-option",
    "disable-dco",
    "dns",
    "explicit-exit-notify",
    "fast-io",
    "float",
    "fragment",
    "hand-window",
    "http-proxy-option",
    "ifconfig-nowarn",
    "ignore-unknown-option",
    "inactive",
    "keepalive",
    "key-direction",
    "key-method",
    "link-mtu",
    "local",
    "lport",
    "mlock",
    "mssfix",
    "mtu-disc",
    "mute",
    "mute-replay-warnings",
    "ncp-ciphers",
    "ncp-disable",
    "nice",
    "nobind",
    "ns-cert-type",
    "persist-local-ip",
    "persist-remote-ip",
    "ping",
    "ping-exit",
    "ping-restart",
    "port",
    "proto",
    "proto-force",
    "pull",
    "pull-filter",
    "push-peer-info",
    "rcvbuf",
    "redirect-gateway",
    "redirect-private",
    "remote",
    "remote-cert-eku",
    "remote-cert-ku",
    "remote-cert-tls",
    "remote-random",
    "remote-random-hostname",
    "reneg-bytes",
    "reneg-pkts",
    "reneg-sec",
    "replay-window",
    "resolv-retry",
    "route",
    "route-gateway",
    "route-ipv6",
    "route-metric",
    "route-nopull",
    "rport",
    "server-poll-timeout",
    "setenv",
    "single-session",
    "sndbuf",
    "socket-flags",
    "static-challenge",
    "tcp-nodelay",
    "tls-cert-profile",
    "tls-cipher",
    "tls-ciphersuites",
    "tls-client",
    "tls-exit",
    "tls-groups",
    "tls-timeout",
    "tls-version-max",
    "tls-version-min",
    "topology",
    "tran-window",
    "tun-ipv6",
    "tun-mtu",
    "tun-mtu-extra",
    "txqueuelen",
    "verify-x509-name",
];

/// Options that name a file: only as an inline block (`<ca>…</ca>`), which
/// the app makes of the files beside the profile when importing it.
const INLINE: &[&str] = &[
    "auth-user-pass",
    "ca",
    "cert",
    "crl-verify",
    "dh",
    "extra-certs",
    "http-proxy-user-pass",
    "key",
    "peer-fingerprint",
    "pkcs12",
    "secret",
    "tls-auth",
    "tls-crypt",
    "tls-crypt-v2",
];

/// Left out: Crake does these itself (the device, scripts, DNS, how it
/// signs in and logs), or they'd stop it from working.
const DROP: &[&str] = &[
    "auth-retry",
    "block-outside-dns",
    "daemon",
    "dev",
    "dev-node",
    "dev-type",
    "dns-updown",
    "down",
    "down-pre",
    "group",
    "ifconfig-noexec",
    "ip-win32",
    "ipchange",
    "machine-readable-output",
    "management-query-passwords",
    "persist-key",
    "persist-tun",
    "register-dns",
    "route-delay",
    "route-method",
    "route-noexec",
    "route-pre-down",
    "route-up",
    "script-security",
    "up",
    "up-delay",
    "up-restart",
    "user",
    "verb",
    "windows-driver",
];

/// A profile craked may run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    /// What craked writes and runs.
    pub text: String,
    /// The gateway it names first (a name or an address), for people.
    pub remote: String,
    /// Its own `<auth-user-pass>`: the user name, and the password when
    /// it has one (often a placeholder, for a browser sign-in).
    pub credentials: Option<(String, Option<String>)>,
}

/// Read a profile; refuse it, saying why, if craked shouldn't run it.
pub fn check(text: &str) -> Result<Profile> {
    if text.len() > MAX_SIZE {
        bail!("That profile is too large to be an OpenVPN profile.");
    }
    let mut out = String::new();
    let mut remote = None;
    let mut credentials = None;
    let mut asks_password = false;
    let mut client = false;
    let mut lines = text.lines();
    while let Some(raw) = lines.next() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if let Some(tag) = opening(line) {
            let body = block(&mut lines, tag)?;
            match tag {
                "auth-user-pass" => {
                    let mut l = body.lines().map(str::trim);
                    let user = l.next().unwrap_or("").to_owned();
                    let pass = l.next().filter(|p| !p.is_empty()).map(str::to_owned);
                    credentials = Some((user, pass));
                    asks_password = true;
                }
                "connection" => {
                    let inner = check_connection(&body, &mut remote)?;
                    out.push_str(&format!("<connection>\n{inner}</connection>\n"));
                }
                t if INLINE.contains(&t) => {
                    out.push_str(&format!("<{t}>\n{body}</{t}>\n"));
                }
                t => bail!("The profile has a <{t}> block, which Crake can't use."),
            }
            continue;
        }
        let words = words(line);
        let Some(name) = words.first().map(String::as_str) else {
            continue;
        };
        let name = name.strip_prefix("--").unwrap_or(name);
        match name {
            "client" | "tls-client" => client = true,
            "remote" if remote.is_none() => remote = words.get(1).cloned(),
            _ => {}
        }
        if DROP.contains(&name) {
            continue;
        }
        if name == "auth-user-pass" {
            match words.get(1).map(String::as_str) {
                None | Some("[inline]") => asks_password = true,
                Some(_) => bail!(file_named(name)),
            }
            continue;
        }
        if INLINE.contains(&name) {
            match words.get(1).map(String::as_str) {
                Some("[inline]") => continue,
                Some(_) => bail!(file_named(name)),
                None => bail!("The profile's {name} says nothing."),
            }
        }
        if name == "setenv" && words.get(1).is_some_and(|v| v.starts_with("CRAKE_")) {
            bail!("The profile sets {}, which is Crake's own.", words[1]);
        }
        if matches!(name, "http-proxy" | "socks-proxy") {
            // Host and port; for HTTP, then "auto" (and how) for its own
            // proxy authentication. Anything else there names a file of
            // credentials.
            const AUTH_WORD: usize = 3;
            let auth = words.get(AUTH_WORD).map(String::as_str);
            let harmless = match name {
                "socks-proxy" => auth.is_none(),
                _ => matches!(auth, None | Some("auto" | "auto-nct")),
            };
            if !harmless {
                bail!(file_named(name));
            }
        } else if !KEEP.contains(&name) {
            bail!("The profile uses {name}, which Crake doesn't run.");
        }
        out.push_str(line);
        out.push('\n');
    }
    if !client {
        bail!("That isn't a client profile: it doesn't say client.");
    }
    let Some(remote) = remote else {
        bail!("The profile names no gateway (remote).");
    };
    if asks_password {
        // craked answers it over the management interface.
        out.push_str("auth-user-pass\n");
    }
    Ok(Profile {
        text: out,
        remote,
        credentials,
    })
}

fn file_named(name: &str) -> String {
    format!(
        "The profile's {name} points at a file. Import the profile in Crake, so its files come with it."
    )
}

/// `<name>` opens a block.
fn opening(line: &str) -> Option<&str> {
    let tag = line.strip_prefix('<')?.strip_suffix('>')?;
    (!tag.starts_with('/') && !tag.is_empty()).then_some(tag)
}

/// A block's lines up to `</tag>`.
fn block<'a>(lines: &mut impl Iterator<Item = &'a str>, tag: &str) -> Result<String> {
    let close = format!("</{tag}>");
    let mut body = String::new();
    for line in lines.by_ref() {
        if line.trim() == close {
            return Ok(body);
        }
        body.push_str(line);
        body.push('\n');
    }
    bail!("The profile's <{tag}> block never ends.")
}

/// What a `<connection>` may say: where and how to reach the gateway.
fn check_connection(body: &str, remote: &mut Option<String>) -> Result<String> {
    const CONNECTION: &[&str] = &[
        "bind",
        "connect-retry",
        "connect-retry-max",
        "connect-timeout",
        "explicit-exit-notify",
        "float",
        "fragment",
        "link-mtu",
        "local",
        "lport",
        "mssfix",
        "nobind",
        "port",
        "proto",
        "remote",
        "rport",
        "server-poll-timeout",
        "tun-mtu",
        "tun-mtu-extra",
    ];
    let mut out = String::new();
    for line in body.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        let w = words(line);
        let name = w.first().map_or("", String::as_str);
        if !CONNECTION.contains(&name) {
            bail!("A <connection> in the profile uses {name}, which Crake doesn't run.");
        }
        if name == "remote" && remote.is_none() {
            *remote = w.get(1).cloned();
        }
        out.push_str(line);
        out.push('\n');
    }
    Ok(out)
}

/// A line's words as openvpn splits them: by spaces, "quoted" or 'quoted'
/// words whole, a backslash keeping the next character.
pub fn words(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    let mut has_word = false;
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match (quote, c) {
            (_, '\\') if quote != Some('\'') => {
                if let Some(next) = chars.next() {
                    word.push(next);
                }
                has_word = true;
            }
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => word.push(c),
            (None, '"' | '\'') => {
                quote = Some(c);
                has_word = true;
            }
            (None, '#' | ';') if !has_word => break,
            (None, c) if c.is_whitespace() => {
                if has_word {
                    words.push(std::mem::take(&mut word));
                    has_word = false;
                }
            }
            (None, c) => {
                word.push(c);
                has_word = true;
            }
        }
    }
    if has_word {
        words.push(word);
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    const INITECH: &str = "\
auth-nocache
<auth-user-pass>
sso-placeholder
sso-placeholder
</auth-user-pass>
<ca>
-----BEGIN CERTIFICATE-----
MIIB
-----END CERTIFICATE-----
</ca>
client
data-ciphers AES-256-GCM:AES-128-GCM:AES-256-CBC
dev tun
nobind
persist-key
persist-tun
proto udp
remote vpn.initech.example 1194
remote-cert-tls server
resolv-retry infinite
verb 3
";

    #[test]
    fn a_browser_sign_in_profile_keeps_what_openvpn_needs() {
        let p = check(INITECH).unwrap();
        assert_eq!(p.remote, "vpn.initech.example");
        assert_eq!(
            p.credentials,
            Some(("sso-placeholder".into(), Some("sso-placeholder".into())))
        );
        assert!(p.text.contains("<ca>\n-----BEGIN CERTIFICATE-----\nMIIB\n"));
        assert!(p.text.contains("remote vpn.initech.example 1194\n"));
        // Crake's own: the device, keeping it, the log's detail.
        for gone in [
            "dev tun",
            "persist-tun",
            "persist-key",
            "verb 3",
            "sso-placeholder",
        ] {
            assert!(!p.text.contains(gone), "{gone} kept:\n{}", p.text);
        }
        // craked answers the password over the management interface.
        assert!(p.text.ends_with("auth-user-pass\n"));
    }

    #[test]
    fn scripts_are_left_out() {
        let p = check(
            "client\nremote vpn.example 1194\nscript-security 2\n\
             up /etc/openvpn/update-resolv-conf\ndown /etc/openvpn/update-resolv-conf\n",
        )
        .unwrap();
        assert_eq!(p.text, "client\nremote vpn.example 1194\n");
    }

    #[test]
    fn nothing_that_runs_or_reads_files_as_root_gets_in() {
        for bad in [
            "plugin /usr/lib/x.so",
            "engine dynamic",
            "pkcs11-providers /tmp/evil.so",
            "management 127.0.0.1 7505",
            "config /etc/shadow",
            "log /etc/cron.d/x",
            "cd /root",
            "tls-verify /tmp/x",
            "ca /etc/ssl/private/key.pem",
            "auth-user-pass /root/creds",
            "http-proxy proxy.example 3128 /root/creds",
            "socks-proxy proxy.example 1080 /root/creds",
            "setenv CRAKE_HOOK_SOCKET /tmp/x",
            "status /etc/passwd",
            "bogus-option 1",
        ] {
            let text = format!("client\nremote vpn.example 1194\n{bad}\n");
            assert!(check(&text).is_err(), "{bad} was let in");
        }
        let text = "client\nremote vpn.example\n<plugin>\nx\n</plugin>\n";
        assert!(check(text).is_err());
    }

    #[test]
    fn connection_blocks_name_the_gateway_and_are_checked() {
        let p = check(
            "client\n<connection>\nremote one.example 443 tcp\n</connection>\n\
             <connection>\nremote two.example 1194 udp\n</connection>\n",
        )
        .unwrap();
        assert_eq!(p.remote, "one.example");
        assert!(check("client\n<connection>\nremote a 1\nup /x\n</connection>\n").is_err());
    }

    #[test]
    fn a_profile_says_it_is_a_client_and_where_to() {
        assert!(check("remote vpn.example\n").is_err());
        assert!(check("client\n").is_err());
        assert!(
            check("client\n<ca>\nx\n").is_err(),
            "a block that never ends"
        );
    }

    #[test]
    fn words_split_as_openvpn_does() {
        assert_eq!(
            words("remote a.example 1194 # x"),
            ["remote", "a.example", "1194"]
        );
        assert_eq!(words(r#"setenv UV_X "a b""#), ["setenv", "UV_X", "a b"]);
        assert_eq!(words(r"x a\ b"), ["x", "a b"]);
    }
}
