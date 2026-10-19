//! Diagnostics (design/proposals/report-in-settings.html, A): the report,
//! out of everyday view, where someone helping you would send you
//! ("Settings, Diagnostics, Save a report"). Saving keeps the button as it
//! is (nothing in the row moves); what's happening, then how it went, is
//! said in a box under the row until the window closes.

use super::*;

/// After this long, saving says how long it's been (as New tunnel's
/// "Asking the gateway… 6 s"): a slow save on a broken network doesn't
/// look stuck.
const SAY_SECONDS_AFTER: std::time::Duration = std::time::Duration::from_secs(2);

/// The report's state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) enum Report {
    #[default]
    None,
    /// craked is being asked, since then (a few seconds; longer when the
    /// internet is what's broken).
    Saving(std::time::Instant),
    /// Written: the file.
    Saved(std::path::PathBuf),
    /// Not written: the system's reason.
    Failed(String),
}

impl Report {
    /// CRAKE_SHOW=settings:diagnostics:saved / :saving (development).
    pub(super) fn shown() -> Self {
        match std::env::var("CRAKE_SHOW").as_deref() {
            Ok("settings:diagnostics:saved") => Self::Saved(
                crate::report::default_folder().join(crate::report::file_name(&jiff::Zoned::now())),
            ),
            Ok("settings:diagnostics:saving") => Self::Saving(std::time::Instant::now()),
            _ => Self::None,
        }
    }
}

/// How to make the same file from a terminal, on this system (neither
/// Windows nor macOS puts `crake` on the PATH).
fn in_a_terminal() -> &'static str {
    if cfg!(windows) {
        "\"C:\\Program Files\\Crake\\crake.exe\" report"
    } else if cfg!(target_os = "macos") {
        "/Applications/Crake.app/Contents/MacOS/crake report"
    } else {
        "crake report"
    }
}

