//! New tunnel, step 1, OpenVPN 3: importing a .ovpn file as the user's
//! profile (design/proposals/ovpn-import.html, option A). The file picker
//! gives a file; the files it points to go inline (`ovpn_file`), openvpn3
//! takes it, and it's the chosen profile.

use std::path::PathBuf;

use super::*;

/// What the Profile field says under it after an import: how it went.
pub(super) enum ProfileNote {
    Imported(String),
    Failed(String),
}

impl NewTunnel {
    /// The label row's "Import .ovpn…", or the button there's no list
    /// without: the file picker.
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

    /// Import the .ovpn file at `path`: inline what it points to, hand it to
    /// openvpn3 under a name no profile has yet, and choose it.
    pub(super) fn import_ovpn(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let file = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let name = free_name(&crate::ovpn_file::profile_name(&path), &self.profiles);
        let task = cx.background_executor().spawn({
            let name = name.clone();
            async move {
                let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
                let dir = path.parent().map(PathBuf::from).unwrap_or_default();
                let config = crate::ovpn_file::inline(&text, &dir).map_err(|p| p.said())?;
                give_openvpn3(&name, &config)
                    .map_err(|e| format!("openvpn3 didn\u{2019}t take it ({e})."))
            }
        });
        cx.spawn(async move |this, cx| {
            let done = task.await;
            let _ = this.update(cx, |this, cx| {
                this.profile_note = Some(match done {
                    Ok(()) => {
                        if !this.profiles.contains(&name) {
                            this.profiles.push(name.clone());
                            this.profiles.sort();
                        }
                        this.profile = Some(name.clone());
                        this.problem = None;
                        ProfileNote::Imported(
                            if crate::ovpn_file::profile_name(&PathBuf::from(&file)) == name {
                                format!("Imported {file} as your profile.")
                            } else {
                                format!("Imported {file} as your profile \u{201c}{name}\u{201d}.")
                            },
                        )
                    }
                    Err(why) => {
                        ProfileNote::Failed(format!("Couldn\u{2019}t import {file}: {why}"))
                    }
                });
                cx.notify();
            });
        })
        .detach();
    }
}

/// `wanted`, or with " 2", " 3"... after it if a profile has that name
/// already (openvpn3 would keep both, and Crake finds profiles by name).
fn free_name(wanted: &str, taken: &[String]) -> String {
    if !taken.iter().any(|t| t == wanted) {
        return wanted.to_owned();
    }
    (2..)
        .map(|n| format!("{wanted} {n}"))
        .find(|c| !taken.contains(c))
        .unwrap_or_else(|| wanted.to_owned())
}

/// openvpn3's import; tests never touch this computer's openvpn3.
fn give_openvpn3(name: &str, config: &str) -> Result<(), String> {
    if cfg!(test) {
        // A test's file says "openvpn3 refuses" to see a refusal through.
        return if config.contains("openvpn3 refuses") {
            Err("Configuration file contains errors".into())
        } else {
            Ok(())
        };
    }
    crate::ovpn3::import(name, config)
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_name_no_profile_has() {
        let taken = vec!["initech".to_owned(), "initech 2".to_owned()];
        assert_eq!(super::free_name("globex", &taken), "globex");
        assert_eq!(super::free_name("initech", &taken), "initech 3");
    }
}
