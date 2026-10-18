//! New tunnel, step 1, OpenVPN (design/proposals/openvpn-tunnel.html): a
//! .ovpn file imported (from the Gateway label's row, which picks OpenVPN
//! by itself, or the Profile field's), or one of OpenVPN 3's profiles taken
//! over (Crake takes a copy; OpenVPN 3 keeps its own). The files a .ovpn
//! points to go inline (`ovpn_file`), and the profile is read against what
//! craked may run (`crake_proto::openvpn_profile`) the moment it's chosen.

use std::path::PathBuf;

use crake_proto::openvpn_profile;

use super::*;

/// What the Profile field says under it after an import: how it went.
pub(super) enum ProfileNote {
    Imported(String),
    Failed(String),
}

/// The OpenVPN profile chosen: its text (sent with AddTunnel), the gateway
/// it names, and whether it came from a file (else from OpenVPN 3).
#[derive(Clone, Debug)]
pub(super) struct Chosen {
    pub text: String,
    pub remote: String,
    pub from_file: bool,
}

impl NewTunnel {
    /// "Import .ovpn…" (beside Gateway or Profile), or the button there's no
    /// list without: the file picker.
    pub(super) fn choose_ovpn(&mut self, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Import".into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let _ = this.update(cx, |this, cx| this.import_ovpn(path, cx));
        })
        .detach();
    }

    /// Import the .ovpn file at `path`: inline what it points to, read it,
    /// and take it: OpenVPN, its gateway, and a name from the file if
    /// there's none yet.
    pub(super) fn import_ovpn(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let file = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let named = crate::ovpn_file::profile_name(&path);
        let task = cx.background_executor().spawn({
            let file = file.clone();
            async move {
                let text = std::fs::read_to_string(&path)
                    .map_err(|e| format!("Couldn\u{2019}t import {file}: {e}."))?;
                let dir = path.parent().map(PathBuf::from).unwrap_or_default();
                let text = crate::ovpn_file::inline(&text, &dir)
                    .map_err(|p| format!("Couldn\u{2019}t import {file}: {}", p.said()))?;
                let profile = openvpn_profile::check(&text).map_err(|r| r.said(&file))?;
                Ok::<_, String>(Chosen {
                    text,
                    remote: profile.remote,
                    from_file: true,
                })
            }
        });
        cx.spawn(async move |this, cx| {
            let done = task.await;
            let _ = this.update(cx, |this, cx| {
                this.profile_note = Some(match done {
                    Ok(chosen) => {
                        this.protocol = "openvpn";
                        this.profile = None;
                        this.ovpn = Some(chosen);
                        this.problem = None;
                        if this.display_name(cx).is_empty() {
                            this.pending_name = Some(title_case(&named));
                        }
                        ProfileNote::Imported(format!(
                            "From {file}. Crake keeps its own copy, which only its service can read, so you can delete the file."
                        ))
                    }
                    Err(why) => ProfileNote::Failed(why),
                });
                cx.notify();
            });
        })
        .detach();
    }

    /// One of OpenVPN 3's profiles chosen: read it (as the user, who owns
    /// it) and take a copy, unless it's locked.
    pub(super) fn take_over(&mut self, name: String, cx: &mut Context<Self>) {
        self.profile = Some(name.clone());
        self.ovpn = None;
        let task = cx
            .background_executor()
            .spawn(async move { from_openvpn3(&name) });
        cx.spawn(async move |this, cx| {
            let done = task.await;
            let _ = this.update(cx, |this, cx| {
                this.profile_note = Some(match done {
                    Ok(chosen) => {
                        let said = format!(
                            "It connects to {}. Crake takes its own copy; OpenVPN 3 keeps its own.",
                            chosen.remote
                        );
                        this.ovpn = Some(chosen);
                        this.problem = None;
                        ProfileNote::Imported(said)
                    }
                    Err(why) => ProfileNote::Failed(why),
                });
                cx.notify();
            });
        })
        .detach();
    }
}

/// An OpenVPN 3 profile taken over, or what to say instead.
fn from_openvpn3(name: &str) -> Result<Chosen, String> {
    use crate::openvpn::NotTaken;
    match crate::openvpn::from_openvpn3(name) {
        Ok((text, remote)) => Ok(Chosen {
            text,
            remote,
            from_file: false,
        }),
        Err(NotTaken::Locked) => Err(format!(
            "{name} is locked in OpenVPN 3: import its .ovpn file instead."
        )),
        Err(NotTaken::Missing) => Err(format!("OpenVPN 3 has no profile called {name} now.")),
        Err(NotTaken::Refused(said)) => Err(said),
        Err(NotTaken::Unreadable(e)) => {
            Err(format!("Couldn\u{2019}t read {name} from OpenVPN 3 ({e})."))
        }
    }
}

/// "initech-office" as a name for people: its first letter capital.
fn title_case(name: &str) -> String {
    let mut c = name.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_name_from_the_file() {
        assert_eq!(super::title_case("initech-office"), "Initech-office");
        assert_eq!(super::title_case(""), "");
    }
}
