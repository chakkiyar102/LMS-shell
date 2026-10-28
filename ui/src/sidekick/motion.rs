//! How Sidekick gets around: a flight is a quadratic Bézier arc from where he is to where help is needed,
//! sampled into transform keyframes (translate + a little banking), eased so takeoff and landing are
//! slow and the middle is quick. Pure, so the path, the timings and every "may he move?" rule are
//! tested natively; sidekick.rs plays the keyframes with the Web Animations API (transform only).
use super::SIZE;

pub type Pt = (f64, f64);
/// (left, top, right, bottom) in viewport px.
pub type Rect = (f64, f64, f64, f64);

/// Sidekick never flies higher than this (the top bar sits above it).
pub const SKY: f64 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flight { pub from: Pt, pub ctrl: Pt, pub to: Pt, pub ms: f64 }

/// The arc from `from` to `to`: it rises above the higher end by a lift that grows with distance,
/// never above `SKY`. A quadratic Bézier stays inside the triangle of its three points, so with all
/// three on screen the whole flight is on screen.
pub fn plan(from: Pt, to: Pt) -> Flight {
    let d = dist(from, to);
    let lift = (0.3 * d + 40.0).min(180.0);
    let ctrl = ((from.0 + to.0) / 2.0, (from.1.min(to.1) - lift).max(SKY));
    Flight { from, ctrl, to, ms: duration(d) }
}

pub fn dist(a: Pt, b: Pt) -> f64 { ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt() }

/// Longer trips take longer, within a range that still feels quick: 600 to 1300 ms.
pub fn duration(d: f64) -> f64 { (420.0 + d * 0.6).clamp(600.0, 1300.0) }

pub fn at(f: &Flight, t: f64) -> Pt {
    let u = 1.0 - t;
    (u * u * f.from.0 + 2.0 * u * t * f.ctrl.0 + t * t * f.to.0, u * u * f.from.1 + 2.0 * u * t * f.ctrl.1 + t * t * f.to.1)
}

fn velocity(f: &Flight, t: f64) -> Pt {
    (2.0 * (1.0 - t) * (f.ctrl.0 - f.from.0) + 2.0 * t * (f.to.0 - f.ctrl.0), 2.0 * (1.0 - t) * (f.ctrl.1 - f.from.1) + 2.0 * t * (f.to.1 - f.ctrl.1))
}

/// Ease in and out (smoothstep): slow lift-off, quick middle, gentle touchdown.
pub fn ease(t: f64) -> f64 { t * t * (3.0 - 2.0 * t) }

/// One keyframe: offset 0..1, position, banking in degrees (nose into the climb, either way he faces).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame { pub offset: f64, pub x: f64, pub y: f64, pub tilt: f64 }

pub const MAX_TILT: f64 = 18.0;

/// `n` keyframes (n >= 2) along the eased arc; level at takeoff and at landing.
pub fn frames(f: &Flight, n: usize) -> Vec<Frame> {
    let n = n.max(2);
    (0..n).map(|i| {
        let offset = i as f64 / (n - 1) as f64;
        let t = ease(offset);
        let (x, y) = at(f, t);
        let (vx, vy) = velocity(f, t);
        let tilt = if i == 0 || i == n - 1 || (vx.abs() + vy.abs()) < 1e-6 { 0.0 } else {
            let a = vy.atan2(vx.abs()).to_degrees();
            (if vx < 0.0 { -a } else { a }).clamp(-MAX_TILT, MAX_TILT)
        };
        Frame { offset, x, y, tilt }
    }).collect()
}

/// Sidekick's mode, and whether a dialog, the app tour or What's new is up, decide how he may move.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Travel {
    /// A full flight along the arc.
    Fly,
    /// Fade out here, fade in there: reduced motion, or Quiet (going home to the dock).
    Fade,
    /// Don't move at all.
    Hold,
}

pub fn travel(mode: super::Mode, reduced: bool, modal_open: bool) -> Travel {
    if modal_open || mode == super::Mode::Off { return Travel::Hold; }
    if reduced || mode == super::Mode::Quiet { Travel::Fade } else { Travel::Fly }
}

/// Whether Sidekick at `p` (a SIZE square) overlaps `r`, with `pad` px of breathing room.
pub fn overlaps(p: Pt, r: Rect, pad: f64) -> bool {
    p.0 < r.2 + pad && p.0 + SIZE > r.0 - pad && p.1 < r.3 + pad && p.1 + SIZE > r.1 - pad
}

