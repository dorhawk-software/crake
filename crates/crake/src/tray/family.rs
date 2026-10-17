//! The bird everywhere else, drawn from the same geometry as the tray's:
//! the spec copies of the tray icons (design/icons), the panel header and
//! its badges, the leftover notice's head, the first-run bird, the
//! whole-bird symbolic icons (CRAKE_TRAY=bird) and the app icon.
//! `tests::the_bird_assets_are_current` checks the committed files against
//! these; `cargo test -p crake write_bird_assets -- --ignored` writes them.

use std::path::PathBuf;

use super::art::{self, Art};
use super::{Shape, rest, resting_svg};

/// The ink of a symbolic icon (GNOME's), and the spec copies'.
const SYMBOLIC: &str = "#2e3436";
/// The family's drawings sit on the 24 px grid of the other icons.
const FAMILY: u32 = 24;
/// Draw-time colour: the app recolours each SVG, so black is "the colour".
const BLACK: &str = "#000";
/// The app icon's bird: DESIGN.md's dark text colour, one ink on the
/// tile like the bird everywhere else.
const LIGHT_INK: &str = "#ECEDEE";
/// The tile behind the app icon (DESIGN.md's dark panel).
const TILE: &str = "#1E2022";
const ACCENT: &str = "#2DD4BF";

fn root() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

fn svg(n: u32, body: String) -> String {
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{n}" height="{n}" viewBox="0 0 {n} {n}">{body}</svg>"##
    )
}

/// `art` on the family's grid, in black, without its coloured marks.
fn ink(art: &Art) -> String {
    let art::Parts { defs, ink, .. } = art.parts(FAMILY, BLACK);
    svg(FAMILY, format!("<defs>{defs}</defs>{ink}"))
}

/// The whole bird in the grass, for CRAKE_TRAY=bird: the grass as it was,
/// the bird nested among it.
fn symbolic(shape: Shape) -> String {
    let grass =
        r#"<path d="M3.5 20.5 5.3 15M6.2 20.5l.6-3.4M17.4 20.5l-.6-3.6M20.5 20.5l-1.7-5.2"/>"#;
    let bird = match shape {
        Shape::Off => String::new(),
        _ => {
            let art::Parts { defs, ink, over } = rest(shape).parts(32, SYMBOLIC);
            format!(
                r##"<svg x="4" y="4.5" width="16" height="16" viewBox="0 0 32 32"><defs>{defs}</defs>{ink}{over}</svg>"##
            )
        }
    };
    let mark = match shape {
        Shape::Trouble => format!(
            r##"<path d="M4.5 3.5v4"/><circle cx="4.5" cy="10.3" r="0.9" fill="{SYMBOLIC}" stroke="none"/>"##
        ),
        _ => String::new(),
    };
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="{SYMBOLIC}" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round">{grass}{bird}{mark}</svg>"##
    )
}

/// The app icon: the bird on the dark tile in one light ink, its eye and
/// wing cut, calling in accent, as the panel's header draws it.
fn app_icon() -> String {
    let art::Parts { defs, ink, .. } = rest(Shape::Listening).parts(FAMILY, LIGHT_INK);
    let (_, call) = art::marks(FAMILY as f32 / 32., 1., 1., 1., ACCENT);
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="256" height="256" viewBox="0 0 24 24"><rect width="24" height="24" rx="5.5" fill="{TILE}"/><svg x="3" y="3.5" width="18" height="18" viewBox="0 0 24 24"><defs>{defs}</defs>{ink}{call}</svg></svg>"##
    )
}

/// Every asset drawn from the bird: its path from the repository's root,
/// and its contents.
pub(super) fn assets() -> Vec<(PathBuf, String)> {
    let root = root();
    let mut out = Vec::new();
    for (shape, name) in [
        (Shape::Off, "off"),
        (Shape::Connecting, "connecting"),
        (Shape::Listening, "listening"),
        (Shape::Routed, "routed"),
        (Shape::Trouble, "trouble"),
        (Shape::OffTrouble, "off-trouble"),
        (Shape::ConnectingTrouble, "connecting-trouble"),
        (Shape::Warning, "warning"),
        (Shape::OffWarning, "off-warning"),
        (Shape::ConnectingWarning, "connecting-warning"),
    ] {
        out.push((
            root.join(format!("design/icons/tray-head-{name}.svg")),
            resting_svg(shape).replace("#ffffff", SYMBOLIC),
        ));
    }
    for (shape, name) in [
        (Shape::Off, "off"),
        (Shape::Connecting, "connecting"),
        (Shape::Listening, "listening"),
        (Shape::Routed, "routed"),
        (Shape::Trouble, "trouble"),
    ] {
        for dir in ["assets/icons", "design/icons"] {
            out.push((
                root.join(format!("{dir}/crake-{name}-symbolic.svg")),
                symbolic(shape),
            ));
        }
    }
    let icons = root.join("assets/icons");
    let listening = rest(Shape::Listening);
    let s = FAMILY as f32 / 32.;
    let (_, call) = art::marks(s, 1., 1., 1., BLACK);
    let (_, trouble, trouble_mark) = art::badge(s, 1., false, BLACK, BLACK);
    let (_, warning, warning_mark) = art::badge(s, 1., true, BLACK, BLACK);
    out.extend([
        // The panel header's bird, and the first-run bird that rises from the grass.
        (icons.join("bird.svg"), ink(&listening)),
        (icons.join("bird-alone.svg"), ink(&listening)),
        (icons.join("call.svg"), svg(FAMILY, call)),
        // Over the header's bird: what the bird in the bar wears.
        (icons.join("bird-badge-trouble.svg"), svg(FAMILY, trouble)),
        (icons.join("bird-badge-mark.svg"), svg(FAMILY, trouble_mark)),
        (
            icons.join("bird-badge-warning.svg"),
            svg(FAMILY, warning.clone()),
        ),
        (
            icons.join("bird-badge-warning-mark.svg"),
            svg(FAMILY, warning_mark.clone()),
        ),
        // The leftover notice: the bird with the triangle's gap cut, and the triangle.
        (
            icons.join("head-warning.svg"),
            ink(&Art {
                badge: Some(1.),
                warning: true,
                ..listening
            }),
        ),
        (icons.join("warning-triangle.svg"), svg(FAMILY, warning)),
        (icons.join("warning-mark.svg"), svg(FAMILY, warning_mark)),
        (icons.join("app/crake.svg"), app_icon()),
    ]);
    out
}
