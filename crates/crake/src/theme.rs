//! Tokens from design/DESIGN.md. One accent: teal means "Crake made a decision";
//! red, what stopped or failed; amber, what needs you.

use gpui_kit::{App, Rgba, rgb};

// The full token set from DESIGN.md; not every view uses every token yet.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub dark: bool,
    pub panel: Rgba,
    pub surface: Rgba,
    pub line: Rgba,
    pub line_strong: Rgba,
    pub ink: Rgba,
    pub muted: Rgba,
    pub accent: Rgba,
    pub accent_ink: Rgba,
    pub accent_soft: Rgba,
    pub danger: Rgba,
    /// What reads on `danger`: the × of a window's close button pointed at.
    pub on_danger: Rgba,
    /// A Settings section pointed at: halfway from the sidebar's colour to
    /// the current section's (design/proposals/hover-close-and-sections.html).
    pub nav_hover: Rgba,
    /// Needs you, nothing has broken yet (design/proposals/leftover-warning.html):
    /// the warning triangle, a notice's tint and hairline, the "!" on it.
    pub warning: Rgba,
    pub warning_soft: Rgba,
    pub warning_line: Rgba,
    pub on_warning: Rgba,
    /// Chart volume bars.
    pub bars: Rgba,
    /// The switch's track when off.
    pub track: Rgba,
}

impl Theme {
    pub fn light() -> Self {
        Self {
            dark: false,
            panel: rgb(0xFFFFFF),
            surface: rgb(0xF1F1EE),
            line: rgb(0xE2E2DE),
            line_strong: rgb(0xBDBDB8),
            ink: rgb(0x1A1C1E),
            muted: rgb(0x5A5F64),
            accent: rgb(0x0F766E),
            accent_ink: rgb(0xFFFFFF),
            accent_soft: rgb(0xE2F1EF),
            danger: rgb(0xB42318),
            on_danger: rgb(0xFFFFFF),
            nav_hover: rgb(0xF9F9F7),
            warning: rgb(0xE39A1B),
            warning_soft: rgb(0xFCF4E4),
            warning_line: rgb(0xEFD9AE),
            on_warning: rgb(0x1A1C1E),
            bars: rgb(0xD8D9D5),
            track: rgb(0xBDBDB8),
        }
    }

    pub fn dark() -> Self {
        Self {
            dark: true,
            panel: rgb(0x1E2022),
            surface: rgb(0x2A2D30),
            line: rgb(0x33363A),
            line_strong: rgb(0x4A4E53),
            ink: rgb(0xECEDEE),
            muted: rgb(0xA9AEB4),
            accent: rgb(0x2DD4BF),
            accent_ink: rgb(0x0B1514),
            accent_soft: rgb(0x17332F),
            danger: rgb(0xF97066),
            on_danger: rgb(0x0B1514),
            nav_hover: rgb(0x242729),
            warning: rgb(0xF5B740),
            warning_soft: rgb(0x2C2618),
            warning_line: rgb(0x5C4A22),
            on_warning: rgb(0x1A1408),
            bars: rgb(0x3E4246),
            track: rgb(0x4A4E53),
        }
    }

    /// Keeps the widget toolkit's own theme (text fields, their cursor) in the
    /// same light or dark as Crake's, as tern does.
    pub fn sync_components(&self, cx: &mut App) {
        let mode = if self.dark {
            gpui_kit::component::ThemeMode::Dark
        } else {
            gpui_kit::component::ThemeMode::Light
        };
        gpui_kit::component::Theme::change(mode, None, cx);
    }

    /// Follows the desktop's light or dark setting.
    pub fn current(cx: &App) -> Self {
        // Settings: "Appearance" may pin light or dark.
        match cx
            .try_global::<crate::prefs::Prefs>()
            .map(|p| p.appearance.as_str())
        {
            Some("light") => return Self::light(),
            Some("dark") => return Self::dark(),
            _ => {}
        }
        match cx.window_appearance() {
            gpui_kit::WindowAppearance::Dark | gpui_kit::WindowAppearance::VibrantDark => {
                Self::dark()
            }
            _ => Self::light(),
        }
    }
}

/// Bundled from `assets/fonts/` (OFL).
pub mod fonts {
    use std::borrow::Cow;

    use gpui_kit::App;

    /// Everything you read.
    pub const UI: &str = "Atkinson Hyperlegible Next";
    /// Machine facts only: hosts, addresses, times.
    pub const MONO: &str = "Atkinson Hyperlegible Mono";

    macro_rules! font_files {
        ($($file:literal),* $(,)?) => {
            [$(Cow::Borrowed(include_bytes!(concat!("../../../assets/fonts/", $file)).as_slice())),*]
        };
    }

    pub fn load(cx: &App) {
        let files = font_files![
            "AtkinsonHyperlegibleNext-Regular.ttf",
            "AtkinsonHyperlegibleNext-SemiBold.ttf",
            "AtkinsonHyperlegibleNext-Bold.ttf",
            "AtkinsonHyperlegibleNext-Italic.ttf",
            "AtkinsonHyperlegibleMono-Regular.ttf",
            "AtkinsonHyperlegibleMono-Medium.ttf",
        ];
        cx.text_system()
            .add_fonts(Vec::from(files))
            .expect("bundled fonts are valid");
    }
}
