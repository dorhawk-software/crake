//! OpenVPN tunnels (CLAUDE.md, 7i): craked runs `openvpn` with the profile
//! it keeps (`run`), answers its management interface (`sign_in`), and
//! asks the app what only the user can answer (here). Windows comes later
//! (7i, slice 4: its management interface is a TCP port there, and its
//! drivers come with OpenVPN's installer); until then switching such a
//! tunnel on there says so (`tunnels::connect_with`).

use super::*;

#[cfg(unix)]
mod run;
#[cfg(unix)]
mod sign_in;

impl Manager {
    /// What a tunnel's sign-in asks the user now (D-Bus `Asking`).
    pub fn asking(&self, tunnel: &str) -> Option<BTreeMap<String, String>> {
        let map = self.tunnels.lock().unwrap();
        Some(map.get(tunnel)?.asking.as_ref()?.0.to_map())
    }

    /// The user's answer to question `id` (D-Bus `Answer`).
    pub fn answer(&self, tunnel: &str, id: u32, text: String) -> Result<()> {
        let map = self.tunnels.lock().unwrap();
        let e = map
            .get(tunnel)
            .ok_or_else(|| anyhow!("no tunnel named {tunnel}"))?;
        match &e.asking {
            Some((ask, answers)) if ask.id == id => {
                let _ = answers.send((id, text));
                Ok(())
            }
            _ => bail!("{} isn\u{2019}t asking that any more.", e.config.display()),
        }
    }
}
