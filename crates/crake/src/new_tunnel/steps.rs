//! New tunnel's steps: Tab between a step's fields, whether a step can be
//! left, the fields Add and Save send, and Next.

use std::collections::HashMap;

use gpui_kit::component::input::InputState;
use gpui_kit::{Context, Entity, Focusable, Window};

use super::{Close, Detect, LAST_STEP, NewTunnel};
use crate::Request;

/// Adding or saving while craked is out of reach.
pub(super) const NO_SERVICE: &str = "Crake\u{2019}s service isn\u{2019}t running, so this can\u{2019}t be saved yet. What you entered stays here: try again once it\u{2019}s back.";

impl NewTunnel {
    /// The text fields of the step on screen, in the order Tab goes
    /// through them.
    pub(super) fn tab_order(&self) -> Vec<Entity<InputState>> {
        match self.step {
            1 if self.protocol == "openvpn" => vec![self.name.clone()],
            1 if self.protocol == "ipsec" => [
                vec![self.name.clone(), self.gateway.clone()],
                self.ipsec.tab_fields(),
            ]
            .concat(),
            1 => vec![self.name.clone(), self.gateway.clone()],
            2 => vec![self.user.clone()],
            _ => Vec::new(),
        }
    }

    /// Tab to the step's next text field (`back`: Shift+Tab, the one
    /// before): nothing in GPUI moves the focus on Tab by itself here.
    pub(super) fn tab(&mut self, back: bool, window: &mut Window, cx: &mut Context<Self>) {
        let fields = self.tab_order();
        let Some(at) = fields
            .iter()
            .position(|f| f.read(cx).focus_handle(cx).is_focused(window))
        else {
            return;
        };
        let next = if back {
            (at + fields.len() - 1) % fields.len()
        } else {
            (at + 1) % fields.len()
        };
        fields[next].update(cx, |f, cx| f.focus(window, cx));
        cx.notify();
    }

    /// Can step `step` be left? Says what's missing if not.
    pub(super) fn check(&mut self, step: usize, cx: &Context<Self>) -> bool {
        if step == 2
            && self.method == "browser"
            && self.protocol != "openvpn"
            && let Some(why) = self.no_browser()
        {
            self.problem = Some(why);
            return false;
        }
        let problem = match step {
            1 if self.display_name(cx).is_empty() => Some("Give it a name."),
            1 if self.missing(self.protocol).is_some() => self.missing(self.protocol),
            // A new one needs its profile; an OpenVPN 3 one moving over too.
            1 if self.protocol == "openvpn"
                && self.ovpn.is_none()
                && self.editing_protocol != "openvpn" =>
            {
                Some("Which profile? Import its .ovpn file.")
            }
            1 if self.protocol != "openvpn" && self.gateway.read(cx).value().trim().is_empty() => {
                Some("Which gateway? Its name or address, like vpn.example.com.")
            }
            1 if self.protocol().is_none() => match self.detect {
                Detect::Asking(_) => Some("Still asking the gateway. A moment."),
                _ => Some("Pick the protocol: the gateway didn\u{2019}t say."),
            },
            2 if self.protocol == "ipsec"
                && super::ipsec::needs_password(self.ipsec.method)
                && self.user.read(cx).value().trim().is_empty() =>
            {
                Some("Your username, as your IT team gave it.")
            }
            2 if self.protocol == "ipsec"
                && self.ipsec.method != "password"
                && self.cert_file.is_none()
                && !self.ipsec.cert_kept =>
            {
                Some("Choose the certificate file.")
            }
            2 if self.method == "certificate"
                && self.cert_source == "file"
                && self.cert_file.is_none()
                && self.editing.is_none() =>
            {
                Some("Choose the certificate file.")
            }
            _ => None,
        };
        self.problem = problem.map(str::to_owned);
        if problem.is_none() && step == 1 && self.protocol == "ipsec" {
            self.problem = self.ipsec.problem(cx);
        }
        self.problem.is_none()
    }

