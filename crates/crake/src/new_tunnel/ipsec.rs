//! New tunnel's IPsec (IKEv2) parts (design/proposals/ipsec-tunnel.html,
//! chosen by the user 2026-10-19): the gateway's key and the company's
//! domains in step 1, under the gateway, on the Name and Gateway grid; in
//! step 2 a plain "Username and password" card, as OpenVPN's "As the
//! profile says", with the gateway's identity under Advanced. The key is
//! sent once and kept by craked alone (`profiles/<name>.ipsec`): editing
//! shows it as kept, with Change….

use super::*;
use gpui_kit::App;

/// What IPsec asks for beyond the gateway.
pub(super) struct Ipsec {
    pub(super) key: Entity<InputState>,
    pub(super) domains: Entity<InputState>,
    pub(super) identity: Entity<InputState>,
    /// The key shown as typed (Show), not as dots.
    shown: bool,
    /// Editing a tunnel whose key craked keeps, and it isn't being changed.
    kept: bool,
    /// Editing: the tunnel was IPsec already (its key is kept).
    had_key: bool,
    /// The install command was copied, and when ("Copied" for a while).
    copied: Option<std::time::Instant>,
    /// The gateway proves itself with its certificate, not a key
    /// (ipsec-certificates.html, 1; `ipsec_proof`).
    pub(super) certificate: bool,
    /// IT's certificate authority picked here: its file's name, and PEM.
    pub(super) ca: Option<(String, String)>,
    /// Editing: craked keeps IT's authority for it.
    pub(super) ca_kept: bool,
    /// The picked file isn't an authority, or couldn't be read.
    pub(super) ca_problem: Option<String>,
}

/// What an edited IPsec tunnel has (craked's, never the secrets): a key,
/// or a certificate and whether IT's authority is kept.
#[derive(Default, Clone, Copy)]
pub(super) struct Kept {
    pub(super) key: bool,
    pub(super) certificate: bool,
    pub(super) ca: bool,
}

/// How long "Copied" shows after the install command is copied.
const COPIED_FOR: Duration = Duration::from_secs(3);

impl Ipsec {
    /// `domains`: the tunnel's, as AddTunnel's text gives them (a line each).
    pub(super) fn new(
        domains: &str,
        kept: Kept,
        window: &mut Window,
        cx: &mut Context<NewTunnel>,
    ) -> Self {
        let had_key = kept.key;
        let make = |value: String,
                    placeholder: &'static str,
                    masked: bool,
                    window: &mut Window,
                    cx: &mut Context<NewTunnel>| {
            cx.new(|cx| {
                let mut s = InputState::new(window, cx)
                    .placeholder(placeholder)
                    .masked(masked);
                s.set_value(value, window, cx);
                s
            })
        };
        let domains = domains.split_whitespace().collect::<Vec<_>>().join(" ");
        Self {
            key: make(String::new(), "", true, window, cx),
            domains: make(domains, "acme.internal", false, window, cx),
            identity: make(String::new(), "", false, window, cx),
            shown: false,
            kept: had_key,
            had_key,
            copied: None,
            certificate: kept.certificate,
            ca: None,
            ca_kept: kept.ca,
            ca_problem: None,
        }
    }

    /// What's missing or wrong in step 1, when Continue is pressed (never
    /// before: the key's help line says where it comes from).
    pub(super) fn problem(&self, cx: &App) -> Option<String> {
        if self.certificate {
            if let Some(p) = &self.ca_problem {
                return Some(p.clone());
            }
        } else if !self.kept && self.key.read(cx).value().trim().is_empty() {
            return Some(
                "Add the gateway\u{2019}s key, or choose Its certificate if IT gave none.".into(),
            );
        }
        self.domain_list(cx)
            .into_iter()
            .find(|d| !looks_like_domain(d))
            .map(|d| {
                format!(
                    "\u{201c}{d}\u{201d} isn\u{2019}t a domain. Just the domain, like nordbank.internal; several are separated by spaces."
                )
            })
    }

    fn domain_list(&self, cx: &App) -> Vec<String> {
        self.domains
            .read(cx)
            .value()
            .split([' ', ',', '\n'])
            .map(str::trim)
            .filter(|d| !d.is_empty())
            .map(str::to_owned)
            .collect()
    }

    /// AddTunnel's IPsec fields: the domains, and the key and the gateway's
    /// identity when they were typed (craked keeps them; never sent again).
    pub(super) fn fields(&self, f: &mut HashMap<String, String>, cx: &App) {
        f.insert("domains".into(), self.domain_list(cx).join("\n"));
        let id = self.identity.read(cx).value().trim().to_owned();
        if self.certificate {
            // IT's authority when one was picked; a kept one stays.
            f.insert("gateway_proof".into(), "certificate".into());
            if let Some((_, pem)) = &self.ca {
                f.insert("gateway_ca".into(), pem.clone());
            }
            if !id.is_empty() {
                f.insert("gateway_id".into(), id);
            }
            return;
        }
        f.insert("gateway_proof".into(), "key".into());
        if self.kept {
            return;
        }
        f.insert(
            "gateway_key".into(),
            self.key.read(cx).value().trim().to_owned(),
        );
        if !id.is_empty() {
            f.insert("gateway_id".into(), id);
        }
    }

