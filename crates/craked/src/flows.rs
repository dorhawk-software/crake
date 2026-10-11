//! A TCP connection as craked follows it: its four-tuple and what became
//! of it. Linux reports these from conntrack (`conntrack`); other systems
//! from their own connection tables. Portable.

use std::net::Ipv4Addr;

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
    pub src: Ipv4Addr,
    pub dst: Ipv4Addr,
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