/// A spot clamped onto the screen, under the top bar (a window that shrank under a Sidekick out pointing).
pub fn clamp_drop(p: Pt, vw: f64, vh: f64, y_top: f64) -> Pt {
    (p.0.clamp(super::SPOT_MARGIN, (vw - SIZE - super::SPOT_MARGIN).max(super::SPOT_MARGIN)), p.1.clamp(y_top, (vh - SIZE - super::SPOT_MARGIN).max(y_top)))
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::sidekick::{pick_spot, Mode};

    #[test]
    fn flight_stays_on_screen_and_lands_exactly() {
        for (vw, vh) in [(900.0, 600.0), (1280.0, 840.0), (1920.0, 1080.0)] {
            let pts = [(SPOT_M, 72.0), (vw - SIZE - SPOT_M, vh - SIZE - SPOT_M), (vw / 2.0, 80.0), (20.0, vh - 200.0), (vw - 100.0, 72.0)];
            for &a in &pts { for &b in &pts {
                let f = plan(a, b);
                let fr = frames(&f, 16);
                assert_eq!((fr[0].x, fr[0].y), a);
                assert!((fr[15].x - b.0).abs() < 1e-9 && (fr[15].y - b.1).abs() < 1e-9);
                assert_eq!((fr[0].tilt, fr[15].tilt), (0.0, 0.0), "level at takeoff and landing");
                for k in &fr {
                    assert!(k.x >= 0.0 && k.x + SIZE <= vw && k.y >= SKY - 1e-9 && k.y + SIZE <= vh, "{vw}x{vh} {a:?}->{b:?}: {k:?}");
                    assert!(k.tilt.abs() <= MAX_TILT);
                }
                assert!(fr.windows(2).all(|w| w[1].offset > w[0].offset));
                assert!((600.0..=1300.0).contains(&f.ms));
            } }
        }
    }
    const SPOT_M: f64 = crate::sidekick::SPOT_MARGIN;

    #[test]
    fn flight_arcs_upward_and_banks_into_the_climb() {
        let f = plan((100.0, 700.0), (1000.0, 700.0));
        assert!(at(&f, 0.5).1 < 700.0 - 40.0, "a level trip still arcs up");
        let fr = frames(&f, 12);
        assert!(fr[2].tilt < 0.0, "climbing right: nose up (counter-clockwise)");
        assert!(fr[9].tilt > 0.0, "descending right: nose down");
        let back = frames(&plan((1000.0, 700.0), (100.0, 700.0)), 12);
        assert!(back[2].tilt > 0.0, "climbing left: nose up (clockwise, he faces left)");
        assert!(plan((0.0, 0.0), (10.0, 0.0)).ms < plan((0.0, 0.0), (1400.0, 0.0)).ms);
    }

    #[test]
    fn the_landing_spot_never_covers_the_target() {
        let free = |_: f64, _: f64| false;
        for (vw, vh) in [(1000.0, 700.0), (1280.0, 840.0), (1920.0, 1080.0)] {
            for t in [(290.0, 326.0, 1150.0, 675.0), (600.0, 500.0, 700.0, 540.0), (40.0, 200.0, 400.0, 260.0), (vw - 300.0, 120.0, vw - 40.0, 180.0)] {
                if let Some((x, y, _)) = pick_spot(t, vw, vh, 72.0, false, free) {
                    assert!(!overlaps((x, y), t, 0.0), "{vw}x{vh} {t:?} -> {x},{y}");
                    // and the flight there ends where the spot is
                    let fr = frames(&plan((vw - 100.0, vh - 170.0), (x, y)), 10);
                    assert_eq!((fr[9].x.round(), fr[9].y.round()), (x.round(), y.round()));
                }
            }
        }
    }

    #[test]
    fn modes_reduced_motion_and_dialogs_decide_travel() {
        assert_eq!(travel(Mode::On, false, false), Travel::Fly);
        assert_eq!(travel(Mode::On, true, false), Travel::Fade, "reduced motion: no flight");
        assert_eq!(travel(Mode::Quiet, false, false), Travel::Fade);
        assert_eq!(travel(Mode::Off, false, false), Travel::Hold);
        for m in [Mode::On, Mode::Quiet, Mode::Off] { for r in [false, true] {
            assert_eq!(travel(m, r, true), Travel::Hold, "never moves while a dialog is open");
        } }
    }

    #[test]
    fn a_spot_is_pulled_back_on_screen_when_the_window_shrinks() {
        // the reported case: Sidekick at y 605 on a 1024 window, then 800x600: back inside, under the bar
        assert_eq!(clamp_drop((900.0, 605.0), 800.0, 600.0, 72.0), (800.0 - SIZE - SPOT_M, 600.0 - SIZE - SPOT_M));
        assert_eq!(clamp_drop((-50.0, 5000.0), 1280.0, 840.0, 72.0), (SPOT_M, 840.0 - SIZE - SPOT_M));
        // a wrapped top bar (390 px phone, large text) is 150 px tall: nothing goes under it
        assert_eq!(clamp_drop((20.0, 90.0), 390.0, 844.0, 158.0).1, 158.0);
        assert!(pick_spot((20.0, 150.0, 200.0, 200.0), 390.0, 844.0, 158.0, false, |_, _| false).is_some_and(|(_, y, _)| y >= 158.0));
    }
}
