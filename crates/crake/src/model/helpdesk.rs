//! "Copy for helpdesk": what Crake says about itself, its tunnels and its
//! latest decisions, in plain text anyone can read.

use super::*;

/// Decisions in a helpdesk copy.
const DECISIONS: usize = 20;
/// The helpdesk copy's first sentence: what Crake does, for someone who
/// hasn't met it.
pub(super) const OPENER: &str =
    "Crake sends only the hosts that need a tunnel through one. Everything else goes direct.";

impl Model {
    /// "Copy for helpdesk": a plain-text summary anyone can read.
    /// With `hide_hosts`, host names and addresses become "a host": tunnels
    /// and timing only (Settings, "Hide names and addresses when sharing").
    pub fn helpdesk(&self, hide_hosts: bool) -> String {
        let say = |t: &str| if hide_hosts { scrub(t) } else { t.to_owned() };
        let now = jiff::Zoned::now();
        let mut s = format!(
            "Crake, {}\n\n{OPENER}\n\nTunnels\n",
            now.strftime("%Y-%m-%d %H:%M %Z")
        );
        if self.tunnels.is_empty() {
            s += "  (none)\n";
        }
        for t in &self.tunnels {
            s += &format!("  {}: {}", t.display, tunnel_state(t));
            // Since when, to which gateway, with which engine: what a
            // helpdesk asks first.
            if t.state == "connected" && t.since > 0 {
                let since = jiff::Timestamp::from_second(t.since as i64)
                    .map(|at| {
                        at.to_zoned(jiff::tz::TimeZone::system())
                            .strftime("%H:%M")
                            .to_string()
                    })
                    .unwrap_or_default();
                s += &format!(", since {since}");
                if !t.gateway.is_empty() && !hide_hosts {
                    s += &format!(" to {}", t.gateway);
                }
                s += &format!(" with {}", engine(&t.protocol));
            }
            if !t.address.is_empty() && !hide_hosts {
                s += &format!(", address {}", t.address);
            }
            s += "\n";
        }
        s += "\nRecent decisions (newest first)\n";
        let recent = self.decisions(DECISIONS);
        if recent.is_empty() {
            s += "  (none)\n";
        }
        for d in recent {
            s += &format!("  {}  {}\n", d.time, say(&d.entry.message));
            if let Some(r) = &d.entry.reason {
                s += &format!("         {}\n", say(r));
            }
        }
        // What only craked sees: saves asking the user to run commands.
        if !self.diagnostics.is_empty() || !self.own_dns_browsers.is_empty() {
            s += "\nCrake's view of this computer\n";
            for line in &self.diagnostics {
                s += &format!("  {}\n", say(line));
            }
            if !self.own_dns_browsers.is_empty() {
                let names = crate::browser_dns::and_list(&self.own_dns_browsers);
                let verb = if self.own_dns_browsers.len() == 1 {
                    "looks"
                } else {
                    "look"
                };
                s += &format!(
                    "  {names} {verb} names up themselves (DNS over HTTPS): Crake can't route a company's names there before the first packet.\n"
                );
            }
        }
        s
    }
}

/// What runs a tunnel's protocol, as a helpdesk would know it.
fn engine(protocol: &str) -> &'static str {
    match protocol {
        "openvpn3" => "openvpn3",
        "openvpn" => "OpenVPN",
        "ipsec" => "strongSwan",
        _ => "openconnect",
    }
}
