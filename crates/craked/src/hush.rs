//! "Keep internal names private" (Settings, Names): while a tunnel is off,
//! names under its domains must not go to the internet provider's DNS. A
//! dummy link, `crake-hush`, takes those domains in systemd-resolved as
//! routing domains; its DNS server is an address on that link where craked
//! answers every question with REFUSED, so such names fail at once and stay
//! on the machine.

use std::net::Ipv4Addr;

use anyhow::Result;
use tokio::net::UdpSocket;

use crate::dns;

pub const DEVICE: &str = "crake-hush";
/// Link-local, on the dummy link only.
pub const ADDR: Ipv4Addr = Ipv4Addr::new(169, 254, 53, 53);

/// The answer to any question: the same id and question, REFUSED, nothing else.
pub fn refuse(query: &[u8]) -> Option<Vec<u8>> {
    reply_code(query, dns::REFUSED)
}

/// The same id and question with response code `rcode` (5 REFUSED,
/// 2 SERVFAIL, 0 with no answers), nothing else.
pub fn reply_code(query: &[u8], rcode: u8) -> Option<Vec<u8>> {
    if query.len() < dns::HEADER || query[dns::FLAGS] & dns::QR != 0 {
        return None; // too short, or itself a response
    }
    let mut out = query.to_vec();
    out[dns::FLAGS] = dns::QR | (query[dns::FLAGS] & dns::OPCODE_AND_RD);
    out[dns::CODES] = dns::RA | (rcode & dns::RCODE);
    // Keep the question; no answers, authority or additional records.
    out[dns::RECORD_COUNTS].fill(0);
    let mut i = dns::HEADER;
    let questions = u16::from_be_bytes([query[dns::QUESTIONS], query[dns::QUESTIONS + 1]]);
    for _ in 0..questions {
        // Labels (a length, then that many bytes) up to the name's end.
        while *out.get(i)? != 0 {
            i += usize::from(out[i]) + 1;
        }
        i += 1 + dns::QUESTION_FIELDS;
    }
    out.truncate(i.min(out.len()));
    Some(out)
}

/// Answer on ADDR:53 until the socket fails. Windows: craked's DNS server
/// answers for this link (`sys::windows::dns_queue`).
pub async fn serve() -> Result<()> {
    if cfg!(windows) {
        return Ok(());
    }
    let sock = UdpSocket::bind((ADDR, dns::PORT)).await?;
    let mut buf = vec![0u8; dns::BUFFER];
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
