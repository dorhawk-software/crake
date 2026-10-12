//! One tunnel's routing as craked wants it (a `Plan`), and the numbers each
//! tunnel gets by its position (mark, routing table, rule priority). What
//! the operating system's routing (`routing`) puts in place. Portable.

use std::net::{IpAddr, Ipv4Addr};

use crate::hosts::Target;

pub const MARK_BASE: u32 = 0x4352_0000; // "CR"
/// Rule priorities and routing tables are BASE + 1 + tunnel index.
pub const ID_BASE: u32 = 7300;
pub const MAX_TUNNELS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ids {
    pub mark: u32,
    pub table: u32,
    pub priority: u32,
}

pub fn ids(index: usize) -> Ids {
    let n = index as u32 + 1;
    Ids {
        mark: MARK_BASE | n,
        table: ID_BASE + n,
        priority: ID_BASE + n,
    }
}

/// Everything needed to put one tunnel's routing in place.
pub struct Plan<'a> {
    pub name: &'a str,
    pub index: usize,
    pub device: &'a str,
    /// Default expiry for learned hosts, seconds.
    pub timeout: u64,
    /// The tunnel's DNS servers; their answers are queued for learning.
    pub dns: &'a [Ipv4Addr],
    pub pinned: &'a [Target],
    /// Learned hosts with seconds left.
    pub learned: &'a [(IpAddr, u64)],
    /// Full tunnel (a managed policy): everything goes through except these
    /// (the gateway, the local networks). Other tunnels' hosts still go
    /// their own way.
    pub full_except: Option<&'a [Target]>,
}
