use super::*;

/// The drawing's own units at `SIZE`: 2 px a unit.
const UNIT: f32 = SIZE as f32 / 32.;

/// The ARGB pixel of `icon` at a point in the drawing's units.
fn px(icon: &Icon, x: f32, y: f32) -> [u8; 4] {
    let (px, py) = ((x * UNIT) as usize, (y * UNIT) as usize);
    icon.data[(py * SIZE as usize + px) * 4..(py * SIZE as usize + px) * 4 + 4]
        .try_into()
        .unwrap()
}

#[test]
fn the_eye_is_a_hole_the_bar_shows_through() {
    let listening = &ICONS[Shape::Listening.index()];
    // The eye hole, where the raised head puts it; the breast solid.
    assert_eq!(px(listening, 14.5, 7.)[0], 0, "the eye is see-through");
    assert_eq!(px(listening, 16., 26.)[0], 255, "the breast is solid");
    assert_eq!(px(listening, 10., 24.)[0], 0, "the wing is a cut");
    // Routed: teal marks in front of the bill; trouble: a red badge at the
    // bottom right, over the breast; needs you: an amber triangle there.
    assert_eq!(
        px(&ICONS[Shape::Routed.index()], 29.1, 3.1)[1..],
        [0x2D, 0xD4, 0xBF]
    );
    for shape in [Shape::Trouble, Shape::OffTrouble, Shape::ConnectingTrouble] {
        assert_eq!(
            px(&ICONS[shape.index()], 22., 25.)[1..],
            [0xF9, 0x70, 0x66],
            "{shape:?}"
        );
    }
    for shape in [Shape::Warning, Shape::OffWarning, Shape::ConnectingWarning] {
        assert_eq!(
            px(&ICONS[shape.index()], 22., 29.)[1..],
            [0xF5, 0xB7, 0x40],
            "{shape:?}"
        );
    }
}

#[test]
fn the_badge_reads_at_sixteen_pixels() {
    // On whole pixels (tray-badge.html, B): a 7 px circle on rows 9-15,
    // its "!" one pixel wide on the middle column, solid: a stem on rows
    // 10-12, a gap, a dot on row 14. Today's badge sat between pixels and
    // its "!" was a smudge.
    let (_, small) = ICONS[Shape::Trouble.index()]
        .small
        .iter()
        .find(|(n, _)| *n == 16)
        .unwrap();
    let at = |x: usize, y: usize| -> [u8; 4] { small[(y * 16 + x) * 4..][..4].try_into().unwrap() };
    for y in [10, 11, 12, 14] {
        assert_eq!(at(12, y), [255, 0x0b, 0x15, 0x14], "the \"!\" at row {y}");
    }
    for (x, y) in [(12, 13), (11, 11), (13, 11), (12, 9), (9, 12), (15, 12)] {
        assert_eq!(at(x, y), [255, 0xF9, 0x70, 0x66], "red at {x},{y}");
    }
}

#[test]
fn a_light_bar_gets_the_light_themes_badge() {
    let svg = resting_svg(Shape::Trouble) + &resting_svg(Shape::Warning);
    let light = for_light_bar(&svg, "#1f1f1f");
    for dark in [
        "#ffffff",
        art::TROUBLE,
        art::ON_TROUBLE,
        art::WARNING,
        art::ON_WARNING,
    ] {
        assert!(!light.contains(dark), "{dark} is left");
    }
    for colour in ["#1f1f1f", "#B42318", "#FFFFFF", "#BF7E12", "#1A1C1E"] {
        assert!(light.contains(colour), "{colour} is missing");
    }
}

#[test]
fn the_sixteen_pixel_bird_is_drawn_smooth() {
    // 0.10.0 placed it pixel by pixel with no anti-aliasing, and it looked
    // pixelated beside the other icons in the bar: its edges are soft again,
    // as at every other size.
    let smallest = SMALL[0];
    for shape in [Shape::Listening, Shape::Off, Shape::Trouble] {
        let (_, data) = ICONS[shape.index()]
            .small
            .iter()
            .find(|(size, _)| *size == smallest)
            .unwrap();
        let soft = data
            .chunks(4)
            .filter(|p| p[0] > 0 && p[0] < u8::MAX)
            .count();
        assert!(soft > 10, "{shape:?}: {soft} soft edge pixels");
    }
}

