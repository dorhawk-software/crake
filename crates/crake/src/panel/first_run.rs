//! No tunnel yet (design/proposals/panel-no-tunnels.html, B): nothing to
//! chart or report, so the panel shows the bird, one sentence and the way
//! to add a tunnel. The bird peeks out of the grass now and then, looks
//! left and right for a tunnel, finds none and settles back
//! (design/proposals/first-run-bird.html, B, chosen by the user: again
//! and again, after a pause that's different each time). Still with
//! reduce motion on.

use std::time::{Duration, Instant};

use gpui_kit::{Task, Transformation, point, size};

use super::*;

/// The icon's size, and the drawing's (its viewBox, 24 units).
const BIRD: f32 = 52.;
const UNITS: f32 = 24.;
/// One peek, as in the mockup.
const PEEK: Duration = Duration::from_millis(2800);
/// The first one, a moment after the panel opens.
const FIRST: Duration = Duration::from_millis(600);
/// Between peeks: at least this long, and up to this much more.
const PAUSE: Duration = Duration::from_secs(3);
const PAUSE_MORE_MS: u64 = 5000;
/// How far it rises out of the grass, in the drawing's units.
const RISE: f32 = 1.6;

/// When the bird peeks: the one under way, or when the next is due, and
/// the timer that wakes the panel for it.
pub(super) struct Peek {
    started: Option<Instant>,
    next: Instant,
    wake: Option<Task<()>>,
}

impl Peek {
    pub(super) fn new() -> Self {
        Self {
            started: None,
            next: Instant::now() + FIRST,
            wake: None,
        }
    }
}

// literal: the peek's beats, as fractions of it (design/proposals/first-run-bird.html, B)
/// Where the bird is `at` into a peek: how far up (drawing units, up is
/// negative) and which way it faces (1 right, -1 left; between them it is
/// turning).
pub(super) fn peek(at: Duration) -> (f32, f32) {
    let f = at.as_secs_f32() / PEEK.as_secs_f32();
    let part =
        |from: f32, to: f32| crate::motion::ease_out(((f - from) / (to - from)).clamp(0., 1.));
    let up = -RISE * (part(0., 0.14) - part(0.66, 0.80));
    let turned = part(0.26, 0.32) - part(0.46, 0.52);
    (up, 1. - 2. * turned)
}

/// A pause between peeks, different each time (from the clock's
/// nanoseconds: no randomness needs to be any better here).
fn pause() -> Duration {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    PAUSE + Duration::from_millis(u64::from(nanos) % (PAUSE_MORE_MS + 1))
}

impl Panel {
    pub(super) fn first_run(
        &mut self,
        t: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let (up, facing) = self.bird_now(window, cx);
        let scale = BIRD / UNITS;
        let bird =
            Transformation::scale(size(facing, 1.)).with_translation(point(px(0.), px(up * scale)));
        div()
            .border_t_1()
            .border_color(t.line)
            .pt(px(34.))
            .pb(px(30.))
            .px(px(32.))
            .flex()
            .flex_col()
            .items_center()
            .child(
                div()
                    .relative()
                    .size(px(BIRD))
                    .child(svg().path("crake/bird-grass.svg").absolute().size(px(BIRD)).text_color(t.ink))
                    .child(
                        svg()
                            .path("crake/bird-alone.svg")
                            .absolute()
                            .size(px(BIRD))
                            .text_color(t.ink)
                            .with_transformation(bird),
                    ),
            )
            .child(
                div()
                    .mt(px(14.))
                    .text_size(px(18.))
                    .line_height(px(24.))
                    .font_weight(FontWeight::BOLD)
                    .child("No tunnels yet"),
            )
            .child(
                div()
                    .mt(px(6.))
                    .max_w(px(300.))
                    .text_center()
                    .text_color(t.muted)
                    .child(
                        "Add the VPN your company gave you. Crake connects to it and sends through only what needs it.",
                    ),
            )
            .child(
                div()
                    .id("add-tunnel")
                    .mt(px(18.))
                    .w(px(220.))
                    .h(px(40.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(6.))
                    .bg(t.accent)
                    .text_color(t.accent_ink)
                    .font_weight(FontWeight::SEMIBOLD)
                    .cursor_pointer()
                    .child("Add a tunnel\u{2026}")
                    .keys()
                    .on_click(cx.listener(|this, _, _, _| this.send(Request::NewTunnel(None))))
                    .test_support(),
            )
    }

    /// The bird now: mid-peek (asking for the next frame), or still, with a
    /// timer to wake the panel when the next peek is due.
    fn bird_now(&mut self, window: &mut Window, cx: &mut Context<Self>) -> (f32, f32) {
        if !crate::motion::enabled() {
            return (0., 1.);
        }
        let slow = crate::motion::slowed();
        let now = Instant::now();
        if let Some(started) = self.peek.started {
            let at = now.duration_since(started) / slow;
            if at < PEEK {
                window.request_animation_frame();
                return peek(at);
            }
            self.peek.started = None;
            self.peek.next = now + pause() * slow;
            self.peek.wake = None;
        }
        if now >= self.peek.next {
            self.peek.started = Some(now);
            window.request_animation_frame();
            return peek(Duration::ZERO);
        }
        if self.peek.wake.is_none() {
            let wait = self.peek.next - now;
            self.peek.wake = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(wait).await;
                let _ = this.update(cx, |this, cx| {
                    this.peek.wake = None;
                    cx.notify();
                });
            }));
        }
        (0., 1.)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bird_peeks_out_looks_both_ways_and_settles() {
        let at = |ms| peek(Duration::from_millis(ms));
        // Down in the grass, facing right.
        assert_eq!(at(0), (0., 1.));
        // Up, still facing right.
        let (up, facing) = at(600);
        assert!((up + RISE).abs() < 1e-3 && facing == 1., "{up} {facing}");
        // Up, looking left.
        let (up, facing) = at(1100);
        assert!((up + RISE).abs() < 1e-3 && facing == -1., "{up} {facing}");
        // Right again, then down: where it started.
        assert_eq!(at(1600).1, 1.);
        let (up, facing) = at(2600);
        assert!(up.abs() < 1e-3 && facing == 1., "{up} {facing}");
    }

    #[test]
    fn pauses_differ_within_reason() {
        let p = pause();
        assert!(p >= PAUSE && p <= PAUSE + Duration::from_millis(PAUSE_MORE_MS));
    }
}
