//! Where Sidekick's bubbles open (a quip, a nudge, a coach mark, hover help): wholly inside the window,
//! below the top bar, never over what must stay usable (the top bar, the text-size control, the page's
//! primary buttons, the thing being explained, Sidekick himself), and over as few of the page's other
//! controls as possible: beside what they point at when there's a clear spot there, else the nearest
//! clear spot. On a phone every bubble is a slim strip across the window, at the bottom unless that
//! would cover a primary button, then under the top bar. sidekick.rs measures each bubble and passes its
//! real size; this is pure, so every corner, edge and reported case is tested natively.
use super::motion::Rect;

/// Gap between the bubble and what it points at; margin from the window's edges.
pub const GAP: f64 = 10.0;
pub const MARGIN: f64 = 12.0;
/// The candidate grid's step: fine enough to find the gaps between a page's controls.
const STEP: f64 = 24.0;
/// A phone strip is never taller than this share of the window (its text scrolls inside).
pub const STRIP_SHARE: f64 = 0.45;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    pub left: f64,
    pub top: f64,
    /// The bubble's width (a strip's exact width; otherwise its max-width) and the tallest it may grow
    /// (its text scrolls inside past that).
    pub w: f64,
    pub max_h: f64,
    /// The tail's distance from the bubble's left edge, pointing at the anchor's centre, when the bubble
    /// sits right above or below the anchor (`pointed`); a bubble further off has no tail.
    pub tail: f64,
    pub pointed: bool,
    /// The bubble is above the anchor (the tail on its bottom edge).
    pub above: bool,
    /// A phone strip: exactly `w` wide.
    pub strip: bool,
}

impl Place {
    /// Nothing to place (the bubble isn't up): off screen, so it can't cover anything, and as wide as any
    /// bubble may be, so a bubble appearing there is measured at its natural width.
    pub const NONE: Place = Place { left: -10_000.0, top: -10_000.0, w: 320.0, max_h: 0.0, tail: 0.0, pointed: false, above: false, strip: false };

    /// Inline CSS for the bubble.
    pub fn css(&self) -> String {
        let tail = if self.pointed { format!("; --tail-l: {:.0}px", self.tail) } else { String::new() };
        let width = if self.strip { format!("; width: {:.0}px", self.w) } else { String::new() };
        format!("left: {:.0}px; top: {:.0}px; max-width: {:.0}px; --max-h: {:.0}px{tail}{width}", self.left, self.top, self.w, self.max_h)
    }

    /// The box a bubble `h` tall occupies here.
    #[cfg(test)]
    pub fn rect(&self, h: f64) -> Rect { (self.left, self.top, self.left + self.w, self.top + h.min(self.max_h)) }
}

pub fn hits(a: Rect, b: Rect) -> bool { a.0 < b.2 && a.2 > b.0 && a.1 < b.3 && a.3 > b.1 }

fn centre_in(b: Rect, r: Rect) -> bool {
    let (x, y) = ((r.0 + r.2) / 2.0, (r.1 + r.3) / 2.0);
    x > b.0 && x < b.2 && y > b.1 && y < b.3
}

/// What the box `b` would cost, or None once it's sure to cost at least `beat`: anything in `hard` it
/// covers outweighs everything else, then each control in `soft` whose centre it covers (a click there
/// would land on the bubble), then each it touches, then its distance from the anchor.
fn cost(b: Rect, anchor: Rect, hard: &[Rect], soft: &[Rect], beat: f64) -> Option<f64> {
    let (al, at, ar, ab) = anchor;
    // best exactly GAP above or below the anchor; closer is cramped, further (or off to one side) is less
    // clearly about it
    let d = (al - b.2).max(b.0 - ar).max(0.0) + (at - b.3).max(b.1 - ab).max(0.0);
    let gap = if d < GAP { (GAP - d) * 4.0 } else { d - GAP } + if al - b.2 > 0.0 || b.0 - ar > 0.0 { 30.0 } else { 0.0 };
    let mut c = hard.iter().filter(|r| hits(b, **r)).count() as f64 * 1e8 + gap;
    for r in soft {
        if c >= beat { return None; }
        if hits(b, *r) { c += if centre_in(b, *r) { 1e5 } else { 400.0 } }
    }
    (c < beat).then_some(c)
}