    /// The text fields in step 1, in the order Tab goes through them.
    pub(super) fn tab_fields(&self) -> Vec<Entity<InputState>> {
        if self.kept || self.certificate {
            vec![self.domains.clone()]
        } else {
            vec![self.key.clone(), self.domains.clone()]
        }
    }
}

/// A domain as a person types it: letters, digits, hyphens and dots, with a
/// dot inside (craked checks it again, `OwnTunnel::from_map`).
fn looks_like_domain(d: &str) -> bool {
    d.contains('.')
        && !d.starts_with('.')
        && !d.ends_with('.')
        && d.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
}

impl NewTunnel {
    /// IPsec's key, domains and gateway identity fields (the IPsec
    /// journey, Linux's only).
    #[cfg(all(test, target_os = "linux"))]
    pub(crate) fn ipsec_fields_for_tests(
        &self,
    ) -> (Entity<InputState>, Entity<InputState>, Entity<InputState>) {
        (
            self.ipsec.key.clone(),
            self.ipsec.domains.clone(),
            self.ipsec.identity.clone(),
        )
    }

    /// strongSwan's install command, for this distribution, with Copy as
    /// the leftover notice has it (DESIGN.md, Small actions and links).
    pub(super) fn ipsec_install(&self, t: &Theme, cx: &mut Context<Self>) -> Div {
        let release = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
        let command = crate::engines::strongswan_install(&release);
        let copied = self.ipsec.copied.is_some_and(|c| c.elapsed() < COPIED_FOR);
        div()
            .mt(px(8.))
            .flex()
            .items_stretch()
            .rounded(px(6.))
            .bg(t.surface)
            .overflow_hidden()
            .text_size(px(13.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .px(px(12.))
                    .py(px(8.))
                    .font_family(fonts::MONO)
                    .child(command.clone()),
            )
            .child(
                div()
                    .id("ipsec-copy")
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .px(px(12.))
                    .border_l_1()
                    .border_color(t.line)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(if copied { t.muted } else { t.ink })
                    .cursor_pointer()
                    .child(
                        svg()
                            .path(if copied {
                                "crake/check.svg"
                            } else {
                                "crake/clipboard.svg"
                            })
                            .size(px(14.))
                            .text_color(if copied { t.muted } else { t.ink }),
                    )
                    .child(if copied { "Copied" } else { "Copy" })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(command.clone()));
                        this.ipsec.copied = Some(std::time::Instant::now());
                        cx.notify();
                        cx.spawn(async move |this, cx| {
                            cx.background_executor().timer(COPIED_FOR).await;
                            let _ = this.update(cx, |_, cx| cx.notify());
                        })
                        .detach();
                    }))
                    .test_support(),
            )
    }

    /// Step 1's key and domains, under the protocol (the list folds once
    /// it's chosen, so they sit just below the gateway).
    pub(super) fn ipsec_server(&self, t: &Theme, cx: &mut Context<Self>) -> Div {
        let ipsec = &self.ipsec;
        let key_action = if ipsec.kept {
            crate::controls::action("ipsec-change", "Change\u{2026}", t)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.ipsec.kept = false;
                    cx.notify();
                }))
                .test_support()
        } else {
            crate::controls::action("ipsec-show", if ipsec.shown { "Hide" } else { "Show" }, t)
                .on_click(cx.listener(|this, _, window, cx| {
                    this.ipsec.shown = !this.ipsec.shown;
                    let masked = !this.ipsec.shown;
                    this.ipsec
                        .key
                        .update(cx, |s, cx| s.set_masked(masked, window, cx));
                    cx.notify();
                }))
                .test_support()
        };
        let key_field = if ipsec.kept {
            div()
                .h(px(42.))
                .px(px(12.))
                .flex()
                .items_center()
                .rounded(px(6.))
                .border_1()
                .border_color(t.line)
                .bg(t.surface)
                .text_color(t.muted)
                .child("Kept by Crake\u{2019}s service")
        } else {
            field(&ipsec.key, ipsec.shown, t)
        };
        let help = |text: &'static str| {
            div()
                .mt(px(10.))
                .text_size(px(13.))
                .line_height(px(19.))
                .text_color(t.muted)
                .child(text)
        };
        let key_help = if ipsec.kept {
            None
        } else if ipsec.had_key {
            Some(
                help("The new key replaces the kept one when you save.").child(
                    crate::controls::action("ipsec-keep", "Keep the old key", t)
                        .mt(px(4.))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.ipsec.kept = true;
                            this.ipsec
                                .key
                                .update(cx, |s, cx| s.set_value(String::new(), window, cx));
                            cx.notify();
                        }))
                        .test_support(),
                ),
            )
        } else {
            Some(help(
                "From your IT team, who may call it the pre-shared key. Only Crake\u{2019}s service can read it.",
            ))
        };
        // The key's slot takes IT's authority when the gateway has a
        // certificate (ipsec-certificates.html, 1): nothing below moves.
        let slot = if ipsec.certificate {
            self.ipsec_authority(t, cx)
        } else {
            div()
                .flex_basis(px(0.))
                .flex_grow(1.)
                .min_w_0()
                .child(
                    div()
                        .flex()
                        .items_baseline()
                        .justify_between()
                        .child(label("Gateway\u{2019}s key"))
                        .child(key_action),
                )
                .child(key_field)
                .children(key_help)
        };
        let grid = div()
            .flex()
            .gap(px(14.))
            .mt(px(20.))
            .child(slot)
            .child(
                div()
                    .flex_basis(px(0.))
                    .flex_grow(super::server::WIDE_COLUMN)
                    .min_w_0()
                    .child(
                        div()
                            .flex()
                            .items_baseline()
                            .justify_between()
                            .child(label("Company domains"))
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(t.muted)
                                    .child("optional"),
                            ),
                    )
                    .child(field(&ipsec.domains, true, t))
                    .child(help(
                        "Names under them go to the tunnel\u{2019}s DNS. Without them, Crake asks public DNS first and finds those names a moment later.",
                    )),
            );
        div().child(self.ipsec_proof(t, cx)).child(grid)
    }

    /// Step 2: a username and password, the gateway asking for them over
    /// EAP as it asks FortiClient; the gateway's identity under Advanced
    /// (with the key: a kept one keeps its identity).
    pub(super) fn ipsec_sign_in(&self, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let name = self.display_name(cx);
        // With a certificate, what's checked when it's left empty is new
        // (ipsec-certificates.html, 2); with a key, today's words.
        let gateway = self.gateway.read(cx).value().trim().to_owned();
        let identity_help = if self.ipsec.certificate && !gateway.is_empty() {
            format!(
                "Only if your IT team gave one, like vpn.nordbank.example. Without it, the gateway\u{2019}s certificate must be for {gateway}."
            )
        } else {
            "Only if your IT team gave one, like vpn.nordbank.example. Crake then signs in only to a gateway that proves it\u{2019}s that one.".to_owned()
        };
        let asks = if name.is_empty() {
            "The gateway asks for them when you switch it on.".to_owned()
        } else {
            format!("{name}\u{2019}s gateway asks for them when you switch it on.")
        };
        div()
            .child(heading("How do you sign in?"))
            .child(
                div()
                    .mt(px(18.))
                    .py(px(14.))
                    .px(px(16.))
                    .rounded(px(8.))
                    .bg(t.surface)
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Username and password"),
                    )
                    .child(div().text_color(t.muted).child(asks)),
            )
            .child(
                div().mt(px(18.)).child(label("Username")).child(
                    div()
                        .max_w(px(320.))
                        .child(field(&self.user, false, t).text_size(px(15.))),
                ),
            )
            .when(!self.ipsec.kept, |d| {
                d.child(
                    div()
                        .mt(px(22.))
                        .pt(px(14.))
                        .border_t_1()
                        .border_color(t.line)
                        .child(
                            div()
                                .id("advanced")
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .cursor_pointer()
                                .child(
                                    svg()
                                        .path(if self.advanced {
                                            "crake/disclose-down.svg"
                                        } else {
                                            "crake/disclose-right.svg"
                                        })
                                        .size(px(10.))
                                        .text_color(t.ink),
                                )
                                .child("Advanced")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.advanced = !this.advanced;
                                    cx.notify();
                                }))
                                .test_support(),
                        )
                        .when(self.advanced, |d| {
                            d.child(
                                div()
                                    .mt(px(12.))
                                    .max_w(px(420.))
                                    .child(
                                        div()
                                            .flex()
                                            .items_baseline()
                                            .justify_between()
                                            .child(label("Gateway\u{2019}s identity"))
                                            .child(
                                                div()
                                                    .text_size(px(13.))
                                                    .text_color(t.muted)
                                                    .child("optional"),
                                            ),
                                    )
                                    .child(field(&self.ipsec.identity, true, t))
                                    .child(
                                        div()
                                            .mt(px(10.))
                                            .text_size(px(13.))
                                            .line_height(px(19.))
                                            .text_color(t.muted)
                                            .child(identity_help.clone()),
                                    ),
                            )
                        }),
                )
            })
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::looks_like_domain;

    #[test]
    fn what_a_domain_looks_like() {
        assert!(looks_like_domain("nordbank.internal"));
        assert!(looks_like_domain("ux.acme-1.example"));
        assert!(!looks_like_domain("nordbank"));
        assert!(!looks_like_domain("https://nordbank.internal"));
        assert!(!looks_like_domain(".internal"));
    }
}
