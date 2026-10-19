//! charon as the package's systemd unit (crake-charon.service), which gives
//! it a /run of its own: its pid file's path is built in
//! (/var/run/charon.pid), so beside a system strongSwan it wouldn't start,
//! and craked's own /run is read-only to it (ProtectSystem=strict). craked
//! writes charon's settings first, then asks systemd, as root, to start,
//! look at and stop the unit (systemd's Manager over the system bus).

use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use zbus::Connection;
use zbus::zvariant::OwnedObjectPath;

/// How long systemd gets to answer.
const WAIT: Duration = Duration::from_secs(5);
/// How long the unit gets to stop.
const STOP_WAIT: Duration = Duration::from_secs(5);
const POLL: Duration = Duration::from_millis(100);

const SYSTEMD: &str = "org.freedesktop.systemd1";
const MANAGER_PATH: &str = "/org/freedesktop/systemd1";
const MANAGER: &str = "org.freedesktop.systemd1.Manager";
const UNIT: &str = "org.freedesktop.systemd1.Unit";

/// `call`, given WAIT to finish.
async fn within<T>(what: &str, call: impl Future<Output = zbus::Result<T>>) -> Result<T> {
    tokio::time::timeout(WAIT, call)
        .await
        .map_err(|_| anyhow!("{what}: systemd didn't answer"))?
        .context(what.to_owned())
}

pub struct Unit {
    bus: Connection,
    pub name: String,
}

impl Unit {
    /// Start `name` afresh (restarted, if a craked before this one left it
    /// running: it then reads the settings just written).
    pub async fn start(name: &str) -> Result<Self> {
        let bus = within("connecting to the system bus", Connection::system()).await?;
        let unit = Self {
            bus,
            name: name.to_owned(),
        };
        let _: OwnedObjectPath = within(
            &format!("starting {name}"),
            unit.manager()
                .await?
                .call("RestartUnit", &(name, "replace")),
        )
        .await?;
        Ok(unit)
    }

    async fn manager(&self) -> Result<zbus::Proxy<'static>> {
        Ok(zbus::Proxy::new(&self.bus, SYSTEMD, MANAGER_PATH, MANAGER).await?)
    }

    /// systemd's word for its state: "active", "activating", "failed", ...
    async fn state(&self) -> Result<String> {
        let path: OwnedObjectPath = within(
            "GetUnit",
            self.manager().await?.call("GetUnit", &(&self.name,)),
        )
        .await?;
        // Asked for the one property: a proxy that caches would ask for
        // them all and follow their changes, which craked's AppArmor
        // profile doesn't allow (nor need).
        let unit: zbus::Proxy = zbus::proxy::Builder::new(&self.bus)
            .destination(SYSTEMD)?
            .path(path)?
            .interface(UNIT)?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await?;
        within("ActiveState", unit.get_property::<String>("ActiveState")).await
    }

    /// It runs (or is starting).
    pub async fn running(&self) -> bool {
        matches!(
            self.state().await.as_deref(),
            Ok("active" | "activating" | "reloading")
        )
    }

    pub async fn stop(&self) {
        if let Ok(m) = self.manager().await {
            let _ =
                within::<OwnedObjectPath>("StopUnit", m.call("StopUnit", &(&self.name, "replace")))
                    .await;
        }
        let deadline = tokio::time::Instant::now() + STOP_WAIT;
        while self.running().await && tokio::time::Instant::now() < deadline {
            tokio::time::sleep(POLL).await;
        }
    }
}
