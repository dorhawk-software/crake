//! openvpn3's D-Bus services as Crake asks them: a method call, a
//! property, and patience while a service that the call itself started
//! comes up.

use std::time::Duration;

use zbus::Connection;
use zbus::zvariant::OwnedValue;

/// openvpn3's configuration service starts when first asked and exits when
/// idle; just started, it answers before it has read the saved profiles
/// (seen 2026-10-15: "no profile called initech" at once, found on "Try
/// again" ten seconds later). An empty answer is asked again this long.
pub(super) const PROFILES_LOADING: Duration = Duration::from_secs(3);
pub(super) const PROFILES_POLL: Duration = Duration::from_millis(250);

/// Ask openvpn3's configuration service, asking again for a moment while
/// it finds nothing: it may have just started (`PROFILES_LOADING`).
pub(super) async fn patiently<T, E, F, Fut>(ask: F) -> Result<Vec<T>, E>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<Vec<T>, E>>,
{
    let mut waited = Duration::ZERO;
    loop {
        let found = ask().await?;
        if !found.is_empty() || waited >= PROFILES_LOADING {
            return Ok(found);
        }
        tokio::time::sleep(PROFILES_POLL).await;
        waited += PROFILES_POLL;
    }
}

pub(super) async fn call<B, R>(
    conn: &Connection,
    dest: &str,
    path: &str,
    iface: &str,
    method: &str,
    body: &B,
) -> zbus::Result<R>
where
    B: serde::Serialize + zbus::zvariant::DynamicType,
    R: for<'d> zbus::zvariant::DynamicDeserialize<'d>,
{
    let mut waited = Duration::ZERO;
    loop {
        match conn
            .call_method(Some(dest), path, Some(iface), method, body)
            .await
        {
            Ok(reply) => return reply.body().deserialize(),
            // The configuration service, started by this very call, has
            // no manager object yet (seen 2026-10-15: "Object does not
            // exist at path /net/openvpn/v3/configuration" at once, fine
            // on "Try again"). Only the manager: a session's object that
            // doesn't exist really is gone.
            Err(e)
                if path == CONFIGURATION_MANAGER && starting(&e) && waited < PROFILES_LOADING =>
            {
                tokio::time::sleep(PROFILES_POLL).await;
                waited += PROFILES_POLL;
            }
            Err(e) => return Err(e),
        }
    }
}

/// The configuration service's manager object.
const CONFIGURATION_MANAGER: &str = "/net/openvpn/v3/configuration";

/// An error a service gives while it's still starting: no object there yet.
fn starting(e: &zbus::Error) -> bool {
    matches!(e, zbus::Error::MethodError(name, _, _)
        if name.as_str() == "org.freedesktop.DBus.Error.UnknownObject"
            || name.as_str() == "org.freedesktop.DBus.Error.UnknownMethod")
}

pub(super) async fn property<T>(
    conn: &Connection,
    dest: &str,
    path: &str,
    iface: &str,
    name: &str,
) -> zbus::Result<T>
where
    T: TryFrom<OwnedValue>,
{
    let v: OwnedValue = call(
        conn,
        dest,
        path,
        "org.freedesktop.DBus.Properties",
        "Get",
        &(iface, name),
    )
    .await?;
    T::try_from(v).map_err(|_| zbus::Error::Failure(format!("{name}: unexpected type")))
}
