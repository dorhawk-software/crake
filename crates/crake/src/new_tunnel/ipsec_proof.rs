//! New tunnel's step 1 for IPsec: how the gateway proves itself
//! (design/proposals/ipsec-certificates.html, 1, A, chosen by the user
//! 2026-10-21): two cards, "A key" and "Its certificate"; with the
//! certificate, the key's slot on the grid takes IT's certificate
//! authority, optional, so nothing below it moves.

use super::*;
use gpui_kit::App;

/// How a certificate starts in PEM; a DER file starts with a SEQUENCE.
const PEM_CERTIFICATE: &str = "-----BEGIN CERTIFICATE-----";
const DER_SEQUENCE: u8 = 0x30;
/// PEM's line length (RFC 7468).
const PEM_LINE: usize = 64;

/// IT's certificate authority from a file: PEM as it is, DER made PEM;
/// anything else isn't one.
pub(super) fn authority(bytes: &[u8]) -> Option<String> {
    use base64::Engine as _;
    if let Ok(text) = std::str::from_utf8(bytes)
        && text.contains(PEM_CERTIFICATE)
    {
        return Some(text.trim().to_owned());
    }
    if bytes.first() != Some(&DER_SEQUENCE) {
        return None;
    }
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    let lines: Vec<String> = b64
        .as_bytes()
        .chunks(PEM_LINE)
        .map(|l| String::from_utf8_lossy(l).into_owned())
        .collect();
    Some(format!(
        "{PEM_CERTIFICATE}\n{}\n-----END CERTIFICATE-----",
        lines.join("\n")
    ))
}

impl NewTunnel {
    /// The gateway as its certificate must name it: the identity typed in
    /// step 2, else the gateway.
    fn gateway_name(&self, cx: &App) -> String {
        let id = self.ipsec.identity.read(cx).value().trim().to_owned();
        if id.is_empty() {
            self.gateway.read(cx).value().trim().to_owned()
        } else {
            id
        }
    }

    /// What Crake checks of the gateway's certificate, under the cards.
    pub(super) fn proof_line(&self, cx: &App) -> String {
        let name = self.gateway_name(cx);
        if name.is_empty() {
            "Crake checks that its certificate is for the gateway, and asks you before trusting one signed by an authority this computer doesn\u{2019}t know.".to_owned()
        } else if name.parse::<std::net::IpAddr>().is_ok() {
            format!(
                "Its certificate must be for {name}. If your IT team gave the gateway a name, put it under Gateway\u{2019}s identity in the next step."
            )
        } else {
            format!(
                "Crake checks that its certificate is for {name}, and asks you before trusting one signed by an authority this computer doesn\u{2019}t know."
            )
        }
    }

    /// The two cards, and what the chosen one checks.
    pub(super) fn ipsec_proof(&self, t: &Theme, cx: &mut Context<Self>) -> Div {
        let name = self.display_name(cx);
        let title = if name.is_empty() {
            "How the gateway proves itself".to_owned()
        } else {
            format!("How the gateway proves it\u{2019}s {name}\u{2019}s")
        };
        let certificate = self.ipsec.certificate;
        div()
            .mt(px(22.))
            .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
            .child(
                div()
                    .mt(px(8.))
                    .flex()
                    .gap(px(10.))
                    .child(
                        card(
                            "ipsec-key",
                            !certificate,
                            "A key",
                            "From your IT team, who may call it the pre-shared key.",
                            None,
                            t,
                        )
                        .flex_1()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.ipsec.certificate = false;
                            this.problem = None;
                            cx.notify();
                        }))
                        .test_support(),
                    )
                    .child(
                        card(
                            "ipsec-certificate",
                            certificate,
                            "Its certificate",
                            "As a website does. If IT gave you no key, it\u{2019}s this.",
                            None,
                            t,
                        )
                        .flex_1()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.ipsec.certificate = true;
                            this.problem = None;
                            cx.notify();
                        }))
                        .test_support(),
                    ),
            )
            .when(certificate, |d| {
                d.child(
                    div()
                        .mt(px(10.))
                        .text_size(px(13.))
                        .line_height(px(19.))
                        .text_color(t.muted)
                        .child(self.proof_line(cx)),
                )
            })
    }

    /// The key's slot with "Its certificate" chosen: IT's authority, a file.
    pub(super) fn ipsec_authority(&self, t: &Theme, cx: &mut Context<Self>) -> Div {
        let ipsec = &self.ipsec;
        let name = self.display_name(cx);
        let chosen = ipsec.ca.as_ref().map(|(n, _)| n.clone());
        let kept = ipsec.ca_kept && chosen.is_none();
        let button = div()
            .id("ipsec-choose-ca")
            .h(px(36.))
            .px(px(12.))
            .flex()
            .items_center()
            .border_1()
            .border_color(t.line)
            .rounded(px(6.))
            .font_weight(FontWeight::SEMIBOLD)
            .cursor_pointer()
            .child(if chosen.is_some() || kept {
                "Choose another\u{2026}"
            } else {
                "Choose a file\u{2026}"
            })
            .on_click(cx.listener(|this, _, _, cx| this.choose_authority(cx)))
            .test_support();
        let help = if name.is_empty() {
            "Only if your IT team gave one. Crake then trusts it for this gateway alone, and doesn\u{2019}t ask.".to_owned()
        } else {
            format!(
                "Only if your IT team gave one. Crake then trusts it for {name} alone, and doesn\u{2019}t ask."
            )
        };
        div()
            .flex_basis(px(0.))
            .flex_grow(1.)
            .min_w_0()
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .justify_between()
                    .child(label("IT\u{2019}s certificate authority"))
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(t.muted)
                            .child("optional"),
                    ),
            )
            .child(
                div()
                    .mt(px(3.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(button)
                    .children(
                        chosen
                            .or(kept.then(|| "Kept by Crake\u{2019}s service".to_owned()))
                            .map(|n| {
                                div()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .text_size(px(13.))
                                    .text_color(t.muted)
                                    .when(!kept, |d| d.font_family(fonts::MONO))
                                    .child(n)
                            }),
                    ),
            )
            .child(
                div()
                    .mt(px(13.))
                    .text_size(px(13.))
                    .line_height(px(19.))
                    .text_color(if ipsec.ca_problem.is_some() {
                        t.ink
                    } else {
                        t.muted
                    })
                    .child(ipsec.ca_problem.clone().unwrap_or(help)),
            )
    }

    /// IT's certificate authority, from a file the user picks.
    fn choose_authority(&mut self, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose".into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let read = std::fs::read(&path);
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let _ = this.update(cx, |this, cx| {
                this.set_authority(&name, read, cx);
            });
        })
        .detach();
    }

    /// A picked file as IT's authority, or why it isn't one.
    pub(super) fn set_authority(
        &mut self,
        name: &str,
        read: std::io::Result<Vec<u8>>,
        cx: &mut Context<Self>,
    ) {
        match read.map(|b| authority(&b)) {
            Ok(Some(pem)) => {
                self.ipsec.ca = Some((name.to_owned(), pem));
                self.ipsec.ca_problem = None;
            }
            Ok(None) => {
                self.ipsec.ca_problem = Some(format!(
                    "\u{201c}{name}\u{201d} isn\u{2019}t a certificate. IT\u{2019}s authority is a .crt, .pem or .cer file."
                ));
            }
            Err(e) => {
                self.ipsec.ca_problem = Some(format!("Couldn\u{2019}t read that file: {e}"));
            }
        }
        cx.notify();
    }
}

