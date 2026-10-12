//! A TCP connection as craked follows it: its four-tuple and what became
//! of it. Linux reports these from conntrack (`conntrack`); other systems
//! from their own connection tables. Portable.

use std::net::IpAddr;

/// The parts of a TCP state we care about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpState {
    SynSent,
    /// Any state after a reply: the other side answered.
    Answered,
    /// Reset before it was ever established: refused.
    Close,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Flow {
    pub src: IpAddr,
    pub dst: IpAddr,
    pub sport: u16,
    pub dport: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    New,
    Update,
    Destroy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Event {
    pub kind: Kind,
    pub flow: Flow,
    pub state: Option<TcpState>,
}

/// What changed between two looks at a connection table, as conntrack would
/// have said it (Windows has no such events: craked polls its TCP table).
pub fn changes(
    before: &std::collections::HashMap<Flow, TcpState>,
    now: &std::collections::HashMap<Flow, TcpState>,
) -> Vec<Event> {
    let mut out: Vec<Event> = now
        .iter()
        .filter_map(|(flow, state)| {
            let kind = match before.get(flow) {
                None => Kind::New,
                Some(old) if old != state => Kind::Update,
                Some(_) => return None,
            };
            Some(Event {
                kind,
                flow: *flow,
                state: Some(*state),
            })
        })
        .collect();
    out.extend(
        before
            .keys()
            .filter(|f| !now.contains_key(f))
            .map(|flow| Event {
                kind: Kind::Destroy,
                flow: *flow,
                state: None,
            }),
    );
    out
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn a_table_looked_at_twice_says_what_happened() {
        let flow = |port| Flow {
            src: [192, 168, 0, 242].into(),
            dst: [10, 40, 1, 10].into(),
            sport: port,
            dport: 443,
        };
        let before = HashMap::from([
            (flow(40001), TcpState::SynSent),
            (flow(40002), TcpState::SynSent),
            (flow(40003), TcpState::Answered),
        ]);
        let now = HashMap::from([
            (flow(40002), TcpState::Answered),
            (flow(40003), TcpState::Answered),
            (flow(40004), TcpState::SynSent),
        ]);
        let mut seen: Vec<(u16, Kind, Option<TcpState>)> = changes(&before, &now)
            .iter()
            .map(|e| (e.flow.sport, e.kind, e.state))
            .collect();
        seen.sort_by_key(|(port, _, _)| *port);
        assert_eq!(
            seen,
            vec![
                // Gone without an answer: refused, or given up on.
                (40001, Kind::Destroy, None),
                (40002, Kind::Update, Some(TcpState::Answered)),
                (40004, Kind::New, Some(TcpState::SynSent)),
            ]
        );
        assert!(changes(&now, &now).is_empty(), "nothing new, nothing said");
    }
}
