//! "Keep internal names private" (Settings, Names): while a tunnel is off,
//! names under its domains must not go to the internet provider's DNS. A
//! dummy link, `crake-hush`, takes those domains in systemd-resolved as
//! routing domains; its DNS server is an address on that link where craked
//! answers every question with REFUSED, so such names fail at once and stay
//! on the machine.

use std::net::Ipv4Addr;

use anyhow::Result;
use tokio::net::UdpSocket;

pub const DEVICE: &str = "crake-hush";
/// Link-local, on the dummy link only.
pub const ADDR: Ipv4Addr = Ipv4Addr::new(169, 254, 53, 53);

/// The answer to any question: the same id and question, REFUSED, nothing else.
pub fn refuse(query: &[u8]) -> Option<Vec<u8>> {
    if query.len() < 12 || query[2] & 0x80 != 0 {
        return None; // too short, or itself a response
    }
    let mut out = query.to_vec();
    out[2] = 0x80 | (query[2] & 0x79); // QR, keep opcode and RD
    out[3] = 0x80 | 5; // RA, REFUSED
    // Keep the question; no answers, authority or additional records.
    out[6..12].fill(0);
    let mut i = 12;
    for _ in 0..u16::from_be_bytes([query[4], query[5]]) {
        while *out.get(i)? != 0 {
            i += usize::from(out[i]) + 1;
        }
        i += 5;
    }
    out.truncate(i.min(out.len()));
    Some(out)
}

/// Answer on ADDR:53 until the socket fails.
pub async fn serve() -> Result<()> {
    let sock = UdpSocket::bind((ADDR, 53)).await?;
    let mut buf = vec![0u8; 1500];
    loop {
        let (n, from) = sock.recv_from(&mut buf).await?;
        if let Some(reply) = refuse(&buf[..n]) {
            let _ = sock.send_to(&reply, from).await;
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn refuses_with_the_question_kept() {
        let q = crate::dns::query(
            0x1234,
            "git.acme.internal",
            hickory_proto::rr::RecordType::A,
        );
        let r = super::refuse(&q).unwrap();
        let msg = hickory_proto::op::Message::from_vec(&r).unwrap();
        assert_eq!(msg.metadata.id, 0x1234);
        assert_eq!(
            msg.metadata.response_code,
            hickory_proto::op::ResponseCode::Refused
        );
        assert_eq!(msg.queries.len(), 1);
        assert!(msg.answers.is_empty() && msg.additionals.is_empty());
        assert!(super::refuse(&r).is_none(), "never answers an answer");
    }
}
