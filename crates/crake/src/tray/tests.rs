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
fn a_light_bar_gets_the_light_themes_marks() {
    let svg =
        resting_svg(Shape::Trouble) + &resting_svg(Shape::Warning) + &resting_svg(Shape::Routed);
    let light = for_light_bar(&svg, "#1f1f1f");
    for dark in [
        "#ffffff",
        art::CALL,
        art::TROUBLE,
        art::ON_TROUBLE,
        art::WARNING,
        art::ON_WARNING,
    ] {
        assert!(!light.contains(dark), "{dark} is left");
    }
    for colour in [
        "#1f1f1f", "#0F766E", "#B42318", "#FFFFFF", "#BF7E12", "#1A1C1E",
    ] {
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

/// The Mac's drawing of `shape` (menu-bar-mac.html, C), as ARGB at its
/// size, and the rows and columns its ink covers.
fn mac(shape: Shape) -> (Vec<u8>, (usize, usize), (usize, usize)) {
    let n = art::MAC_SIZE as usize;
    let data = raster(&rest(shape).svg_mac(), art::MAC_SIZE);
    let (mut rows, mut cols) = ((n, 0), (n, 0));
    for y in 0..n {
        for x in 0..n {
            if data[(y * n + x) * 4] > 0 {
                rows = (rows.0.min(y), rows.1.max(y));
                cols = (cols.0.min(x), cols.1.max(x));
            }
        }
    }
    (data, rows, cols)
}

#[test]
fn the_macs_bird_is_its_neighbours_size() {
    // About 31 px of the 44 (15 pt; 1Password, the moon and Wi-Fi are 25
    // to 32), on their middle line, the image's, and inside
    // the image in every state: the badge, the call and the dropped head
    // too, so nothing beside it moves.
    let n = art::MAC_SIZE as usize;
    let (_, rows, _) = mac(Shape::Listening);
    let tall = rows.1 - rows.0 + 1;
    assert!((29..=31).contains(&tall), "{tall} px tall");
    assert!(
        (20..=22).contains(&((rows.0 + rows.1) / 2)),
        "rows {rows:?}"
    );
    for shape in [Shape::Off, Shape::Routed, Shape::Trouble, Shape::Warning] {
        let (_, rows, cols) = mac(shape);
        assert!(
            rows.0 > 0 && rows.1 < n - 1 && cols.0 > 0 && cols.1 < n - 1,
            "{shape:?}: {rows:?} {cols:?}"
        );
    }
}

#[test]
fn the_macs_bird_has_its_eye_and_dims_less_asleep() {
    let n = art::MAC_SIZE as usize;
    let at = |data: &[u8], x: usize, y: usize| data[(y * n + x) * 4];
    // The round eye is a hole (where the raised head puts it: 14, 7 of the
    // 32 px drawing, 6 and 6 px in); the breast is solid, the wing a cut.
    let (listening, ..) = mac(Shape::Listening);
    assert_eq!(at(&listening, 6 + 14, 6 + 7), 0, "the eye is see-through");
    assert_eq!(at(&listening, 6 + 16, 6 + 26), 255, "the breast is solid");
    assert_eq!(at(&listening, 6 + 10, 6 + 24), 0, "the wing is a cut");
    // Asleep is dimmed less than in other bars (70 %, not 45 %).
    let ink = |d: &[u8]| d.chunks(4).map(|p| u64::from(p[0])).sum::<u64>();
    let (off, ..) = mac(Shape::Off);
    let peak = off.chunks(4).map(|p| p[0]).max().unwrap();
    assert!((170..=185).contains(&peak), "asleep at {peak} of 255");
    assert!(ink(&off) * 2 > ink(&listening), "asleep isn't half gone");
}

#[test]
fn the_macs_1x_bird_is_drawn_for_its_pixels() {
    // A 1x display gets a drawing of its own, not the 2x one halved: the
    // eye a square hole on whole pixels (2 px, at 9, 5), solid around it,
    // where the halved round eye was a grey smudge.
    let n = art::MAC_POINTS as usize;
    let one_x = raster(&rest(Shape::Listening).svg_mac_1x(), art::MAC_POINTS);
    let at = |x: usize, y: usize| one_x[(y * n + x) * 4];
    for (x, y) in [(9, 5), (10, 5), (9, 6), (10, 6)] {
        assert_eq!(at(x, y), 0, "the eye is see-through at {x}, {y}");
    }
    for (x, y) in [(8, 5), (11, 5), (8, 6), (11, 6), (9, 7), (10, 7)] {
        assert_eq!(at(x, y), 255, "solid next to the eye at {x}, {y}");
    }
    // The Mac's menu bar is handed both; other bars, none of them.
    let sizes: Vec<u32> = rest(Shape::Routed)
        .icon()
        .mac
        .iter()
        .map(|(n, _)| *n)
        .collect();
    if cfg!(target_os = "macos") {
        assert_eq!(sizes, [art::MAC_POINTS, art::MAC_SIZE]);
    } else {
        assert!(sizes.is_empty());
    }
}

#[test]
fn the_warning_triangle_ends_on_a_whole_row() {
    // Its lowest row is solid amber: no faint row under it (the stroke's
    // edge spilled into one, a line under the triangle at 16 px).
    let amber = u32::from_str_radix(&art::WARNING[1..], 16)
        .unwrap()
        .to_be_bytes();
    let drawings = SMALL
        .iter()
        .map(|&n| (n, raster(&rest(Shape::Warning).svg_at(n), n)))
        .chain([
            (
                art::MAC_SIZE,
                raster(&rest(Shape::Warning).svg_mac(), art::MAC_SIZE),
            ),
            (
                art::MAC_POINTS,
                raster(&rest(Shape::Warning).svg_mac_1x(), art::MAC_POINTS),
            ),
        ]);
    for (n, data) in drawings {
        let n = n as usize;
        let is_amber = |p: &[u8]| p[0] > 0 && p[1..] == amber[1..];
        let rows: Vec<Vec<u8>> = (0..n)
            .map(|y| {
                (0..n)
                    .map(|x| &data[(y * n + x) * 4..][..4])
                    .filter(|p| is_amber(p))
                    .map(|p| p[0])
                    .collect()
            })
            .filter(|row: &Vec<u8>| !row.is_empty())
            .collect();
        let last = rows.last().expect("an amber triangle");
        assert_eq!(
            last.iter().max(),
            Some(&255),
            "at {n} px the triangle's last row is faint: {last:?}"
        );
    }
}

#[test]
fn a_light_bar_leaves_no_dark_bar_mark() {
    // Windows and the Mac recolour the drawn image pixel by pixel: the
    // anti-aliased edges of the call marks and the badges too, not only
    // the pixels that came back exactly a mark's colour.
    let hex = |h: &str| u32::from_str_radix(&h[1..], 16).unwrap().to_be_bytes();
    let near = |p: [u8; 3], h: &str| {
        let [_, r, g, b] = hex(h);
        p.iter()
            .zip([r, g, b])
            .all(|(a, b)| (i16::from(*a) - i16::from(b)).abs() <= 24)
    };
    for shape in [Shape::Routed, Shape::Trouble, Shape::Warning] {
        for (n, data) in [
            (32, raster(&rest(shape).svg_at(32), 32)),
            (art::MAC_SIZE, raster(&rest(shape).svg_mac(), art::MAC_SIZE)),
            (
                art::MAC_POINTS,
                raster(&rest(shape).svg_mac_1x(), art::MAC_POINTS),
            ),
        ] {
            for p in data.chunks(4).filter(|p| p[0] > 0) {
                let light = light_bar_pixel([p[1], p[2], p[3]], [0x1f, 0x1f, 0x1f]);
                for dark in [art::CALL, art::TROUBLE, art::WARNING] {
                    assert!(
                        !near(light, dark),
                        "{shape:?} at {n} px: {light:?} is still {dark}"
                    );
                }
            }
        }
    }
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

/// The Mac's drawing of each state, as art.rs draws it, for
/// design/proposals/menu-bar-mac.html's "as built" bars:
/// `cargo test -p crake write_mac_proposal -- --ignored`.
#[test]
#[ignore]
fn write_mac_proposal() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../design/proposals/menu-bar-mac/built");
    std::fs::create_dir_all(&dir).unwrap();
    for shape in SHAPES {
        let svg = rest(shape).svg_mac();
        std::fs::write(dir.join(format!("{shape:?}.svg").to_lowercase()), &svg).unwrap();
        std::fs::write(
            dir.join(format!("{shape:?}-light.svg").to_lowercase()),
            for_light_bar(&svg, "#292929"),
        )
        .unwrap();
    }
}