    pub(super) fn fields(&self, cx: &Context<Self>) -> HashMap<String, String> {
        let mut f = HashMap::new();
        f.insert("display_name".into(), self.display_name(cx));
        // OpenVPN: the gateway its profile names, and the profile (craked
        // keeps it); changed only when one was chosen.
        let gateway = match (&self.ovpn, self.protocol) {
            (Some(o), "openvpn") => o.remote.clone(),
            (None, "openvpn") => self.kept_gateway.clone(),
            _ => self.gateway.read(cx).value().trim().to_owned(),
        };
        f.insert("gateway".into(), gateway);
        if let (Some(o), "openvpn") = (&self.ovpn, self.protocol) {
            f.insert("profile".into(), o.text.clone());
        }
        f.insert(
            "protocol".into(),
            self.protocol().unwrap_or("anyconnect").into(),
        );
        let group = match self.protocol {
            "openvpn" => String::new(),
            _ => self.group.clone().unwrap_or_default(),
        };
        f.insert("group".into(), group);
        // IPsec signs in with a username and password (EAP), its secrets
        // apart (`ipsec::Ipsec::fields`).
        let method = if self.protocol == "ipsec" {
            self.ipsec.method
        } else {
            self.method
        };
        f.insert("sign_in".into(), method.into());
        if self.protocol == "ipsec" {
            self.ipsec.fields(&mut f, cx);
        }
        f.insert("user".into(), self.user.read(cx).value().trim().to_owned());
        let certificate = match (method, self.cert_source) {
            // IPsec's is a file, kept in the keyring (no smart card yet).
            ("certificate" | "certificate-password", _) if self.protocol == "ipsec" => {
                "keyring".to_owned()
            }
            ("certificate", "pkcs11") => "pkcs11:".to_owned(),
            // A chosen file is kept in the keyring (7e).
            ("certificate", _) => "keyring".to_owned(),
            _ => String::new(),
        };
        f.insert("certificate".into(), certificate);
        f.insert("mode".into(), self.mode.into());
        f.insert("list".into(), self.list.join("\n"));
        f.insert("forget_after_days".into(), self.forget.clone());
        f.insert("weak_ciphers".into(), self.weak_ciphers.to_string());
        f.insert("client_version".into(), self.client_version.to_string());
        f.insert("keep_signed_in".into(), self.keep_signed_in.to_string());
        f
    }

    pub(super) fn next(&mut self, cx: &mut Context<Self>) {
        if !self.check(self.step, cx) {
            cx.notify();
            return;
        }
        if self.step < LAST_STEP {
            self.step += 1;
            cx.notify();
            return;
        }
        // Without craked the request would wait unseen and the window close
        // on nothing (a colleague's Windows, where an antivirus had kept it
        // from starting, 2026-10-26): it stays, and says why.
        if self.model.read(cx).reachable == Some(false) {
            self.problem = Some(NO_SERVICE.into());
            cx.notify();
            return;
        }
        let fields = self.fields(cx);
        let certificate = self.cert_file.as_ref().map(|(_, b)| b.clone());
        match &self.editing {
            Some(name) => {
                let _ = self.requests.send_blocking(Request::UpdateTunnel(
                    name.clone(),
                    fields,
                    certificate,
                ));
                if self.send_suggested {
                    let _ = self
                        .requests
                        .send_blocking(Request::Pin(name.clone(), self.suggested.clone()));
                }
            }
            None => {
                let _ = self
                    .requests
                    .send_blocking(Request::AddTunnel(fields, certificate));
            }
        }
        cx.emit(Close);
    }

    pub(super) fn choose_certificate(&mut self, cx: &mut Context<Self>) {
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
            let _ = this.update(cx, |this, cx| {
                match read {
                    Ok(bytes) => {
                        let name = path
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        this.cert_file = Some((name, bytes));
                        this.problem = None;
                    }
                    Err(e) => this.problem = Some(format!("Couldn\u{2019}t read that file: {e}")),
                }
                cx.notify();
            });
        })
        .detach();
    }
}