impl Settings {
    pub(super) fn diagnostics(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let hide = cx.global::<Prefs>().hide_hosts;
        let hosts = if hide {
            "The places you connected to are left out (Privacy & log)."
        } else {
            "It names the places you connected to; Privacy & log can leave them out."
        };
        let saving = matches!(self.report, Report::Saving(_));
        let button = outline(
            "report",
            "Save a report\u{2026}",
            t.line,
            if saving { t.muted } else { t.ink },
            None,
            None,
        )
        .when(saving, |b| b.cursor_default())
        .keys()
        .on_click(cx.listener(|this, _, _, cx| this.save_report(cx)))
        .test_support();
        div()
            .child(
                group(t).mt(px(22.)).child(
                    div()
                        .py(px(14.))
                        .px(px(16.))
                        .child(
                            div()
                                .flex()
                                .items_start()
                                .gap(px(16.))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .child(
                                            div()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child("Save a report for someone helping you"),
                                        )
                                        .child(sub(
                                            "A text file showing how this computer connects to networks (Crake\u{2019}s rules among it), whether the internet answers, and Crake\u{2019}s logs.",
                                            t,
                                        ))
                                        .child(div().mt(px(4.)).child(sub(
                                            &format!(
                                                "Never passwords, sign-ins or keys. {hosts} It stays on this computer: open it and read it before you send it."
                                            ),
                                            t,
                                        ))),
                                )
                                .child(div().flex_none().child(button)),
                        )
                        .children(self.report_status(t)),
                ),
            )
            .child(
                div()
                    .mt(px(10.))
                    .flex()
                    .flex_wrap()
                    .gap(px(6.))
                    .text_size(px(13.))
                    .text_color(t.muted)
                    .child("Or in a terminal:")
                    .child(
                        div()
                            .font_family(fonts::MONO)
                            .child(in_a_terminal()),
                    ),
            )
    }

    /// The box under the row: the ring turning and what's happening, then
    /// the file with Open and Show in folder, or why it wasn't written.
    fn report_status(&self, t: &Theme) -> Option<impl IntoElement> {
        let (icon, said, more, path): (AnyElement, String, Option<String>, _) = match &self.report {
            Report::None => return None,
            Report::Saving(since) => (
                crate::head::under_way(t.ink, *since).into_any_element(),
                saving_said(since.elapsed()),
                Some("Crake is looking at this computer and trying the internet.".to_owned()),
                None,
            ),
            Report::Saved(path) => {
                let file = path.file_name().map(|f| f.to_string_lossy().into_owned());
                let folder = path
                    .parent()
                    .and_then(|p| p.file_name())
                    .map(|f| f.to_string_lossy().into_owned());
                let said = match (file, folder) {
                    (Some(f), Some(d)) => format!("Saved {f} in {d}."),
                    (Some(f), None) => format!("Saved {f}."),
                    _ => "Saved.".to_owned(),
                };
                (
                    icon("crake/file.svg", t.ink).into_any_element(),
                    said,
                    None,
                    Some(path.clone()),
                )
            }
            Report::Failed(why) => (
                icon("crake/cross.svg", t.danger).into_any_element(),
                "Couldn\u{2019}t save the report.".to_owned(),
                Some(why.clone()),
                None,
            ),
        };
        let link = |id: &'static str, label: &'static str| {
            div()
                .id(id)
                .font_weight(FontWeight::SEMIBOLD)
                .underline()
                .cursor_pointer()
                .child(label)
        };
        Some(
            div()
                .id("report-status")
                .w_full()
                .mt(px(10.))
                .py(px(10.))
                .px(px(12.))
                .rounded(px(6.))
                .bg(t.surface)
                .flex()
                .flex_wrap()
                .items_center()
                .gap_x(px(10.))
                .gap_y(px(6.))
                .text_size(px(13.))
                .text_color(t.ink)
                .child(icon)
                .child(
                    div()
                        .id("report-said")
                        .flex_none()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(said)
                        .test_support(),
                )
                .when_some(more, |b, more| {
                    b.child(div().flex_1().min_w_0().text_color(t.muted).child(more))
                })
                .when_some(path, |b, path| {
                    let shown = path.clone();
                    b.child(
                        link("report-open", "Open")
                            .keys()
                            .on_click(move |_, _, cx| cx.open_with_system(&path))
                            .test_support(),
                    )
                    .child(
                        link("report-show", "Show in folder")
                            .keys()
                            .on_click(move |_, _, cx| cx.reveal_path(&shown))
                            .test_support(),
                    )
                })
                .test_support(),
        )
    }

    /// Save a report…: where to, then craked asked (`report::gather`),
    /// then the file, saying how it went under the row.
    fn save_report(&mut self, cx: &mut Context<Self>) {
        if matches!(self.report, Report::Saving(_)) {
            return;
        }
        let now = jiff::Zoned::now();
        let chosen = cx.prompt_for_new_path(
            &crate::report::default_folder(),
            Some(&crate::report::file_name(&now)),
        );
        let hide = cx.global::<Prefs>().hide_hosts;
        let gatherer = cx.try_global::<crate::report::Gatherer>().cloned();
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(path))) = chosen.await else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                this.report = Report::Saving(std::time::Instant::now());
                cx.notify();
            });
            let gathered = gatherer
                .map_or_else(crate::report::gather_apart, |g| (g.0)())
                .recv()
                .await;
            let text = match gathered {
                Ok(g) => crate::report::text(&g, hide, &now),
                Err(_) => String::new(),
            };
            let done = if text.is_empty() {
                Report::Failed("Crake couldn\u{2019}t ask its service.".into())
            } else {
                match crate::report::save(&path, &text) {
                    Ok(()) => Report::Saved(path),
                    Err(why) => Report::Failed(why),
                }
            };
            let _ = this.update(cx, |this, cx| {
                this.report = done;
                cx.notify();
            });
        })
        .detach();
    }
}

fn icon(path: &'static str, colour: gpui_kit::Rgba) -> impl IntoElement {
    svg()
        .path(path)
        .size(px(16.))
        .flex_none()
        .text_color(colour)
}

/// "Saving the report…", and after a moment how long it's been.
fn saving_said(for_: std::time::Duration) -> String {
    if for_ < SAY_SECONDS_AFTER {
        "Saving the report\u{2026}".into()
    } else {
        format!("Saving the report\u{2026} {} s", for_.as_secs())
    }
}