/// Where a bubble `w` wide and `h` tall pointing at `anchor` goes, inside the band (left, right) of a
/// `vh`-tall window and no higher than `y_min` (the top bar's bottom): right below or above the anchor,
/// or beside it, when that spot is clear of `hard` and of the controls in `soft`; otherwise the clear
/// spot nearest the anchor anywhere in the band; with no clear spot at all, the one covering least.
pub fn fit(anchor: Rect, w: f64, h: f64, band: (f64, f64), vh: f64, y_min: f64, hard: &[Rect], soft: &[Rect]) -> Place {
    let (bl, br) = band;
    let w = w.min(br - bl - 2.0 * MARGIN).max(0.0).round();
    let top_lim = y_min.max(MARGIN);
    let h = h.min(vh - MARGIN - top_lim).max(0.0).round();
    let (lo_x, hi_x) = (bl + MARGIN, (br - MARGIN - w).max(bl + MARGIN));
    let (lo_y, hi_y) = (top_lim, (vh - MARGIN - h).max(top_lim));
    let (al, at, ar, ab) = anchor;
    let cx = (al + ar) / 2.0;
    // beside the anchor first (its right edge at the anchor's, its left at the anchor's, centred; below,
    // then above), then just past each thing to keep clear, then a grid, then level with the anchor
    let mut xs = vec![ar - w, al, cx - w / 2.0];
    let mut ys = vec![ab + GAP, at - GAP - h];
    let mut sides = vec![(al - GAP - w, (at + ab) / 2.0 - h / 2.0), (ar + GAP, (at + ab) / 2.0 - h / 2.0), (al - GAP - w, at), (ar + GAP, at)];
    // the gaps between rows of controls are where a bubble fits: just above or below each one
    for r in hard { xs.extend([r.2 + GAP, r.0 - GAP - w]) }
    for r in hard.iter().chain(soft) { ys.extend([r.3 + GAP, r.1 - GAP - h]) }
    let mut x = lo_x;
    while x < hi_x { xs.push(x); x += STEP }
    let mut y = lo_y;
    while y < hi_y { ys.push(y); y += STEP }
    xs.push(hi_x);
    ys.push(hi_y);
    let snap = |v: &mut Vec<f64>, lo: f64, hi: f64| { for x in v.iter_mut() { *x = x.clamp(lo, hi).round() } };
    snap(&mut xs, lo_x, hi_x);
    snap(&mut ys, lo_y, hi_y);
    // the beside-the-anchor spots stay first (ties keep the earlier spot); the rest only once each
    let dedup = |v: &mut Vec<f64>| { let mut seen = std::collections::BTreeSet::new(); v.retain(|x| seen.insert(*x as i64)) };
    dedup(&mut xs);
    dedup(&mut ys);
    let mut best: Option<(f64, f64, f64)> = None;
    let mut consider = |x: f64, y: f64| {
        let (x, y) = (x.clamp(lo_x, hi_x).round(), y.clamp(lo_y, hi_y).round());
        let beat = best.map_or(f64::INFINITY, |b| b.0);
        if let Some(c) = cost((x, y, x + w, y + h), anchor, hard, soft, beat) { best = Some((c, x, y)) }
    };
    for &y in &ys { for &x in &xs { consider(x, y) } }
    for (x, y) in sides.drain(..) { consider(x, y) }
    let (_, left, top) = best.unwrap_or((0.0, lo_x, lo_y));
    // a tail only when the bubble sits right above or below the anchor, over its centre
    let below = (top - ab).abs() <= GAP + 1.0;
    let above = (at - (top + h)).abs() <= GAP + 1.0;
    let pointed = (below || above) && cx >= left + 16.0 && cx <= left + w - 16.0;
    Place { left, top, w, max_h: h, tail: cx - left, pointed, above: above && !below, strip: false }
}

