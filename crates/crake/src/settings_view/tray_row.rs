//! Settings, General: the row that shows or hides the bird
//! (design/proposals/tray-setting-after.html). Each system's own word for
//! where the bird sits, a small bird drawn as that bar draws it, and in the
//! row's "?" what each look of the bird means.

use std::sync::{Arc, LazyLock};

use gpui_kit::prelude::*;
use gpui_kit::{Div, Image, ImageFormat, Rgba, TestSupportExt, div, img, px, rgb};

use crate::theme::Theme;
use crate::tray::Shape;

/// Linux and Windows call it the tray (Windows 11's own Settings: "Other
/// system tray icons"); not "panel", which is Crake's own popover there.
#[cfg(not(target_os = "macos"))]
pub const TITLE: &str = "Show the bird in the tray";
#[cfg(target_os = "macos")]
pub const TITLE: &str = "Show the bird in the menu bar";

/// Where Crake opens from without it.
#[cfg(target_os = "linux")]
const FROM: &str = "your apps";
#[cfg(windows)]
const FROM: &str = "the Start menu";
#[cfg(target_os = "macos")]
const FROM: &str = "Applications";

/// The row's line under its title.
pub static LINE: LazyLock<String> =
    LazyLock::new(|| format!("Without it, open Crake from {FROM}."));

/// The "?" card: what the bird is for, then its looks (`legend`), then how
/// to do without it.
pub static HELP: LazyLock<String> = LazyLock::new(|| {
    format!(
        "The bird is how you open Crake and see what it\u{2019}s doing at a glance.\n\nHide it and Crake keeps working; open Crake from {FROM}."
    )
});

/// Each look of the bird and what it means, in the order a day goes.
const LEGEND: [(Shape, &str); 6] = [
    (Shape::Off, "Asleep: no tunnel is on."),
    (Shape::Connecting, "Dashed: a tunnel is connecting."),
    (Shape::Listening, "Awake: a tunnel is on."),
    (
        Shape::Routed,
        "Two teal marks: Crake just sent a host through a tunnel.",
    ),
    (Shape::Warning, "Amber triangle: something needs you."),
    (Shape::Trouble, "Red badge: a tunnel stopped."),
];

/// The bars: dark (GNOME's, KDE's, Windows' and macOS' dark mode) and
/// light (Windows' and macOS' light mode, where the head is drawn dark).
const BAR_DARK: u32 = 0x2B2D30;
const BAR_LIGHT: u32 = 0xEEEEEE;
/// The head's ink on a light bar (`sys::menu_tray`'s DARK).
const INK_ON_LIGHT: &str = "#1f1f1f";

/// The badge after the title, and the legend's: its side and the head's.
const BADGE: f32 = 18.;
const BADGE_HEAD: f32 = 14.;
const LEGEND_BADGE: f32 = 24.;
const LEGEND_HEAD: f32 = 20.;
/// Drawn at this size (the drawing's own is 32 px), so it stays sharp at 2x.
const DRAWN_AT: &str = r#"width="64" height="64""#;

/// The bar Crake's bird sits in here is light.
fn light_bar() -> bool {
    #[cfg(any(windows, target_os = "macos"))]
    {
        crate::sys::light_bar()
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        false
    }
}

/// A look of the bird as the tray draws it, for this system's bar.
fn drawing(shape: Shape, light: bool) -> Arc<Image> {
    let svg = crate::tray::resting_svg(shape).replacen(r#"width="32" height="32""#, DRAWN_AT, 1);
    let svg = if light {
        crate::tray::for_light_bar(&svg, INK_ON_LIGHT)
    } else {
        svg
    };
    Arc::new(Image::from_bytes(ImageFormat::Svg, svg.into_bytes()))
}

/// A look of the bird drawn for a dark bar and for a light one.
type Drawn = (Shape, Arc<Image>, Arc<Image>);

/// Every look of the bird, drawn once.
static DRAWINGS: LazyLock<Vec<Drawn>> = LazyLock::new(|| {
    LEGEND
        .iter()
        .map(|(shape, _)| (*shape, drawing(*shape, false), drawing(*shape, true)))
        .collect()
});

fn badge(shape: Shape, side: f32, head: f32) -> Div {
    let light = light_bar();
    let image = DRAWINGS
        .iter()
        .find(|(s, ..)| *s == shape)
        .map(|(_, dark, lit)| if light { lit } else { dark })
        .expect("every shape in the legend is drawn")
        .clone();
    let bar: Rgba = rgb(if light { BAR_LIGHT } else { BAR_DARK });
    div()
        .flex_none()
        .size(px(side))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.))
        .bg(bar)
        .child(img(image).size(px(head)))
}

impl super::Settings {
    /// The row: the switch, and when nothing shows tray icons
    /// (`crate::no_tray`), why and what would bring the bird back.
    pub(super) fn bird_row(
        &self,
        show: bool,
        t: &Theme,
        cx: &mut gpui_kit::Context<Self>,
    ) -> gpui_kit::AnyElement {
        use crate::focus::Keys as _;
        let switch = super::toggle("bird", show, t)
            .keys()
            .on_click(cx.listener(move |this, _, _, cx| {
                Self::pref(cx, |p| p.show_bird = !show);
                this.requests
                    .send_blocking(crate::Request::ShowBird(!show))
                    .ok();
            }))
            .test_support();
        let missing = self.model.read(cx).no_tray.clone().filter(|_| show);
        match missing {
            Some(remedy) => super::row_below(
                self.title(TITLE, t, cx),
                remedy.line(),
                switch,
                crate::no_tray_view::remedy(&remedy, &self.requests, t),
                t,
            )
            .into_any_element(),
            None => super::row(self.title(TITLE, t, cx), Some(&LINE), switch, true, t)
                .into_any_element(),
        }
    }
}

/// After the title: the bird awake, as this system's bar shows it.
pub fn title_badge() -> Div {
    badge(Shape::Listening, BADGE, BADGE_HEAD)
}

/// In the "?" card, between its paragraphs: each look and its meaning.
pub fn legend(t: &Theme) -> impl IntoElement {
    let mut list = div().id("bird-legend").flex().flex_col().gap(px(6.));
    for (shape, meaning) in LEGEND {
        list = list.child(
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(badge(shape, LEGEND_BADGE, LEGEND_HEAD))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .text_color(t.ink)
                        .child(meaning),
                ),
        );
    }
    list.test_support()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_look_of_the_bird_is_drawn_and_explained() {
        assert_eq!(DRAWINGS.len(), LEGEND.len());
        for (shape, meaning) in LEGEND {
            assert!(meaning.ends_with('.'), "{meaning}");
            let svg = crate::tray::resting_svg(shape);
            assert!(svg.contains(r#"width="32" height="32""#), "{shape:?}");
        }
        // The light bar's head is dark; the teal marks stay teal.
        let light = String::from_utf8(drawing(Shape::Routed, true).bytes().to_vec()).unwrap();
        assert!(!light.contains("#ffffff") && light.contains(INK_ON_LIGHT));
        assert!(light.contains("#2DD4BF"));
    }

    #[test]
    fn the_words_are_this_systems() {
        assert!(!TITLE.contains("top bar") && !TITLE.contains("panel"));
        assert!(LINE.starts_with("Without it, open Crake from "));
        assert!(!HELP.contains("clock"));
    }
}