impl NewTunnel {
    /// Step 2's "Certificate" (ipsec-certificates.html, 2): a file, as
    /// AnyConnect's "A file…" (`choose_certificate`), its copy kept in the
    /// keyring; editing, the kept one stays unless another is chosen.
    pub(super) fn ipsec_certificate_file(&self, t: &Theme, cx: &mut Context<Self>) -> Div {
        let chosen = self.cert_file.as_ref().map(|(n, _)| n.clone());
        let kept = self.ipsec.cert_kept && chosen.is_none();
        div().w(px(320.)).child(label("Certificate")).child(
            div()
                .mt(px(3.))
                .flex()
                .items_center()
                .gap(px(10.))
                .child(
                    div()
                        .id("ipsec-choose-cert")
                        .h(px(36.))
                        .px(px(12.))
                        .flex()
                        .flex_none()
                        .items_center()
                        .border_1()
                        .border_color(t.line)
                        .rounded(px(6.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .cursor_pointer()
                        .child(if chosen.is_some() || kept {
                            "Choose another\u{2026}"
                        } else {
                            "Choose a file\u{2026}"
                        })
                        .on_click(cx.listener(|this, _, _, cx| this.choose_certificate(cx)))
                        .test_support(),
                )
                .children(
                    chosen
                        .or(kept.then(|| "Kept in your keyring".to_owned()))
                        .map(|n| {
                            div()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .text_size(px(13.))
                                .text_color(t.muted)
                                .when(!kept, |d| d.font_family(fonts::MONO))
                                .child(n)
                        }),
                ),
        )
    }
}

/// Where CRAKE_SHOW's variant is: after "newtunnel", the step and the
/// protocol.
const VARIANT: usize = 3;

/// CRAKE_SHOW's IPsec variants in demo mode (development), as the
/// proposal drew them: "newtunnel:1:ipsec:certificate", the gateway with
/// its certificate and IT's authority picked; "…:certificate-password",
/// step 2's third card with a file chosen (returned, for `cert_file`).
pub(super) fn demo(
    shown: Option<&str>,
    ipsec: &mut super::ipsec::Ipsec,
    window: &mut Window,
    cx: &mut Context<NewTunnel>,
) -> Option<(String, Vec<u8>)> {
    let variant = shown?.split(':').nth(VARIANT)?;
    if !variant.starts_with("certificate") {
        return None;
    }
    ipsec.certificate = true;
    ipsec.ca = Some(("nordbank-ca.pem".into(), String::new()));
    ipsec.domains.update(cx, |f, cx| {
        f.set_value("nordbank.internal nordbank.example", window, cx)
    });
    (variant == "certificate-password").then(|| {
        ipsec.method = "certificate-password";
        ("nordbank-ondrej.p12".to_owned(), Vec::new())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_authority_is_pem_whatever_its_file_is() {
        let pem = "-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----\n";
        assert_eq!(authority(pem.as_bytes()).as_deref(), Some(pem.trim()));
        let der = [DER_SEQUENCE, 3, 1, 2, 3];
        let made = authority(&der).unwrap();
        assert!(made.starts_with(PEM_CERTIFICATE), "{made}");
        assert!(made.contains("MAMBAgM="), "{made}");
        assert_eq!(authority(b"client\nremote vpn.example 1194\n"), None);
        assert_eq!(authority(&[]), None);
    }
}
