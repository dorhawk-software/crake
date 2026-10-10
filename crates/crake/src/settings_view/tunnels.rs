//! Settings, Tunnels: the tunnels in the order traffic tries them, drag to
//! reorder, edit, remove, open detail.

use super::*;

impl Settings {
    /// Settings → Tunnels (design/proposals/settings-tunnels.html): every
    /// tunnel in the order Crake tries them, then the full tunnels that take
    /// everything else; Edit and Remove… for those added here.
    pub(super) fn tunnels(
        &self,
        model: &Model,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let tried: Vec<crate::client::Tunnel> = model.try_order().into_iter().cloned().collect();
        let full: Vec<crate::client::Tunnel> = model
            .tunnels
            .iter()
            .filter(|x| x.full_tunnel)
            .cloned()
            .collect();
        let names: Vec<String> = tried.iter().map(|x| x.name.clone()).collect();
        let mut list = group(t).mt(px(22.));
        if tried.is_empty() && full.is_empty() {
            list = list.child(
                div()
                    .py(px(14.))
                    .px(px(16.))
                    .text_color(t.muted)
                    .child("No tunnels yet. Add the first one below."),
            );
        }
        for (i, tunnel) in tried.iter().enumerate() {
            let names = names.clone();
            let hover = t.accent_soft;
            list = list.child(
                self.tunnel_row(tunnel, Some(i), t, cx)
                    .when(i > 0, |r| r.border_t_1().border_color(t.line))
                    .on_drag(
                        Dragged {
                            from: i,
                            label: tunnel.display.clone(),
                            t: *t,
                        },
                        |d, _, _, cx| cx.new(|_| d.clone()),
                    )
                    .drag_over::<Dragged>(move |s, _, _, _| s.bg(hover))
                    .on_drop(cx.listener(move |this, d: &Dragged, _, _| {
                        let mut names = names.clone();
                        let moved = names.remove(d.from);
                        names.insert(i, moved);
                        this.set("order", names.join("\n"));
                    })),
            );
            list = list.children(self.confirm_remove(tunnel, t, cx));
        }
        if !full.is_empty() {
            list = list.child(
                div()
                    .pt(px(10.))
                    .pb(px(6.))
                    .px(px(16.))
                    .when(!tried.is_empty(), |d| d.border_t_1().border_color(t.line))
                    .text_size(px(12.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(t.muted)
                    .child("Everything else goes through:"),
            );
            for tunnel in &full {
                list = list.child(self.tunnel_row(tunnel, None, t, cx));
                list = list.children(self.confirm_remove(tunnel, t, cx));
            }
        }
        div()
            .child(list)
            .child(
                div().flex().child(
                    div()
                        .id("new-tunnel")
                        .mt(px(16.))
                        .h(px(38.))
                        .px(px(14.))
                        .flex()
                        .items_center()
                        .rounded(px(6.))
                        .bg(t.accent)
                        .text_color(t.accent_ink)
                        .font_weight(FontWeight::BOLD)
                        .cursor_pointer()
                        .child("+ New tunnel")
                        .on_click(cx.listener(|this, _, _, _| {
                            this.requests.send_blocking(Request::NewTunnel(None)).ok();
                        })),
                ),
            )
            .when(tried.len() > 1, |d| {
                d.child(div().mt(px(10.)).child(sub("Drag to change the order.", t)))
            })
    }

    /// One tunnel: grip and number (when it's tried in order), name, gateway
    /// and protocol, and who may change it.
    pub(super) fn tunnel_row(
        &self,
        tunnel: &crate::client::Tunnel,
        position: Option<usize>,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let protocol = match tunnel.protocol.as_str() {
            "anyconnect" => "AnyConnect",
            "gp" => "GlobalProtect",
            "pulse" => "Ivanti Connect Secure",
            "nc" => "Network Connect",
            "fortinet" => "Fortinet SSL VPN",
            "f5" => "F5 BIG-IP",
            "array" => "Array Networks",
            "openvpn3" => "OpenVPN 3",
            other => other,
        }
        .to_owned();
        let meta = if tunnel.gateway.is_empty() {
            protocol
        } else {
            format!("{} \u{b7} {protocol}", tunnel.gateway)
        };
        let right: AnyElement = match tunnel.source.as_str() {
            "app" => {
                let (edit, remove) = (tunnel.name.clone(), tunnel.name.clone());
                div()
                    .flex()
                    .gap(px(8.))
                    .flex_none()
                    .child(
                        small_button(
                            ("edit", position.unwrap_or(99)),
                            "Edit",
                            None,
                            t.line,
                            t.ink,
                            t,
                        )
                        .on_click(cx.listener(move |this, _, _, _| {
                            this.requests
                                .send_blocking(Request::NewTunnel(Some(edit.clone())))
                                .ok();
                        })),
                    )
                    .child(
                        small_button(
                            ("remove", position.unwrap_or(99)),
                            "Remove\u{2026}",
                            Some("crake/trash.svg"),
                            t.danger,
                            t.danger,
                            t,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.removing = Some(remove.clone());
                            cx.notify();
                        })),
                    )
                    .into_any_element()
            }
            "managed" => div()
                .flex_none()
                .text_size(px(13.))
                .text_color(t.muted)
                .child(format!("Set up by {}", tunnel.organisation))
                .into_any_element(),
            _ => div()
                .flex_none()
                .text_size(px(13.))
                .text_color(t.muted)
                .child("From the config file")
                .into_any_element(),
        };
        div()
            .id(gpui_kit::ElementId::Name(format!("tunnel-{}", tunnel.name).into()))
            .flex()
            .items_center()
            .gap(px(12.))
            .py(px(12.))
            .px(px(16.))
            .when(position.is_some(), |r| r.cursor_grab())
            .child(match position {
                Some(_) => svg()
                    .path("crake/grip.svg")
                    .size(px(16.))
                    .flex_none()
                    .text_color(t.muted)
                    .into_any_element(),
                None => div().w(px(16.)).flex_none().into_any_element(),
            })
            .child(
                div()
                    .w(px(10.))
                    .flex_none()
                    .font_family(fonts::MONO)
                    .text_size(px(12.))
                    .text_color(t.muted)
                    .child(position.map(|i| (i + 1).to_string()).unwrap_or_default()),
            )
            .child(
                // Its name and gateway open tunnel detail.
                div()
                    .id(gpui_kit::ElementId::Name(format!("open-{}", tunnel.name).into()))
                    .flex_1()
                    .min_w_0()
                    .cursor_pointer()
                    .on_click({
                        let name = tunnel.name.clone();
                        cx.listener(move |this, _, _, _| {
                            this.requests
                                .send_blocking(Request::OpenDetail(name.clone()))
                                .ok();
                        })
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                div()
                                    .text_size(px(15.))
                                    .font_weight(FontWeight::BOLD)
                                    .child(tunnel.display.clone()),
                            )
                            .when(tunnel.managed, |d| {
                                d.child(
                                    div()
                                        .px(px(7.))
                                        .py(px(1.))
                                        .rounded(px(4.))
                                        .bg(t.surface)
                                        .text_size(px(12.))
                                        .line_height(px(16.))
                                        .text_color(t.muted)
                                        .child("Managed"),
                                )
                            })
                            .when(crate::new_tunnel::beta(&tunnel.protocol), |d| {
                                d.child(crate::new_tunnel::beta_tag(t))
                            }),
                    )
                    .child(
                        div()
                            .mt(px(1.))
                            .truncate()
                            .font_family(fonts::MONO)
                            .text_size(px(12.))
                            .text_color(t.muted)
                            .child(meta),
                    )
                    .when(position.is_none(), |d| {
                        d.child(sub(
                            "A full tunnel: while it\u{2019}s on, whatever the tunnels above don\u{2019}t take goes through it.",
                            t,
                        ))
                    }),
            )
            .child(right)
            // Opens tunnel detail too: says the row leads somewhere.
            .child(
                div()
                    .id(gpui_kit::ElementId::Name(format!("chevron-{}", tunnel.name).into()))
                    .size(px(28.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(6.))
                    .cursor_pointer()
                    .hover(|s| s.bg(t.surface))
                    .child(
                        svg()
                            .path("crake/chevron-right.svg")
                            .size(px(16.))
                            .text_color(t.muted),
                    )
                    .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation()
                    })
                    .on_click({
                        let name = tunnel.name.clone();
                        cx.listener(move |this, _, _, _| {
                            this.requests
                                .send_blocking(Request::OpenDetail(name.clone()))
                                .ok();
                        })
                    }),
            )
    }

    /// "Remove Home lab?" under its row, after Remove… was pressed.
    pub(super) fn confirm_remove(
        &self,
        tunnel: &crate::client::Tunnel,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        (self.removing.as_deref() == Some(tunnel.name.as_str())).then(|| {
            let name = tunnel.name.clone();
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .py(px(12.))
                .px(px(16.))
                .border_t_1()
                .border_color(t.line)
                .bg(t.surface)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .font_weight(FontWeight::BOLD)
                                .child(format!("Remove {}?", tunnel.display)),
                        )
                        .child(sub(
                            "What Crake learned for it and its saved sign-in go too.",
                            t,
                        )),
                )
                .child(
                    small_button("keep-tunnel", "Keep it", None, t.line, t.ink, t).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.removing = None;
                            cx.notify();
                        }),
                    ),
                )
                .child(
                    small_button(
                        "remove-tunnel",
                        "Remove",
                        Some("crake/trash.svg"),
                        t.danger,
                        t.panel,
                        t,
                    )
                    .bg(t.danger)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.requests
                            .send_blocking(Request::RemoveTunnel(name.clone()))
                            .ok();
                        this.removing = None;
                        cx.notify();
                    })),
                )
        })
    }
}
