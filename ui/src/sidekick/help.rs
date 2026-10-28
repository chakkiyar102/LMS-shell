//! Hover help: the mouse resting on a control (`data-help`) for `DWELL_MS`, give or take a small
//! jitter, makes Sidekick explain it, once per control per device (`sidekick_help_seen`); a touch long-press does
//! the same on demand. Tooltips show a control's short label after `TIP_MS`, placed wholly on screen and
//! clear of the native practice pane. Pure, so the timings and every placement are tested natively.
use super::motion::{Pt, Rect};

pub const DWELL_MS: f64 = 5_000.0;
/// How far the pointer may drift and still count as resting.
pub const JITTER: f64 = 14.0;
pub const TIP_MS: f64 = 500.0;
pub const LONG_PRESS_MS: f64 = 600.0;
/// A hover this long that ends without a click says the learner was unsure.
pub const HESITATE_MS: f64 = 1_200.0;

/// Where the pointer has been resting, and on what.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Dwell { key: Option<String>, at: Pt, since: f64, fired: bool, clicked: bool }

/// What one pointer sample means.
#[derive(Clone, Debug, PartialEq)]
pub enum Rest {
    Nothing,
    /// Rested long enough on `key`: explain it (once per rest).
    Explain(String),
    /// Left `key` after hovering `ms` without clicking it.
    LeftUnclicked(String),
}

impl Dwell {
    /// The pointer at `at`, over the control `key` (None: over nothing with help), at `now`.
    pub fn sample(&mut self, key: Option<&str>, at: Pt, now: f64) -> Rest {
        let moved = super::motion::dist(at, self.at) > JITTER;
        if key != self.key.as_deref() {
            let left = self.key.take().filter(|_| !self.clicked && now - self.since >= HESITATE_MS);
            *self = Dwell { key: key.map(str::to_string), at, since: now, fired: false, clicked: false };
            return left.map(Rest::LeftUnclicked).unwrap_or(Rest::Nothing);
        }
        if moved {
            // still on the same control but moving: the rest starts over from here
            self.at = at;
            if !self.fired { self.since = now }
            return Rest::Nothing;
        }
        match &self.key {
            Some(k) if !self.fired && now - self.since >= DWELL_MS => { self.fired = true; Rest::Explain(k.clone()) }
            _ => Rest::Nothing,
        }
    }
    /// The learner clicked the control under the pointer.
    pub fn click(&mut self) { self.clicked = true; self.fired = true }
    /// Typing, a dialog, Sidekick busy: whatever rest was building starts over.
    pub fn reset(&mut self, now: f64) { self.since = now; }
}

/// Where a `w` x `h` tooltip goes for target `t` in a `vw` x `vh` window, never over `forbid` (the native
/// practice app pane): below, above, right, then left of the target, the first wholly on screen and clear;
/// failing all, below and clamped onto the screen. Returns its top-left.
pub fn tip_spot(t: Rect, w: f64, h: f64, vw: f64, vh: f64, forbid: &[Rect]) -> Pt {
    const GAP: f64 = 8.0;
    const M: f64 = 6.0;
    let (l, top, r, b) = t;
    let cx = ((l + r) / 2.0 - w / 2.0).clamp(M, (vw - w - M).max(M));
    let cy = ((top + b) / 2.0 - h / 2.0).clamp(M, (vh - h - M).max(M));
    let tries = [(cx, b + GAP), (cx, top - GAP - h), (r + GAP, cy), (l - GAP - w, cy)];
    let fits = |&(x, y): &(f64, f64)| x >= M - 0.5 && y >= M - 0.5 && x + w <= vw - M + 0.5 && y + h <= vh - M + 0.5
        && !forbid.iter().any(|f| x < f.2 && x + w > f.0 && y < f.3 && y + h > f.1);
    tries.into_iter().find(fits).unwrap_or((cx, (b + GAP).clamp(M, (vh - h - M).max(M))))
}

