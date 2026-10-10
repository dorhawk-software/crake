//! Keeping known names working while a tunnel's DNS doesn't answer
//! (CLAUDE.md, 7h). The tunnel's domains move to a dummy link, `crake-keep`,
//! whose DNS server is an address where craked answers from what that DNS
//! last said, for names it has seen, as long as the address still answers
//! through the tunnel. Anything else gets SERVFAIL: it doesn't resolve,
//! honestly.

use std::net::Ipv4Addr;
use std::sync::Arc;

use anyhow::Result;
use hickory_proto::op::Message;
use hickory_proto::rr::RecordType;
use tokio::net::UdpSocket;

pub const DEVICE: &str = "crake-keep";
/// Link-local, on the dummy link only (next to `hush::ADDR`).
pub const ADDR: Ipv4Addr = Ipv4Addr::new(169, 254, 53, 54);
/// Remembered answers are short-lived: the real DNS may be back soon.
const TTL: u32 = 30;

/// The question in a query: its name (normalized) and type.
pub fn question(query: &[u8]) -> Option<(String, RecordType)> {
    let msg = Message::from_vec(query).ok()?;
    let q = msg.queries.first()?;
    Some((crate::dns::normalize(&q.name.to_ascii()), q.query_type))
}

/// An answer to `query` with these A records (none: "no such record", for
/// a name that exists).
pub fn answer(query: &[u8], addrs: &[Ipv4Addr]) -> Option<Vec<u8>> {
    let mut out = crate::hush::reply_code(query, 0)?;
    out[7] = u8::try_from(addrs.len()).ok()?; // ANCOUNT (low byte; few records)
    for a in addrs {
        // Name: a pointer to the question's (offset 12); type A, class IN.
        out.extend_from_slice(&[0xc0, 0x0c, 0, 1, 0, 1]);
        out.extend_from_slice(&TTL.to_be_bytes());
        out.extend_from_slice(&[0, 4]);
        out.extend_from_slice(&a.octets());
    }
    Some(out)
}

/// Answer on ADDR:53 until the socket fails.
pub async fn serve(mgr: Arc<crate::manager::Manager>) -> Result<()> {
    let sock = Arc::new(UdpSocket::bind((ADDR, 53)).await?);
    let mut buf = vec![0u8; 1500];
    loop {
        let (n, from) = sock.recv_from(&mut buf).await?;
        let query = buf[..n].to_vec();
        let (mgr, sock) = (Arc::clone(&mgr), Arc::clone(&sock));
        // Each answer may wait for a check through the tunnel.
        tokio::spawn(async move {
            if let Some(reply) = mgr.keep_answer(&query).await {
                let _ = sock.send_to(&reply, from).await;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_with_the_remembered_addresses() {
        let q = crate::dns::query(0x4242, "gitlab.initech.internal", RecordType::A);
        assert_eq!(
            question(&q),
            Some(("gitlab.initech.internal".to_owned(), RecordType::A))
        );
        let a: Ipv4Addr = "10.81.1.4".parse().unwrap();
        let r = answer(&q, &[a]).unwrap();
        let msg = Message::from_vec(&r).unwrap();
        assert_eq!(msg.metadata.id, 0x4242);
        assert_eq!(
            msg.metadata.response_code,
            hickory_proto::op::ResponseCode::NoError
        );
        assert_eq!(crate::dns::a_records(&msg), vec![a]);
        assert!(msg.answers.iter().all(|r| r.ttl == TTL));
        let fail = crate::hush::reply_code(&q, 2).unwrap();
        let msg = Message::from_vec(&fail).unwrap();
        assert_eq!(
            msg.metadata.response_code,
            hickory_proto::op::ResponseCode::ServFail
        );
        assert!(msg.answers.is_empty());
    }
}
