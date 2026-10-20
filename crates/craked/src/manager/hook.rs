//! The hook's calls: openconnect's script (and openvpn's and charon's)
//! reporting a tunnel up, reconnecting or down.

use super::*;

impl Manager {
    /// Apply one call from the hook. An error makes the hook fail, which makes
    /// openconnect give up on that step.
    pub async fn handle_hook(&self, msg: HookMessage) -> Result<()> {
        let (display, state, protocol, replacing) = {
            let map = self.tunnels.lock().unwrap();
            let e = map
                .get(&msg.tunnel)
                .ok_or_else(|| anyhow!("unknown tunnel {}", msg.tunnel))?;
            (
                e.config.display().to_owned(),
                e.state,
                e.config.protocol,
                e.replacing,
            )
        };
        // Only a tunnel Crake started may report in.
        if state == TunnelState::Off || state == TunnelState::Trouble {
            bail!("{display} is not running");
        }
        let name = msg.tunnel.as_str();

        match msg.reason.as_str() {
            "pre-init" => Ok(()),
            "connect" | "reconnect" if replacing => {
                tracing::debug!(
                    tunnel = name,
                    "a connect from the openconnect being replaced"
                );
                Ok(())
            }
            "connect" | "reconnect" => {
                let info = match protocol {
                    crate::config::Protocol::Openvpn => {
                        // Windows' openvpn keeps the pushed DNS to itself:
                        // what its output said fills in what the hook lacks.
                        let mut env = msg.env.clone();
                        if let Some(p) = self.pushed.lock().unwrap().get(name) {
                            p.fill(&mut env);
                        }
                        crate::openvpn::env::info(&env)?
                    }
                    crate::config::Protocol::Ipsec => {
                        // charon calls updown once per pair of traffic
                        // selectors, and again when it signs in anew: one
                        // tunnel coming up once.
                        if state == TunnelState::Connected {
                            return Ok(());
                        }
                        crate::ipsec::env::info(&msg.env, &interface_name(name))?
                    }
                    _ => TunnelInfo::from_env(&msg.env)?,
                };
                // What the engine handed over: its variables' names (values
                // may be personal), and what Crake made of them.
                tracing::debug!(
                    tunnel = name,
                    keys = ?msg.env.keys().collect::<Vec<_>>(),
                    ?info,
                    "the hook's {}",
                    msg.reason
                );
                if !crate::sys::tunnel_device(name, &info.device) {
                    bail!("device {} does not belong to {display}", info.device);
                }
                if let Some(addr) = info.address {
                    self.net
                        .assign(
                            &info.device,
                            addr,
                            info.prefix.unwrap_or(crate::hosts::V4_BITS),
                            info.mtu,
                        )
                        .await?;
                }
                if let Some((addr, prefix)) = info.address6 {
                    self.net.assign6(&info.device, addr, prefix).await?;
                }
                self.tunnel_up(name, &display, info, msg.reason == "connect")
                    .await
            }
            "attempt-reconnect" => {
                self.set_state(name, TunnelState::Connecting, false);
                if let Some(e) = self.tunnels.lock().unwrap().get_mut(name) {
                    e.ups += 1;
                }
                self.log.system(
                    Some(name),
                    format!("{display} lost the gateway. Trying again."),
                    None,
                );
                Ok(())
            }
            "disconnect" => Ok(()), // the supervisor logs the end once the process exits
            other => {
                tracing::debug!(reason = other, "ignoring hook reason");
                Ok(())
            }
        }
    }
}