#[test]
fn motion_ends_where_the_state_rests() {
    // The last frame of each transition looks like the resting icon.
    let close = |a: &Icon, b: &Icon| {
        a.data
            .iter()
            .zip(&b.data)
            .filter(|(x, y)| x.abs_diff(**y) > 40)
            .count()
            < 64
    };
    for (from, to) in [
        (Shape::Connecting, Shape::Listening),
        (Shape::Listening, Shape::Routed),
        (Shape::Routed, Shape::Listening),
        (Shape::Listening, Shape::Trouble),
        (Shape::Listening, Shape::Off),
        (Shape::Off, Shape::Listening),
        (Shape::Listening, Shape::OffTrouble),
        (Shape::Off, Shape::OffTrouble),
        (Shape::Connecting, Shape::ConnectingTrouble),
        (Shape::OffTrouble, Shape::Trouble),
        (Shape::Listening, Shape::Warning),
        (Shape::Off, Shape::OffWarning),
        (Shape::Connecting, Shape::ConnectingWarning),
        (Shape::OffWarning, Shape::Warning),
        // A tunnel stops while something needs you: red takes its place.
        (Shape::Warning, Shape::Trouble),
        (Shape::OffWarning, Shape::OffTrouble),
    ] {
        let last = transition(from, to).last().unwrap().icon();
        assert!(close(&last, &ICONS[to.index()]), "{from:?} -> {to:?}");
    }
    // The call: the bill opens as the marks draw, then the marks dim and
    // the bill half closes; the second call does it again.
    let frames = transition(Shape::Listening, Shape::Routed);
    assert!(frames[0].bill > 0. && frames[0].bill < 1. && frames[0].calls.unwrap().1 == 0.);
    assert!(
        frames
            .iter()
            .any(|f| f.bill == 0.5 && f.calls.unwrap().2 < 0.5)
    );
    assert_eq!(frames.iter().filter(|f| f.bill == 0.5).count(), 2);
    // Connecting goes round from the dashed bird at rest, every frame the
    // whole outline, its dashes moved on.
    let frames: Vec<Icon> = connecting_loop(None).iter().map(Art::icon).collect();
    assert!(close(&frames[0], &ICONS[Shape::Connecting.index()]));
    assert!(frames.windows(2).all(|w| w[0] != w[1]));
    let frames: Vec<Icon> = connecting_loop(Some(Badge::Trouble))
        .iter()
        .map(Art::icon)
        .collect();
    assert!(close(&frames[0], &ICONS[Shape::ConnectingTrouble.index()]));
    let frames: Vec<Icon> = connecting_loop(Some(Badge::Warning))
        .iter()
        .map(Art::icon)
        .collect();
    assert!(close(&frames[0], &ICONS[Shape::ConnectingWarning.index()]));
    // Dismissed: the badge just goes.
    assert!(transition(Shape::OffTrouble, Shape::Off).is_empty());
}

#[test]
fn the_coloured_marks_fit_inside_the_icon() {
    // A mark that reaches the edge is cut flat there. The badge sits in
    // the bottom right corner, its circle and triangle meeting the edges
    // without crossing them (tray-badge.html, B).
    let (b, r) = (art::BADGE.centre, art::BADGE.radius);
    assert!(
        b.0 + r <= 32. && b.1 + r <= 32.,
        "the circle fits: {b:?} {r}"
    );
    assert!(b.1 - r >= 0. && b.0 - r >= 0.);
    {
        let shape = Shape::Routed;
        let icon = &ICONS[shape.index()];
        // Coloured: drawn, and not the white bird (which runs off the
        // bottom).
        let coloured = |x: usize, y: usize| {
            let p = &icon.data[(y * 64 + x) * 4..(y * 64 + x) * 4 + 4];
            p[0] > 0 && !(p[1] > 230 && p[2] > 230 && p[3] > 230)
        };
        let at: Vec<(usize, usize)> = (0..64)
            .flat_map(|i| [(i, 0), (63, i), (i, 63), (0, i)])
            .filter(|(x, y)| coloured(*x, *y))
            .collect();
        assert!(at.is_empty(), "{shape:?} touches the icon's edge at {at:?}");
    }
}

#[test]
fn asleep_is_plain_at_a_glance() {
    // Off is far dimmer than listening, not just a different eye.
    let ink = |i: &Icon| i.data.chunks(4).map(|p| u64::from(p[0])).sum::<u64>();
    let (off, on) = (
        ink(&ICONS[Shape::Off.index()]),
        ink(&ICONS[Shape::Listening.index()]),
    );
    assert!(off * 2 < on, "off {off}, listening {on}");
    // The dropped head stays inside the icon's sides and top (the base
    // runs off the bottom, as every state's does).
    let icon = &rest(Shape::Off).icon();
    let edge = |x: usize, y: usize| icon.data[(y * 64 + x) * 4];
    let (mut x0, mut y0, mut x1) = (64, 64, 0);
    for y in 0..64 {
        for x in 0..64 {
            if edge(x, y) > 0 {
                (x0, y0, x1) = (x0.min(x), y0.min(y), x1.max(x));
            }
        }
    }
    assert!(x0 > 0 && y0 > 0 && x1 < 64, "drawn from {x0},{y0} to {x1}");
    // The eye is shut: no hole where the open eye would be.
    assert_ne!(px(icon, 14.5, 7.)[0], 0);
}

