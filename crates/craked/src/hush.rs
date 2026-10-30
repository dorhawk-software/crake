//! "Keep internal names private" (Settings, Names): while a tunnel is off,
//! names under its domains must not go to the internet provider's DNS. A
//! dummy link, `crake-hush`, takes those domains in systemd-resolved as
//! routing domains; its DNS server is an address on that link where craked
//! answers every question with REFUSED, so such names fail at once and stay
//! on the machine. Linux: while a tunnel is up it also carries craked's
//! search domain for short names (`dns::SHORT_SUFFIX`).

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

/// The socket the hush answers on: ADDR:53, taken before the link is given
/// any domain, so a name is never sent there while nobody (or, worse, a
/// DNS server listening on every address) would answer it. Linux only:
/// on Windows and macOS craked's own DNS server answers for this link
/// (`sys::dns_links`), and the address isn't on the machine at all.
pub async fn bind() -> std::io::Result<Option<UdpSocket>> {
    if !cfg!(target_os = "linux") {
        return Ok(None);
    }
    UdpSocket::bind((ADDR, dns::PORT)).await.map(Some)
}

/// Answer on `sock` until it fails: REFUSED, but a short name (resolved
/// asks "expert" as expert.crake.internal, `dns::SHORT_SUFFIX` being this
/// link's search domain while a tunnel is up), which the manager answers
/// with the full name you chose for it, or else offers (`short_names`)
/// and says doesn't exist, so resolved tries the next search domain.
pub async fn serve(sock: UdpSocket, mgr: std::sync::Weak<crate::manager::Manager>) -> Result<()> {
    let sock = std::sync::Arc::new(sock);
    let mut buf = vec![0u8; dns::BUFFER];
    loop {
        let (n, from) = sock.recv_from(&mut buf).await?;
        let query = buf[..n].to_vec();
        let Some(mgr) = dns::short_question(&query).and(mgr.upgrade()) else {
            if let Some(reply) = refuse(&query) {
                let _ = sock.send_to(&reply, from).await;
            }
            continue;
        };
        let sock = std::sync::Arc::clone(&sock);
        tokio::spawn(async move {
            if let Some(reply) = mgr.answer_short(&query).await {
                let _ = sock.send_to(&reply, from).await;
            }
        });
    }
}

/// Why names aren't kept private now, in the user's words (Settings shows
/// it under "Keep internal names private").
pub fn why_not(bind: &std::io::Error) -> String {
    if bind.kind() == std::io::ErrorKind::AddrInUse {
        "Another DNS server on this computer answers on every address, so Crake can\u{2019}t answer for these names.".into()
    } else {
        format!("Crake couldn\u{2019}t answer for these names ({bind}).")
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