/// The widest band (left, right) of the window a bubble for `anchor` may use without going over the
/// native practice pane `slot`: the side of it the anchor is on; for an anchor above the pane (its
/// toolbar), the roomier side.
pub fn band(anchor: Rect, vw: f64, slot: Option<Rect>) -> (f64, f64) {
    let Some(s) = slot.filter(|s| s.2 - s.0 > 1.0) else { return (0.0, vw) };
    if anchor.2 <= s.0 + 1.0 { (0.0, s.0) }
    else if anchor.0 >= s.2 - 1.0 { (s.2, vw) }
    else if vw - s.2 >= s.0 { (s.2, vw) } else { (0.0, s.0) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_seconds_of_rest_explains_once() {
        let mut d = Dwell::default();
        assert_eq!(d.sample(Some("top.tour"), (100.0, 100.0), 0.0), Rest::Nothing);
        assert_eq!(d.sample(Some("top.tour"), (104.0, 98.0), 2_000.0), Rest::Nothing, "jitter is still resting");
        assert_eq!(d.sample(Some("top.tour"), (106.0, 103.0), 4_999.0), Rest::Nothing);
        assert_eq!(d.sample(Some("top.tour"), (106.0, 103.0), 5_000.0), Rest::Explain("top.tour".into()));
        assert_eq!(d.sample(Some("top.tour"), (106.0, 103.0), 9_000.0), Rest::Nothing, "once per rest");
        // moving across the control restarts the clock
        let mut d = Dwell::default();
        d.sample(Some("a.b"), (0.0, 0.0), 0.0);
        d.sample(Some("a.b"), (40.0, 0.0), 3_000.0);
        assert_eq!(d.sample(Some("a.b"), (40.0, 0.0), 6_000.0), Rest::Nothing);
        assert_eq!(d.sample(Some("a.b"), (40.0, 0.0), 8_000.0), Rest::Explain("a.b".into()));
        // a reset (typing, a dialog) starts over too
        let mut d = Dwell::default();
        d.sample(Some("a.b"), (0.0, 0.0), 0.0);
        d.reset(4_000.0);
        assert_eq!(d.sample(Some("a.b"), (0.0, 0.0), 6_000.0), Rest::Nothing);
        // a click means they knew what it was
        let mut d = Dwell::default();
        d.sample(Some("a.b"), (0.0, 0.0), 0.0);
        d.click();
        assert_eq!(d.sample(Some("a.b"), (0.0, 0.0), 6_000.0), Rest::Nothing);
    }

    #[test]
    fn leaving_without_a_click_after_a_while_is_hesitation() {
        let mut d = Dwell::default();
        d.sample(Some("coach.act"), (0.0, 0.0), 0.0);
        assert_eq!(d.sample(None, (300.0, 0.0), 2_000.0), Rest::LeftUnclicked("coach.act".into()));
        d.sample(Some("coach.act"), (0.0, 0.0), 3_000.0);
        assert_eq!(d.sample(Some("coach.hint"), (0.0, 0.0), 3_500.0), Rest::Nothing, "a quick pass is not hesitation");
        d.click();
        assert_eq!(d.sample(None, (0.0, 0.0), 9_000.0), Rest::Nothing, "clicked: not hesitation");
    }

    #[test]
    fn tooltips_stay_on_screen_and_off_the_practice_pane() {
        for (vw, vh) in [(390.0, 844.0), (1024.0, 700.0), (1440.0, 900.0)] {
            for t in [(0.0, 0.0, 32.0, 32.0), (vw - 32.0, 0.0, vw, 32.0), (vw / 2.0, vh - 30.0, vw / 2.0 + 30.0, vh), (0.0, vh - 30.0, 30.0, vh), (vw - 40.0, 300.0, vw, 340.0)] {
                for w in [60.0, 180.0, 300.0] {
                    let (x, y) = tip_spot(t, w, 28.0, vw, vh, &[]);
                    assert!(x >= 5.5 && y >= 5.5 && x + w <= vw - 5.5 && y + 28.0 <= vh - 5.5, "{vw}x{vh} {t:?} w{w}: {x},{y}");
                }
            }
        }
        // a toolbar button over the practice pane: never below it, into the native pane
        let slot = (420.0, 40.0, 1020.0, 800.0);
        let (x, y) = tip_spot((600.0, 6.0, 628.0, 34.0), 90.0, 28.0, 1380.0, 800.0, &[slot]);
        assert!(!(x < slot.2 && x + 90.0 > slot.0 && y < slot.3 && y + 28.0 > slot.1), "{x},{y}");
        // below by default, centred
        let (x, y) = tip_spot((500.0, 100.0, 540.0, 140.0), 100.0, 28.0, 1200.0, 800.0, &[]);
        assert_eq!((x, y), (470.0, 148.0));
    }

    #[test]
    fn bubbles_take_the_side_of_the_practice_pane_their_anchor_is_on() {
        let slot = Some((420.0, 74.0, 1020.0, 800.0));
        assert_eq!(band((1100.0, 700.0, 1200.0, 730.0), 1380.0, slot), (1020.0, 1380.0), "the companion's column");
        assert_eq!(band((80.0, 200.0, 300.0, 240.0), 1380.0, slot), (0.0, 420.0), "the course column");
        assert_eq!(band((600.0, 6.0, 628.0, 34.0), 1380.0, slot), (0.0, 420.0), "the toolbar: the roomier side");
        assert_eq!(band((600.0, 6.0, 628.0, 34.0), 1380.0, None), (0.0, 1380.0));
    }
}