#[test]
fn every_state_has_its_own_shape() {
    let icons = &*ICONS;
    for (i, a) in icons.iter().enumerate() {
        assert_eq!(a.data.len(), 64 * 64 * 4);
        assert!(
            a.data.chunks(4).any(|p| p[0] > 0),
            "icon {i} draws something"
        );
        for b in &icons[i + 1..] {
            assert_ne!(a.data, b.data, "shapes differ");
            assert_ne!(a.small, b.small, "shapes differ at every size");
        }
    }
}

#[test]
fn red_wins_over_amber() {
    // A badge on the bird as it is; trouble's replaces a warning's.
    assert_eq!(Shape::Off.with(Badge::Warning), Shape::OffWarning);
    assert_eq!(
        Shape::Connecting.with(Badge::Warning),
        Shape::ConnectingWarning
    );
    assert_eq!(Shape::Routed.with(Badge::Warning), Shape::Warning);
    assert_eq!(Shape::OffWarning.with(Badge::Trouble), Shape::OffTrouble);
    assert_eq!(
        Shape::Warning.badged(),
        (Shape::Listening, Some(Badge::Warning))
    );
}

#[test]
fn the_bird_assets_are_current() {
    // Every drawing of the bird outside the tray comes from the tray's
    // geometry (`family`); the committed files are what it draws now.
    let stale: Vec<String> = family::assets()
        .into_iter()
        .filter(|(path, wanted)| std::fs::read_to_string(path).ok().as_deref() != Some(wanted))
        .map(|(path, _)| path.display().to_string())
        .collect();
    assert!(
        stale.is_empty(),
        "out of date (run `cargo test -p crake write_bird_assets -- --ignored`): {stale:#?}"
    );
}

/// Writes every bird asset: `cargo test -p crake write_bird_assets -- --ignored`.
#[test]
#[ignore]
fn write_bird_assets() {
    for (path, contents) in family::assets() {
        std::fs::write(&path, contents).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    }
}

/// The badges design/proposals/tray-badge.html compares, drawn by the
/// app's own code: `cargo test -p crake write_badge_proposal -- --ignored`.
#[test]
#[ignore]
fn write_badge_proposal() {
    use art::Place;
    let options = [
        (
            "today",
            Place {
                centre: (5.5, 5.5),
                radius: 4.5,
                snapped: false,
            },
        ),
        (
            "a",
            Place {
                centre: (5.5, 5.5),
                radius: 4.5,
                snapped: true,
            },
        ),
        (
            "b",
            Place {
                centre: (25., 25.),
                radius: 7.,
                snapped: true,
            },
        ),
        (
            "c",
            Place {
                centre: (24., 24.),
                radius: 8.,
                snapped: true,
            },
        ),
    ];
    let states = [
        ("awake", Shape::Trouble),
        ("asleep", Shape::OffTrouble),
        ("connecting", Shape::ConnectingTrouble),
        ("warning", Shape::Warning),
        ("warning-asleep", Shape::OffWarning),
    ];
    // On a light bar the bird is drawn dark (menu_tray.rs); the badge in
    // the light theme's danger and warning, as proposed, or as today.
    let light = |svg: String| {
        svg.replace("#F97066", "#B42318")
            .replace("#0b1514", "#FFFFFF")
            .replace("#F5B740", "#BF7E12")
            .replace("#1a1408", "#1A1C1E")
    };
    let mut js = String::from(
        "// Written by `cargo test -p crake write_badge_proposal -- --ignored`.\nconst ART = {\n",
    );
    for (key, place) in options {
        js += &format!("  {key:?}: {{\n");
        for (state, shape) in states {
            js += &format!("    {state:?}: {{\n");
            for n in [16_u32, 20, 22, 24, 32, 36] {
                let svg = |ink: &str| {
                    let art::Parts { defs, ink, over } = rest(shape).parts_at(n, ink, place);
                    format!(
                        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{n}" height="{n}" viewBox="0 0 {n} {n}"><defs>{defs}</defs>{ink}{over}</svg>"#
                    )
                };
                js += &format!(
                    "      {n}: {{ dark: {:?}, lightNow: {:?}, light: {:?} }},\n",
                    svg("#ffffff"),
                    svg("#1f1f1f"),
                    light(svg("#1f1f1f"))
                );
            }
            js += "    },\n";
        }
        js += "  },\n";
    }
    js += "};\n";
    let dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../design/proposals/tray-badge");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("art.js"), js).unwrap();
}