/// On a phone: a strip `h` tall (capped at `STRIP_SHARE` of the window) across the band (left, right),
/// at the bottom, resting on `floor` (above the text-size control); when that would cover anything in
/// `hard` (a primary button, the next step), under the top bar (`y_min`); failing both, the lowest spot
/// clear of `hard`; with none, the one covering least.
pub fn strip(h: f64, band: (f64, f64), vh: f64, y_min: f64, floor: f64, hard: &[Rect]) -> Place {
    let (bl, br) = band;
    let w = (br - bl - 2.0 * MARGIN).max(0.0).round();
    let top_lim = y_min.max(MARGIN);
    let floor = floor.min(vh - MARGIN);
    let h = h.min(vh * STRIP_SHARE).min((floor - top_lim).max(0.0)).max(0.0).round();
    let left = bl + MARGIN;
    let bottom = (floor - h).max(top_lim);
    let count = |t: f64| hard.iter().filter(|r| hits((left, t, left + w, t + h), **r)).count();
    let mut tries = vec![bottom, top_lim];
    let mut t = bottom - STEP;
    while t > top_lim { tries.push(t); t -= STEP }
    let top = tries.iter().copied().find(|&t| count(t) == 0)
        .unwrap_or_else(|| tries.iter().copied().min_by_key(|&t| count(t)).unwrap_or(bottom));
    Place { left, top, w, max_h: h, tail: 0.0, pointed: false, above: false, strip: true }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sidekick::{SIZE, SPOT_MARGIN};

    /// The text-size control, bottom left (.zoom), as the app lays it out.
    fn zoom(vh: f64) -> Rect { (20.0, vh - 60.0, 148.0, vh - 20.0) }
    fn inside(b: Rect, band: (f64, f64), vh: f64, y_min: f64) -> bool {
        b.0 >= band.0 + MARGIN - 0.5 && b.2 <= band.1 - MARGIN + 0.5 && b.1 >= y_min.max(MARGIN) - 0.5 && b.3 <= vh - MARGIN + 0.5
    }

    #[test]
    fn bubbles_stay_in_the_window_at_every_corner_and_edge() {
        for (vw, vh) in [(800.0, 600.0), (1024.0, 768.0), (1440.0, 900.0)] {
            let (l, r) = (SPOT_MARGIN, vw - SIZE - SPOT_MARGIN);
            let (t, b) = (72.0, vh - SIZE - SPOT_MARGIN);
            let (cx, cy) = ((vw - SIZE) / 2.0, (vh - SIZE) / 2.0);
            let spots = [(l, t), (r, t), (l, b), (r, b), (cx, t), (cx, b), (l, cy), (r, cy), (cx, cy), (20.0, vh - 60.0 - SIZE - 8.0)];
            let bar = (0.0, 0.0, vw, 64.0);
            for (x, y) in spots {
                for (w, h) in [(260.0, 44.0), (320.0, 140.0), (320.0, 400.0)] {
                    let sidekick = (x, y, x + SIZE, y + SIZE);
                    let p = fit(sidekick, w, h, (0.0, vw), vh, 64.0 + GAP, &[zoom(vh), bar, sidekick], &[]);
                    let bx = p.rect(h);
                    assert!(inside(bx, (0.0, vw), vh, 64.0 + GAP), "{vw}x{vh} sidekick {x},{y} {w}x{h}: {p:?}");
                    for a in [zoom(vh), bar, sidekick] { assert!(!hits(bx, a), "{vw}x{vh} sidekick {x},{y} {w}x{h}: {bx:?} over {a:?}") }
                    if p.pointed { assert!(p.tail >= 16.0 && p.tail <= p.w - 16.0) }
                }
            }
        }
    }

    #[test]
    fn opens_beside_what_it_points_at_when_that_is_clear() {
        // Sidekick at the right edge: the bubble opens below him, its right edge at his, the tail on him
        let sidekick = (1000.0 - 80.0, 300.0, 1000.0 - 8.0, 372.0);
        let p = fit(sidekick, 320.0, 120.0, (0.0, 1000.0), 700.0, 0.0, &[sidekick], &[]);
        assert_eq!((p.left + p.w, p.top), (sidekick.2.min(1000.0 - MARGIN), sidekick.3 + GAP), "{p:?}");
        assert!(p.pointed && !p.above && (p.left + p.tail - (sidekick.0 + sidekick.2) / 2.0).abs() < 1.0);
        // near the bottom it opens above
        let low = (500.0, 600.0, 572.0, 672.0);
        let p = fit(low, 320.0, 120.0, (0.0, 1000.0), 700.0, 0.0, &[low], &[]);
        assert!(p.above && p.pointed && p.top + 120.0 <= low.1, "{p:?}");
        assert!(p.css().contains("--tail-l"));
    }

    #[test]
    fn keeps_off_the_top_bar_primary_buttons_and_controls() {
        // the reported cases: at 800 and 1024 the quip/nudge at the tab (Sidekick peeking at y 10) opened over a
        // wrapped 108 px top bar; a quip over Save as PDF; hover help over the quiz options it explained
        for (vw, vh, bar_h) in [(800.0, 600.0, 108.0), (1024.0, 768.0, 108.0), (1440.0, 900.0, 64.0)] {
            let peek = (vw - 58.0, 10.0, vw - 6.0, 62.0);
            let bar = (0.0, 0.0, vw - 40.0, bar_h);
            let save = (vw - 300.0, bar_h + 30.0, vw - 60.0, bar_h + 74.0);
            for (w, h) in [(260.0, 48.0), (320.0, 150.0)] {
                let p = fit(peek, w, h, (0.0, vw - 40.0), vh, bar_h + GAP, &[bar, save, zoom(vh), peek], &[]);
                let b = p.rect(h);
                assert!(inside(b, (0.0, vw - 40.0), vh, bar_h + GAP), "{vw}: {p:?}");
                for a in [bar, save, zoom(vh)] { assert!(!hits(b, a), "{vw} {w}x{h}: {b:?} over {a:?}") }
            }
        }
        // hover help on the first of five quiz options: it never covers the centre of an option
        let opts: Vec<Rect> = (0..5).map(|i| (300.0, 200.0 + 70.0 * i as f64, 900.0, 256.0 + 70.0 * i as f64)).collect();
        let submit = (300.0, 560.0, 420.0, 604.0);
        let ring = (294.0, 194.0, 906.0, 262.0);
        let p = fit(ring, 320.0, 160.0, (0.0, 1024.0), 768.0, 74.0, &[submit, ring, zoom(768.0)], &opts);
        let b = p.rect(160.0);
        assert!(!opts.iter().any(|o| centre_in(b, *o)) && !hits(b, submit) && !hits(b, ring), "{b:?}");
    }

    #[test]
    fn a_bubble_in_a_band_stays_in_it() {
        // Sidekick's column right of the practice pane (1020..1380): the bubble never crosses into the pane
        for anchor in [(1100.0, 600.0, 1200.0, 640.0), (1300.0, 60.0, 1370.0, 100.0), (1030.0, 400.0, 1060.0, 430.0)] {
            let p = fit(anchor, 320.0, 140.0, (1020.0, 1380.0), 800.0, 0.0, &[anchor], &[]);
            assert!(inside(p.rect(140.0), (1020.0, 1380.0), 800.0, 0.0), "{anchor:?}: {p:?}");
        }
        let p = fit((60.0, 300.0, 200.0, 340.0), 320.0, 140.0, (0.0, 420.0), 800.0, 0.0, &[], &[]);
        assert!(inside(p.rect(140.0), (0.0, 420.0), 800.0, 0.0), "{p:?}");
    }

    #[test]
    fn too_tall_for_the_window_it_scrolls_inside() {
        let p = fit((500.0, 300.0, 572.0, 372.0), 320.0, 2000.0, (0.0, 1000.0), 600.0, 80.0, &[], &[]);
        assert!(p.top >= 80.0 && p.top + p.max_h <= 600.0 - MARGIN + 0.5, "{p:?}");
    }

    #[test]
    fn a_phone_bubble_is_a_strip_clear_of_primary_buttons() {
        let (vw, vh, bar) = (390.0, 844.0, 182.0);
        let band = (0.0, vw - 40.0);
        let floor = zoom(vh).1 - GAP;
        // nothing at the bottom: the strip rests on the text-size control
        let p = strip(120.0, band, vh, bar, floor, &[zoom(vh)]);
        let b = p.rect(120.0);
        assert!(p.strip && (b.3 - floor).abs() < 0.5 && b.2 <= vw - 40.0 - MARGIN + 0.5 && b.0 >= MARGIN - 0.5, "{b:?}");
        assert!(p.css().contains("width: ") && !p.css().contains("--tail-l"));
        // the reported case: Resubmit at the bottom of a graded check: the strip goes under the top bar
        let resubmit = (16.0, 620.0, 150.0, 664.0);
        let p = strip(120.0, band, vh, bar, floor, &[zoom(vh), resubmit]);
        let b = p.rect(120.0);
        assert!(!hits(b, resubmit) && (b.1 - bar).abs() < 0.5, "{b:?}");
        // primary buttons at both ends: the lowest clear spot between them
        let save = (16.0, 200.0, 200.0, 244.0);
        let p = strip(120.0, band, vh, bar, floor, &[zoom(vh), resubmit, save]);
        let b = p.rect(120.0);
        assert!(!hits(b, resubmit) && !hits(b, save) && b.1 >= bar, "{b:?}");
        // large text on a phone: a tall bubble is capped and scrolls inside
        assert!(strip(900.0, band, vh, bar, floor, &[]).max_h <= vh * STRIP_SHARE + 0.5);
    }
}
