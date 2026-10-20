//! Settings that change what Crake does with names: asking the tunnels
//! about unknown names, and keeping internal names private.

use super::platform::{self, look_up, way};
use super::{Up, set};

/// A name only ACME's DNS knows, under a public-looking domain, asked by
/// no other scenario (so no system cache holds it).
const UNKNOWN: (&str, &str) = ("files.acme.example", "10.40.1.15");

/// With "Ask the tunnels about unknown names" off, a name public DNS
/// doesn't know stays unknown, and nothing is routed for it.
pub(crate) async fn with_the_rescue_off_an_unknown_name_stays_unknown(up: &mut Up) {
    set(&mut up.app, "ask_unknown", "false").await;
    let (name, at) = UNKNOWN;
    let found = look_up(name).await;
    assert!(
        !found.iter().any(|a| a.to_string() == at),
        "{name} was rescued with the rescue off: {found:?}"
    );
    assert_ne!(way(at), up.device, "{at} was routed");
}

/// With "Keep internal names private" off and the tunnel off, its names
/// go to public DNS like any other (where the test can see public DNS:
/// the question is in its log).
pub(crate) async fn names_not_kept_private_go_to_public_dns(up: &mut Up) {
    set(&mut up.app, "keep_private", "false").await;
    up.off().await;
    let before = platform::public_questions();
    let name = "wiki.acme.internal";
    assert!(look_up(name).await.is_empty(), "{name} with its tunnel off");
    if let (Some(before), Some(after)) = (before, platform::public_questions()) {
        let since = after.strip_prefix(before.as_str()).unwrap_or(&after);
        assert!(
            since.contains(name),
            "{name} wasn't asked of public DNS:\n{since}"
        );
    }
}
