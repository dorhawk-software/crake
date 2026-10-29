//! What craked keeps for each tunnel while it runs.

use super::*;

/// Where a run of openvpn takes the user's answers: (question id, text).
pub(super) type Answers = tokio::sync::mpsc::UnboundedSender<(u32, String)>;

pub(super) struct Entry {
    pub(super) config: TunnelConfig,
    /// Position in the config; picks the tunnel's mark, rule and table.
    pub(super) index: usize,
    pub(super) state: TunnelState,
    pub(super) info: Option<TunnelInfo>,
    pub(super) stop: Option<oneshot::Sender<Stop>>,
    /// The last sign-in, reused to switch the tunnel on again.
    pub(super) session: Option<Session>,
    /// openconnect's supervisor hasn't finished yet (stays true while stopping).
    pub(super) running: bool,
    /// The kernel sets and rule for this tunnel are in place.
    pub(super) routed: bool,
    /// Restarted over TCP (UDP blocked): its next "connect" is a return.
    pub(super) resumed: bool,
    /// The openconnect that found UDP blocked is being stopped for one over
    /// TCP: a "connect" it still sends (its script was already running) is
    /// for a device about to go, not the tunnel coming up.
    pub(super) replacing: bool,
    /// When it last came up (seconds since the epoch); kept across a
    /// reconnect, cleared when it goes off.
    pub(super) up_since: Option<u64>,
    /// How many times it came up or began to come up again (connected,
    /// reconnecting, restarted over TCP): a probe that spans a change says
    /// nothing about the host.
    pub(super) ups: u64,
    /// When it lost its gateway (seconds since the epoch), until it's back.
    pub(super) lost_at: Option<u64>,
    /// Refused because it clashes with a tunnel that's up: it's stopping,
    /// already said why, and goes Off quietly.
    pub(super) refused: bool,
    /// Handed over to the new craked of a package upgrade (`handover`): it
    /// stops, and starts again there, without a line in the log.
    pub(super) handover: bool,
    /// OpenVPN 3: what the app attached it with, for a handover.
    pub(super) attached: Option<HashMap<String, String>>,
    /// In trouble: why (cleared when it leaves trouble).
    pub(super) why: Option<String>,
    /// OpenVPN: the sign-in the gateway gave (its auth-token), reused like
    /// openconnect's session.
    pub(super) token: Option<crate::openvpn::Token>,
    /// OpenVPN: what it's asking the user now, and where the answer (with
    /// the question's id) goes.
    pub(super) asking: Option<(crate::openvpn::Ask, Answers)>,
}

impl Entry {
    /// A tunnel as it starts: off, nothing kept.
    pub(super) fn new(config: TunnelConfig, index: usize) -> Self {
        Self {
            config,
            index,
            state: TunnelState::Off,
            info: None,
            stop: None,
            session: None,
            running: false,
            routed: false,
            resumed: false,
            replacing: false,
            up_since: None,
            ups: 0,
            lost_at: None,
            refused: false,
            handover: false,
            attached: None,
            why: None,
            token: None,
            asking: None,
        }
    }
}
