//! Drawing shared by the charts.

use gpui_kit::{PathBuilder, Pixels, point, px};

/// A traffic bar centred on `x`, `width` wide, standing on `base`, `height`
/// tall. Its edges snap to whole device pixels and every bar gets the same
/// pixel width: at fractional positions anti-aliasing would draw some bars
/// a pixel thinner than their neighbours.
pub fn bar(
    x: f32,
    width: f32,
    base: f32,
    height: f32,
    scale: f32,
) -> Option<gpui_kit::Path<Pixels>> {
    let scale = scale.max(1.);
    let w = (width * scale).round().max(1.);
    let left = ((x - width / 2.) * scale).round() / scale;
    let right = left + w / scale;
    let top = ((base - height) * scale).round() / scale;
    let base = (base * scale).round() / scale;
    let mut p = PathBuilder::fill();
    p.move_to(point(px(left), px(base)));
    p.line_to(point(px(left), px(top)));
    p.line_to(point(px(right), px(top)));
    p.line_to(point(px(right), px(base)));
    p.close();
    p.build().ok()
}

/// A chart bar with a flat bottom on the baseline and a rounded top: a
/// stroke of the two-stroke tick (DESIGN.md, Charts).
pub fn rounded_bar(x: f32, top: f32, base: f32, w: f32) -> Option<gpui_kit::Path<Pixels>> {
    let r = w / 2.;
    let mut p = PathBuilder::fill();
    p.move_to(point(px(x - r), px(base)));
    p.line_to(point(px(x - r), px(top + r)));
    p.arc_to(
        point(px(r), px(r)),
        px(0.),
        false,
        true,
        point(px(x + r), px(top + r)),
    );
    p.line_to(point(px(x + r), px(base)));
    p.close();
    p.build().ok()
}
