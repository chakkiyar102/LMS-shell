//! Sidekick the companion: the small Sidekick that moves around the course, nudges at the right moments,
//! and takes the learner places (destination chips). Pure helpers first,
//! tested natively; the components follow.
use std::collections::BTreeMap;

pub mod coach;
pub mod familiar;
pub mod help;
pub mod lines;
pub mod motion;
pub mod pane;
pub mod place;
pub mod tricks;
pub mod stuck;

/// Settings → Sidekick companion. Quiet: docked, no movement, no unprompted nudges. Off: no sprite.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Mode { On, Quiet, Off }

pub const MODES: [(Mode, &str, &str); 3] = [(Mode::On, "on", "On"), (Mode::Quiet, "quiet", "Quiet"), (Mode::Off, "off", "Off")];

/// `sidekick_mode` from storage; anything unexpected means On.
pub fn parse_mode(v: Option<&str>) -> Mode {
    MODES.iter().find(|(_, k, _)| Some(*k) == v).map(|(m, _, _)| *m).unwrap_or(Mode::On)
}

pub fn mode_str(m: Mode) -> &'static str { MODES.iter().find(|(x, _, _)| *x == m).map(|(_, k, _)| *k).unwrap_or("on") }

pub const SNOOZE_MS: f64 = 7.0 * 24.0 * 3_600_000.0;

/// `sidekick_snooze`: `{"<course>/<trigger>": until_ms}`. Bad JSON → empty; expired entries dropped.
pub fn parse_snooze(v: Option<&str>, now_ms: f64) -> BTreeMap<String, f64> {
    let m: BTreeMap<String, f64> = v.and_then(|s| serde_json::from_str(s).ok()).unwrap_or_default();
    m.into_iter().filter(|(_, until)| until.is_finite() && *until > now_ms).collect()
}

pub fn snooze_str(m: &BTreeMap<String, f64>) -> String { serde_json::to_string(m).unwrap_or_else(|_| "{}".into()) }

/// Triggers snoozed for `course`, from the stored map.
pub fn snoozed_for(m: &BTreeMap<String, f64>, course: &str) -> Vec<String> {
    m.keys().filter_map(|k| k.strip_prefix(course).and_then(|r| r.strip_prefix('/'))).map(str::to_string).collect()
}

/// `sidekick_shown`: unprompted nudge times (ms) in the rolling last hour. Bad JSON → empty.
pub fn parse_shown(v: Option<&str>, now_ms: f64) -> Vec<f64> {
    let all: Vec<f64> = v.and_then(|s| serde_json::from_str(s).ok()).unwrap_or_default();
    shared::recent(&all, now_ms)
}

pub fn shown_str(v: &[f64]) -> String { serde_json::to_string(v).unwrap_or_else(|_| "[]".into()) }

/// Turnaround views (ui/assets/mascot/view-*.png), named by the way Sidekick faces on screen.
/// They are fetched once the companion has settled, not at start-up (`prefetch_views`).
pub const VIEWS: [&str; 5] = ["view-side-left", "view-side-right", "view-three-quarter-left", "view-three-quarter-right", "view-back"];
#[cfg(test)]
pub const VIEW_BACK: &str = "view-back";

/// The side view facing the way Sidekick travels from `from_x` to `to_x`.
pub fn travel_view(from_x: f64, to_x: f64) -> &'static str {
    if to_x < from_x { "view-side-left" } else { "view-side-right" }
}

/// The three-quarter view for an idle glance toward the bubble: its centre `toward_x` against
/// Sidekick's centre `sidekick_x`. The bubble opens to Sidekick's left unless it sits clearly to the right.
#[cfg(test)]
pub fn glance_view(toward_x: f64, sidekick_x: f64) -> &'static str {
    if toward_x > sidekick_x + 8.0 { "view-three-quarter-right" } else { "view-three-quarter-left" }
}

/// Sidekick keeps this far from the window's edges, and this far from the edges of what it walks to.
pub const SPOT_MARGIN: f64 = 8.0;
pub const SPOT_GAP: f64 = 12.0;

/// Where Sidekick stands for target `t` (left, top, right, bottom, in viewport px): beside it on the right,
/// then on the left; failing both, and only when `inside_ok`, inside it at the right end, clear of its
/// edges (the certificate's to-do card in a narrow window). The spot is always wholly on screen and
/// never straddles the target's edge; `blocked(x, y)` vetoes a spot over something protected.
/// `y_top` is the highest Sidekick may stand: just under the top bar (`top_min`). Returns (x, y, points_left).
pub fn pick_spot(t: (f64, f64, f64, f64), vw: f64, vh: f64, y_top: f64, inside_ok: bool, blocked: impl Fn(f64, f64) -> bool) -> Option<(f64, f64, bool)> {
    let (l, top, r, b) = t;
    let (y_min, y_max) = (y_top, vh - SIZE - SPOT_MARGIN);
    let fits_x = |x: f64| x >= SPOT_MARGIN && x + SIZE <= vw - SPOT_MARGIN;
    if y_min <= y_max {
        let y = (top + (b - top - SIZE) / 2.0).clamp(y_min, y_max);
        if let Some(spot) = [(r + SPOT_GAP, true), (l - SPOT_GAP - SIZE, false)].into_iter()
            .find(|&(x, _)| fits_x(x) && !blocked(x, y)).map(|(x, left)| (x, y, left)) {
            return Some(spot);
        }
    }
    if !inside_ok { return None; }
    let x = r.min(vw - SPOT_MARGIN) - SPOT_GAP - SIZE;
    let (lo, hi) = ((top + SPOT_GAP).max(y_min), (b - SPOT_GAP - SIZE).min(y_max));
    if x < l + SPOT_GAP || !fits_x(x) || lo > hi { return None; }
    let y = ((top.max(y_min) + b.min(vh)) / 2.0 - SIZE / 2.0).clamp(lo, hi);
    (!blocked(x, y)).then_some((x, y, true))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_falls_back_to_on() {
        assert_eq!(parse_mode(Some("quiet")), Mode::Quiet);
        assert_eq!(parse_mode(Some("off")), Mode::Off);
        for bad in [None, Some(""), Some("OFF"), Some("loud")] { assert_eq!(parse_mode(bad), Mode::On, "{bad:?}") }
        for (m, _, _) in MODES { assert_eq!(parse_mode(Some(mode_str(m))), m) }
    }

    #[test]
    fn snooze_round_trips_and_expires() {
        let now = 1_000_000.0;
        let mut m = BTreeMap::new();
        m.insert("cd/lesson_done_next".to_string(), now + SNOOZE_MS);
        m.insert("cd/studied_today".to_string(), now - 1.0);
        m.insert("adm/check_failed".to_string(), now + 5.0);
        let back = parse_snooze(Some(&snooze_str(&m)), now);
        assert_eq!(back.len(), 2);
        assert_eq!(snoozed_for(&back, "cd"), vec!["lesson_done_next".to_string()]);
        assert_eq!(snoozed_for(&back, "c"), Vec::<String>::new());
        for bad in [None, Some("nope"), Some("[1,2]"), Some("{\"a\":\"x\"}")] { assert!(parse_snooze(bad, now).is_empty(), "{bad:?}") }
    }

    #[test]
    fn lessons_resolve_by_id_or_number() {
        use shared::{CourseView, LessonSummary, SessionView};
        let l = |id: &str, n: &str| LessonSummary { id: id.into(), number: n.into(), title: "T".into(), duration_min: 5, has_video: false, completed: false };
        let s = |id: &str, ls| SessionView { id: id.into(), phase: None, title: "S".into(), outcome: String::new(), lessons: ls, check: format!("{id}-check"), exercise: String::new(), check_best: None };
        let cv = CourseView { id: "c".into(), title: "C".into(), summary: String::new(), final_minutes: 60, final_blurb: String::new(),
            sessions: vec![s("s1", vec![l("1-1-intro", "1.1")]), s("s2", vec![l("2-3-forms", "2.3"), l("adm-2-4-groups", "2.4")])] };
        assert_eq!(super::find_lesson(&cv, "adm-2-4-groups").map(|l| l.number.as_str()), Some("2.4"));
        assert_eq!(super::find_lesson(&cv, "2.3").map(|l| l.id.as_str()), Some("2-3-forms"));
        assert_eq!(super::find_lesson(&cv, "Lesson 2.3").map(|l| l.id.as_str()), Some("2-3-forms"));
        assert_eq!(super::find_lesson(&cv, "1-1-intro").map(|l| l.number.as_str()), Some("1.1"));
        assert!(super::find_lesson(&cv, "9.9").is_none() && super::find_lesson(&cv, "").is_none());
    }

    #[test]
    fn turnaround_views_face_the_right_way() {
        assert_eq!(travel_view(900.0, 200.0), "view-side-left");
        assert_eq!(travel_view(200.0, 900.0), "view-side-right");
        assert_eq!(glance_view(400.0, 1100.0), "view-three-quarter-left");
        assert_eq!(glance_view(1300.0, 1100.0), "view-three-quarter-right");
        assert_eq!(glance_view(1104.0, 1100.0), "view-three-quarter-left");
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/mascot");
        for v in VIEWS { assert!(dir.join(format!("{v}.png")).is_file(), "{v}.png missing"); }
        assert!(VIEWS.contains(&VIEW_BACK) && VIEWS.contains(&travel_view(0.0, 1.0)) && VIEWS.contains(&glance_view(0.0, 1.0)));
        // a view that fails to load falls back to the plain idle pose
        for v in VIEWS { assert_eq!(shared::pose_fallback(v), "idle"); }
    }

    #[test]
    fn walk_target_stays_on_screen_and_clear_of_card_edges() {
        let free = |_: f64, _: f64| false;
        let card = (290.0, 326.0, 1150.0, 675.0);
        // wide: beside the card on the right, a clear gap from its edge
        let (x, y, left) = pick_spot(card, 1440.0, 900.0, 72.0, true, free).unwrap();
        assert!(left && x >= card.2 + SPOT_GAP && x + SIZE <= 1440.0 - SPOT_MARGIN && y >= 72.0, "{x} {y}");
        // narrow (the certificate's to-do card at 1000 px): no room beside, so inside the card, clear of its edges
        let narrow = (70.0, 326.0, 930.0, 675.0);
        let (x, y, left) = pick_spot(narrow, 1000.0, 700.0, 72.0, true, free).unwrap();
        assert!(left && x >= narrow.0 + SPOT_GAP && x + SIZE <= narrow.2 - SPOT_GAP, "{x}");
        assert!(y >= narrow.1 + SPOT_GAP && y + SIZE <= (narrow.3 - SPOT_GAP).min(700.0 - SPOT_MARGIN), "{y}");
        assert_eq!(pick_spot(narrow, 1000.0, 700.0, 72.0, false, free), None, "not inside unless the anchor allows it");
        assert_eq!(pick_spot(card, 1440.0, 900.0, 72.0, true, |_, _| true), None);
        // never straddles the target's edge or leaves the viewport, at any size
        for (vw, vh) in [(900.0, 600.0), (1000.0, 700.0), (1280.0, 840.0), (1440.0, 900.0), (1920.0, 1080.0)] {
            for t in [card, narrow, (16.0, 80.0, vw - 16.0, vh + 300.0), (600.0, 500.0, 700.0, 540.0), (vw - 120.0, 100.0, vw - 10.0, 160.0)] {
                if let Some((x, y, _)) = pick_spot(t, vw, vh, 72.0, true, free) {
                    assert!(x >= SPOT_MARGIN && x + SIZE <= vw - SPOT_MARGIN && y >= 72.0 && y + SIZE <= vh - SPOT_MARGIN, "{vw}x{vh} {t:?}: {x},{y}");
                    let outside = x >= t.2 + SPOT_GAP || x + SIZE <= t.0 - SPOT_GAP || y >= t.3 + SPOT_GAP || y + SIZE <= t.1 - SPOT_GAP;
                    let inside = x >= t.0 + SPOT_GAP && x + SIZE <= t.2 - SPOT_GAP && y >= t.1 + SPOT_GAP && y + SIZE <= t.3 - SPOT_GAP;
                    assert!(outside || inside, "{vw}x{vh} {t:?}: {x},{y} straddles an edge");
                }
            }
        }
    }

    #[test]
    fn every_pose_the_companion_shows_is_shipped() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/mascot");
        let tricks = tricks::TRICKS.map(|t| t.pose());
        let named = ["idle", "point-left", "point-right", "peek", "hop", "celebrate", "wave", "encourage", "thumbsup", "sleepy", VIEW_BACK];
        for p in EXTRA_POSES.iter().chain(tricks.iter()).chain(named.iter()) {
            assert!(dir.join(format!("{p}.png")).is_file(), "{p}.png missing");
            assert!(dir.join(format!("{}.png", fallback(p))).is_file(), "{p}: fallback missing");
        }
        assert_eq!(fallback("fly"), "hop");
        assert_eq!(fallback("goodbye"), "wave", "shared fallbacks still apply");
        let kb: u64 = EXTRA_POSES.iter().map(|p| std::fs::metadata(dir.join(format!("{p}.png"))).unwrap().len()).sum::<u64>() / 1024;
        assert!(kb < 800, "companion-only poses are {kb} KB; ship them at 256 px");
    }

    #[test]
    fn pointing_direction() {
        let t = (400.0, 300.0, 800.0, 360.0);
        assert_eq!(point_pose((820.0, 290.0), t), "point-left");
        assert_eq!(point_pose((300.0, 290.0), t), "point-right");
        assert_eq!(point_pose((700.0, 400.0), t), "point-up");
        assert_eq!(point_pose((700.0, 180.0), t), "point-down");
    }

    #[test]
    fn shown_keeps_the_last_hour() {
        let now = 10.0 * 3_600_000.0;
        let s = shown_str(&[now - 3_700_000.0, now - 60_000.0]);
        assert_eq!(parse_shown(Some(&s), now), vec![now - 60_000.0]);
        assert!(parse_shown(Some("{}"), now).is_empty());
        assert!(parse_shown(None, now).is_empty());
    }
}

// ---------------------------------------------------------------------------------------------
// The companion component
// ---------------------------------------------------------------------------------------------
use crate::{android, api, ctx, storage, Ctx, Icon, Page, Pmode};
use familiar::{Area, Familiar, Signal as Sig};
use help::{Dwell, Rest};
use stuck::Kind;
use leptos::{prelude::*, task::spawn_local};
use serde_json::json;
use shared::*;
use wasm_bindgen::JsCast;
use coach::{Mark, ShowMe};
use motion::{Pt, Travel};
use tricks::{Tap, Taps, Trick};

/// Sprite size in CSS px; the dock sits directly above the Ask Sidekick launcher (right 24, bottom 96).
pub const SIZE: f64 = 72.0;
const DOCK_RIGHT: f64 = 24.0;
const DOCK_BOTTOM: f64 = 96.0;
/// Sidekick never sits over these: video, quizzes and exercise answers, the text being read, Ask Sidekick.
const AVOID: &str = "video, .quiz, .prose, [data-sidekick-protected], .news, .writeup, .zoom, .sidekick-side-pane, .sidekick-tab-wrap, .practice-slot, .sidekick-coach, .sidekick-bubble";
/// Nor over anything the learner reads or clicks (checked where he lands, perches or is dropped).
const CONTENT: &str = "p, h1, h2, h3, h4, li, button, a, input, textarea, select, label, img, video, figure, .chip, .streak-pill";

/// What he never stands on, even inside a card that lets him in.
const CONTROLS: &str = "button, a, input, textarea, select";

/// Poses only the companion uses (flight, tricks, pointing up and down); fetched once start-up has settled.
pub const EXTRA_POSES: [&str; 10] = ["fly", "takeoff", "landing", "dizzy", "laugh", "salute", "wink", "oops", "point-up", "point-down"];

/// The picture to show when `pose` fails to load: the companion's own poses first, then the shared rule.
pub fn fallback(pose: &str) -> &'static str {
    match pose {
        "fly" | "takeoff" | "landing" => "hop",
        "dizzy" | "oops" => "thinking",
        "laugh" => "celebrate",
        "salute" => "wave",
        "wink" => "thumbsup",
        "point-up" | "point-down" => "pointing",
        p => pose_fallback(p),
    }
}

/// Which arrival pose points at a target from Sidekick's spot: sideways, or up/down when it's mostly above or below.
pub fn point_pose(sidekick: Pt, target: motion::Rect) -> &'static str {
    let (cx, cy) = (sidekick.0 + SIZE / 2.0, sidekick.1 + SIZE / 2.0);
    let (tx, ty) = ((target.0 + target.2) / 2.0, (target.1 + target.3) / 2.0);
    let beside = cx < target.0 || cx > target.2;
    if !beside && ty < cy { "point-up" } else if !beside { "point-down" } else if tx < cx { "point-left" } else { "point-right" }
}

pub fn guide_page(p: &Page) -> GuidePage {
    match p {
        Page::Course => GuidePage::Home,
        Page::Session(_) => GuidePage::Session,
        Page::Lesson(_) => GuidePage::Lesson,
        Page::Check(_) => GuidePage::Check,
        Page::Final => GuidePage::Final,
        Page::Exercise(_) => GuidePage::Exercise,
        Page::Certificate => GuidePage::Certificate,
        Page::Settings => GuidePage::Settings,
    }
}

fn coach_on(p: &Page) -> coach::On {
    match p {
        Page::Course => coach::On::Home,
        Page::Lesson(_) => coach::On::Lesson,
        Page::Exercise(_) => coach::On::Exercise,
        Page::Check(_) | Page::Final => coach::On::Check,
        Page::Certificate => coach::On::Certificate,
        Page::Settings => coach::On::Settings,
        Page::Session(_) => coach::On::Any,
    }
}

fn now() -> f64 { js_sys::Date::now() }
fn today() -> String { String::from(js_sys::Date::new_0().to_date_string()) }
fn get(key: &str) -> Option<String> { storage().and_then(|s| s.get_item(key).ok().flatten()) }
fn put(key: &str, v: &str) { if let Some(s) = storage() { let _ = s.set_item(key, v); } }

pub fn reduced_motion() -> bool {
    window().match_media("(prefers-reduced-motion: reduce)").ok().flatten().is_some_and(|m| m.matches())
}

fn video_playing() -> bool {
    let Ok(list) = document().query_selector_all("video") else { return false };
    (0..list.length()).filter_map(|i| list.item(i)).filter_map(|n| n.dyn_into::<web_sys::HtmlMediaElement>().ok()).any(|v| !v.paused())
}

fn typing() -> bool {
    document().active_element().is_some_and(|el| {
        matches!(el.tag_name().as_str(), "INPUT" | "TEXTAREA" | "SELECT") || el.dyn_ref::<web_sys::HtmlElement>().is_some_and(|h| h.is_content_editable())
    })
}

fn viewport() -> (f64, f64) {
    let w = window();
    (w.inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(1280.0), w.inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(840.0))
}

pub fn dock_xy() -> (f64, f64) {
    let (w, h) = viewport();
    // centred over the 68 px launcher below it
    (w - DOCK_RIGHT - 68.0 + (68.0 - SIZE) / 2.0, h - DOCK_BOTTOM - SIZE)
}

/// How far the course has scrolled (the window or the course pane, whichever scrolls).
fn scroll_pos() -> f64 {
    window().scroll_y().unwrap_or(0.0) + document().get_element_by_id("course-pane").map(|e| e.scroll_top() as f64).unwrap_or(0.0)
}

fn rect(sel: &str) -> Option<web_sys::DomRect> {
    document().query_selector(sel).ok().flatten().map(|e| e.get_bounding_client_rect()).filter(|r| r.width() > 0.0 && r.height() > 0.0)
}

/// The highest Sidekick may stand: just under the top bar as it is laid out now (taller when it wraps on a
/// narrow window or with large text); under the column heads (40 px) in a session, when the bar is hidden.
pub fn top_min() -> f64 {
    rect("header.top:not(.hidden)").map(|r| r.bottom().max(0.0)).unwrap_or(40.0) + SPOT_MARGIN
}

/// At or below this width every bubble is a slim strip across the window (sidekick/place.rs `strip`).
pub const PHONE_W: f64 = 600.0;

/// What a bubble keeps clear of. Hard, never covered: the top bar, the text-size control, the session
/// rail, Sidekick's tab, and the page's primary buttons. Soft, covered as little as possible (with `soft`):
/// the page's other controls and the open sidekick's (a bubble may sit over the pane's empty middle:
/// it's Sidekick talking either way, and beside a narrow page there's sometimes nowhere else).
fn keep_clear(soft: bool) -> (Vec<motion::Rect>, Vec<motion::Rect>) {
    let (vw, vh) = viewport();
    let all = |sel: &str, page_only: bool| -> Vec<motion::Rect> {
        let Ok(list) = document().query_selector_all(sel) else { return vec![] };
        (0..list.length()).filter_map(|i| list.item(i)).filter_map(|n| n.dyn_into::<web_sys::Element>().ok())
            .filter(|e| !page_only || e.closest(".sidekick-companion-root").ok().flatten().is_none())
            .map(|e| e.get_bounding_client_rect())
            .filter(|r| r.width() > 0.0 && r.height() > 0.0 && r.bottom() > 0.0 && r.top() < vh && r.right() > 0.0 && r.left() < vw)
            .map(|r| (r.left(), r.top(), r.right(), r.bottom())).collect()
    };
    let mut hard = all("header.top:not(.hidden), .zoom:not(.hidden), .srail, .sidekick-tab-wrap, .sidekick-side-pane .pane-head", false);
    hard.extend(all(".btn.primary", true));
    // the page's controls out of sight under the open sidekick don't count; the sidekick's own do
    let pane = rect(".sidekick-side-pane").map(|r| (r.left(), r.top(), r.right(), r.bottom()));
    let seen = |r: &motion::Rect| pane.is_none_or(|p| { let (x, y) = ((r.0 + r.2) / 2.0, (r.1 + r.3) / 2.0); !(x > p.0 && x < p.2 && y > p.1 && y < p.3) });
    let soft = if soft { let mut v: Vec<_> = all(KEEP_SOFT, true).into_iter().filter(seen).collect(); v.extend(all(KEEP_SOFT_PANE, false)); v } else { vec![] };
    (hard, soft)
}
const KEEP_SOFT: &str = "button, a[href], input:not([type=hidden]), textarea, select, [role=button], [role=option], summary, label";
const KEEP_SOFT_PANE: &str = ".sidekick-side-pane :is(button, a[href], input, textarea, select, [role=button], label)";

/// A bubble's size as laid out now: its width, and its height with nothing scrolled inside it.
fn natural(sel: &str) -> Option<(f64, f64)> {
    let el = document().query_selector(sel).ok().flatten()?.dyn_into::<web_sys::HtmlElement>().ok()?;
    let (w, h) = (el.offset_width() as f64, el.offset_height() as f64);
    if w <= 0.0 || h <= 0.0 { return None; }
    let more = el.query_selector(".sidekick-body").ok().flatten().and_then(|b| b.dyn_into::<web_sys::HtmlElement>().ok())
        .map(|b| (b.scroll_height() - b.client_height()).max(0) as f64).unwrap_or(0.0);
    Some((w.round(), (h + more).round()))
}

fn on_screen(r: &web_sys::DomRect) -> bool {
    let (_, h) = viewport();
    r.bottom() > top_min() - SPOT_MARGIN && r.top() < h - 8.0
}

fn hits(sel: &str, x: f64, y: f64, inset: f64) -> bool {
    let Ok(list) = document().query_selector_all(sel) else { return false };
    (0..list.length()).filter_map(|i| list.item(i)).filter_map(|n| n.dyn_into::<web_sys::Element>().ok()).any(|e| {
        if e.closest(".sidekick-companion-root").ok().flatten().is_some() { return false; }
        let r = e.get_bounding_client_rect();
        r.width() > 0.0 && x + inset < r.right() && x + SIZE - inset > r.left() && y + inset < r.bottom() && y + SIZE - inset > r.top()
    })
}

fn overlaps_avoided(x: f64, y: f64) -> bool { hits(AVOID, x, y, 0.0) }

/// Sidekick at (x, y) would cover text or a control (his picture has ~8 px of clear margin).
fn covers_content(x: f64, y: f64) -> bool { hits(CONTENT, x, y, 8.0) }

/// A spot beside `r` (see `pick_spot`) that is on screen and covers nothing protected, read or
/// clickable; inside `r` as a last resort when `inside_ok`. Returns the spot and whether Sidekick then points left.
pub fn spot_beside(r: &web_sys::DomRect, inside_ok: bool) -> Option<(f64, f64, bool)> {
    if !on_screen(r) { return None; }
    let (w, h) = viewport();
    let t = (r.left(), r.top(), r.right(), r.bottom());
    // standing inside a roomy card (the to-do list) he may cover its text, never its buttons
    pick_spot(t, w, h, top_min(), inside_ok, |x, y| overlaps_avoided(x, y) || if inside_ok { hits(CONTROLS, x, y, 8.0) } else { covers_content(x, y) })
        .or_else(|| {
            // no room beside: just above or below its right end, if that's clear
            [r.top() - SPOT_GAP - SIZE, r.bottom() + SPOT_GAP].into_iter().map(|y| ((r.right() - SIZE - 16.0).clamp(SPOT_MARGIN, w - SIZE - SPOT_MARGIN), y))
                .find(|&(x, y)| y >= top_min() && y + SIZE <= h - SPOT_MARGIN && !motion::overlaps((x, y), t, SPOT_GAP - 1.0) && !overlaps_avoided(x, y) && !covers_content(x, y))
                .map(|(x, y)| (x, y, true))
        })
}

/// What the bubble at Sidekick's tab shows: an authored nudge (content/sidekick-nudges.yaml). One at a time.
#[derive(Clone, PartialEq)]
pub enum Bubble {
    Nudge { nudge: Nudge, pick: NudgePick, text: String },
}

/// How the pane should start when something outside opens it.
#[derive(Clone, PartialEq, Debug)]
pub enum Starter {
    /// Ask Sidekick this question.
    Ask(String),
    /// Coach me through what's on screen (or help with the stage I'm on).
    Coach,
    /// Put these words in the composer and focus it.
    Draft(String),
}

/// A nudge to the companion from elsewhere (a click on the pane's mascot, a "Where was I?" chip).
#[derive(Clone, Copy)]
pub struct Ping(RwSignal<u32>);
impl Ping {
    pub fn run(&self, _: ()) { self.0.update(|n| *n = n.wrapping_add(1)) }
    fn track(&self) -> u32 { self.0.get() }
}

/// The one Sidekick both surfaces share (the sidekick on course pages, the column in a session): its mode,
/// its pose and tricks, what it is saying, and how the pane should start. Made by App, before either.
#[derive(Clone, Copy)]
pub struct Side {
    /// Ask Sidekick (true) or Coach, remembered on this device (`sidekick_pane_mode`).
    pub ask: RwSignal<bool>,
    /// The sidekick's coaching state for the lesson on screen (the session column uses Ctx::coach).
    pub coach: RwSignal<Option<CoachState>>,
    pub starter: RwSignal<Option<Starter>>,
    /// The session column's compose box goes to the screen helper: "look" or "act".
    pub screen_mode: RwSignal<Option<&'static str>>,
    pub pose: RwSignal<String>,
    /// The trick animation class playing on the mascot (sidekick.css).
    pub fx: RwSignal<String>,
    /// A short line Sidekick says for a beat (beside the tab, or in the pane's header).
    pub quip: RwSignal<String>,
    /// Sidekick has left his spot (tab or pane header) to fly to something: hide the copy there.
    pub away: RwSignal<bool>,
    pub poke: Ping,
    pub go_next: Ping,
}

impl Side {
    pub fn new() -> Self {
        Side {
            ask: RwSignal::new(get("sidekick_pane_mode").as_deref() == Some("ask")),
            coach: RwSignal::new(None), starter: RwSignal::new(None), screen_mode: RwSignal::new(None),
            pose: RwSignal::new("idle".into()), fx: RwSignal::new(String::new()), quip: RwSignal::new(String::new()),
            away: RwSignal::new(false), poke: Ping(RwSignal::new(0)), go_next: Ping(RwSignal::new(0)),
        }
    }
}

/// What a tip's main button does.
#[derive(Clone, PartialEq, Debug)]
pub enum Act {
    /// Scroll the target into view, focus it, pulse the ring.
    Focus,
    /// Open the pane on Where to?.
    Menu,
    /// Press this control (Show a hint, Next stage).
    Press(&'static str),
    /// The screen helper in this mode (None: the coach) with these words ready in the composer.
    Compose(Option<&'static str>, String),
    /// Open the pane and start it.
    Open(Starter),
    /// Settings, at this field.
    Settings(&'static str),
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum TipKind { Mark, Help, Stuck }

/// One thing Sidekick points at: a coach mark, a hover explanation, or a stuck nudge. A ring goes round the
/// target (if any) and the bubble opens beside it; with no target it opens at Sidekick's tab.
#[derive(Clone, PartialEq, Debug)]
struct Tip {
    id: String,
    kind: TipKind,
    title: Option<String>,
    text: String,
    target: Option<String>,
    act: Option<(String, Act)>,
    pulse: bool,
}

impl Tip {
    fn mark(m: &Mark) -> Tip {
        Tip { id: m.id.into(), kind: TipKind::Mark, title: Some(m.title.into()), text: m.text.into(), target: Some(m.target.into()),
            act: Some(("Show me".into(), if m.show_me == ShowMe::Menu { Act::Menu } else { Act::Focus })), pulse: false }
    }
}

/// A practice pane signal (practice.rs, `practice-signal`): a field error, an error toast, a dialog.
#[derive(serde::Deserialize, Clone, Debug)]
struct PaneSignal { kind: String, #[serde(default)] text: String }

/// Just the pane's tab count from `practice-nav`.
#[derive(serde::Deserialize, Clone, Debug, Default)]
struct PaneTabs { #[serde(default)] tabs: Vec<serde_json::Value> }


/// The page a nudge action (or a destination chip) opens.
pub fn action_page(action: &str, p: &NudgePick, cv: &CourseView) -> Option<Page> {
    Some(match action {
        "next_lesson" => Page::Lesson(p.lesson.clone().or_else(|| next_lesson(cv, None))?),
        "next_check" => p.check.clone().map(Page::Check).unwrap_or(Page::Final),
        "session_check" => Page::Check(p.check.clone()?),
        "session" => Page::Session(p.session.clone()?),
        "final" => Page::Final,
        "certificate" => Page::Certificate,
        "settings" => Page::Settings,
        _ => return None,
    })
}

/// The certificate's to-do list; None when it couldn't be read.
async fn fetch_missing(course: &str) -> Option<Vec<Todo>> {
    api::call::<CertificateView>("get_certificate", json!({ "course_id": course, "name": null })).await.map(|c| c.missing).ok()
}

// ---- Web Animations, through Reflect (no extra web-sys features) ----
fn keyframe(transform: &str, opacity: Option<f64>, offset: f64) -> JsValue {
    let o = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&o, &"transform".into(), &transform.into());
    if let Some(op) = opacity { let _ = js_sys::Reflect::set(&o, &"opacity".into(), &op.into()); }
    let _ = js_sys::Reflect::set(&o, &"offset".into(), &offset.into());
    o.into()
}

fn animate(el: &web_sys::Element, frames: Vec<JsValue>, ms: f64) {
    let arr: js_sys::Array = frames.into_iter().collect();
    let opts = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&opts, &"duration".into(), &ms.into());
    let _ = js_sys::Reflect::set(&opts, &"easing".into(), &"linear".into());
    if let Ok(f) = js_sys::Reflect::get(el, &"animate".into()).and_then(|f| f.dyn_into::<js_sys::Function>().map_err(JsValue::from)) {
        let _ = f.call2(el, &arr, &opts);
    }
}

/// Ends any flight in progress on `el` at once (it then sits where its style says).
fn stop_flight(el: &web_sys::Element) {
    let Ok(get) = js_sys::Reflect::get(el, &"getAnimations".into()).and_then(|f| f.dyn_into::<js_sys::Function>().map_err(JsValue::from)) else { return };
    let Ok(list) = get.call0(el) else { return };
    for a in js_sys::Array::from(&list).iter() {
        if let Ok(f) = js_sys::Reflect::get(&a, &"cancel".into()).and_then(|f| f.dyn_into::<js_sys::Function>().map_err(JsValue::from)) { let _ = f.call0(&a); }
    }
}

fn call_method(target: &JsValue, name: &str, arg: &JsValue) {
    if let Ok(f) = js_sys::Reflect::get(target, &name.into()).and_then(|f| f.dyn_into::<js_sys::Function>().map_err(JsValue::from)) { let _ = f.call1(target, arg); }
}

fn translate((x, y): Pt, tilt: f64) -> String { format!("translate({x:.1}px, {y:.1}px) rotate({tilt:.1}deg)") }

/// Pages in Sidekick's familiarity model, and in stuck detection's back-and-forth.
fn page_area(p: &Page) -> Area { if *p == Page::Settings { Area::Settings } else { Area::Course } }

fn page_key(p: &Page) -> String {
    match p {
        Page::Course => "home".into(), Page::Session(s) => format!("session:{s}"), Page::Lesson(l) => format!("lesson:{l}"),
        Page::Check(c) => format!("check:{c}"), Page::Exercise(e) => format!("exercise:{e}"), Page::Final => "final".into(),
        Page::Certificate => "certificate".into(), Page::Settings => "settings".into(),
    }
}

/// Where Sidekick lives when he isn't out pointing at something: the open pane's header, the session
/// column's header, the tab's peeking mascot, or the session rail's Sidekick button.
fn home_rect() -> Option<web_sys::DomRect> {
    [".sidekick-side-pane .pane-sidekick", ".col-sidekick:not(.collapsed):not(.closed) .pane-sidekick", ".sidekick-peek", ".srail .sidekick-btn"].into_iter().find_map(rect)
}

fn home_xy() -> Pt {
    let (vw, vh) = viewport();
    home_rect().map(|r| ((r.left() + r.width() / 2.0 - SIZE / 2.0).clamp(0.0, (vw - SIZE).max(0.0)), (r.top() + r.height() / 2.0 - SIZE / 2.0).clamp(0.0, (vh - SIZE).max(0.0))))
        .unwrap_or_else(dock_xy)
}

/// A dialog of the page's own is open (Start over, What's new, the tour): Sidekick keeps out of it.
fn page_dialog() -> bool {
    document().query_selector("[aria-modal=\"true\"], dialog[open]").ok().flatten().is_some_and(|e| e.closest(".sidekick-companion-root").ok().flatten().is_none())
}

fn el_rect(e: &web_sys::Element) -> motion::Rect { let r = e.get_bounding_client_rect(); (r.left(), r.top(), r.right(), r.bottom()) }

/// The native practice pane's spot in a session (nothing drawn by the app shows over it).
fn slot_rect() -> Option<motion::Rect> { rect(".col-practice:not(.collapsed):not(.closed) .practice-slot").map(|r| (r.left(), r.top(), r.right(), r.bottom())) }

fn click_el(sel: &str) {
    if let Some(h) = document().query_selector(sel).ok().flatten().and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok()) { h.click() }
}

fn focus_el(sel: &str) {
    let Ok(Some(el)) = document().query_selector(sel) else { return };
    let o = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&o, &"block".into(), &"center".into());
    let _ = js_sys::Reflect::set(&o, &"behavior".into(), &(if reduced_motion() { "auto" } else { "smooth" }).into());
    call_method(&el.clone().into(), "scrollIntoView", &o.into());
    let focusable = el.query_selector("button:not([disabled]), a[href], input:not([disabled]), select, textarea, [tabindex]").ok().flatten()
        .or_else(|| el.matches("button, a, input, select, textarea, [tabindex]").ok().filter(|m| *m).map(|_| el.clone()));
    if let Some(f) = focusable {
        let o = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&o, &"preventScroll".into(), &true.into());
        call_method(&f.into(), "focus", &o.into());
    }
}

/// Sidekick, everywhere outside the lesson text: the tab on the right edge and the sidekick pane it opens
/// (course pages), the bubble and quips at the tab, and the Sidekick who flies out of the tab or the pane's
/// header to point at a control (a coach mark, a hover explanation, a stuck nudge, the Where to? guide)
/// and flies back. Also the app's tooltips, its hover help and its stuck detectors.
#[component]
pub fn Companion(
    /// What's new or the app tour is up: no nudge, no bubble, no coach mark, no movement.
    #[prop(into)] modal_open: Signal<bool>,
) -> impl IntoView {
    let c: Ctx = ctx();
    let sk = use_context::<Side>().expect("sidekick side");
    let nudges = RwSignal::new(Vec::<Nudge>::new());
    spawn_local(async move { if let Ok(n) = api::call::<Vec<Nudge>>("get_sidekick_nudges", json!({})).await { nudges.set(n) } });
    let helps = RwSignal::new(Vec::<HelpEntry>::new());
    spawn_local(async move { if let Ok(h) = api::call::<Vec<HelpEntry>>("get_sidekick_help", json!({})).await { helps.set(h) } });
    let name = RwSignal::new(None::<String>);
    spawn_local(async move {
        if let Ok(Some(p)) = api::call::<Option<Profile>>("get_profile", json!({})).await { name.set(crate::welcome::first_name(&p.name).map(str::to_string)) }
    });
    // the pane remembers whether it was open (c.chat_open is "the sidekick is open")
    if get("sidekick_side").as_deref() == Some("open") { c.chat_open.set(true) }
    Effect::new(move |prev: Option<bool>| { let o = c.chat_open.get(); if prev.is_some() { put("sidekick_side", if o { "open" } else { "tab" }) } o });
    let bubble = RwSignal::new(None::<Bubble>);
    let pose = sk.pose;
    let fx = sk.fx;
    let quip = sk.quip;
    // Some = out of his spot, beside something; `visiting` = the flying sprite is the Sidekick on screen
    let at = RwSignal::new(None::<Pt>);
    let visiting = RwSignal::new(false);
    let at_scroll = RwSignal::new(0.0f64);
    let tick = RwSignal::new(0u32);
    let dismissed = RwSignal::new(Vec::<String>::new());
    let where_open = RwSignal::new(false);
    let sprite = NodeRef::<leptos::html::Div>::new();
    let flying = RwSignal::new(false);
    let face_left = RwSignal::new(false);
    let flight_seq = StoredValue::new(0u32);
    let taps = StoredValue::new(Taps::default());
    let last_trick = StoredValue::new(None::<Trick>);
    let picker = StoredValue::new(lines::Picker::new(now() as u32 ^ 0x5EED));
    let rng = StoredValue::new(lines::Rng::new((now() as u64 >> 3) as u32));
    let quip_seq = StoredValue::new(0u32);
    let last_quip = StoredValue::new(0.0f64);
    let last_input = StoredValue::new(now());
    let sleeping = RwSignal::new(false);
    let greeted = StoredValue::new(false);
    let tip = RwSignal::new(None::<Tip>);
    let pending_coach = RwSignal::new(None::<&'static str>);
    let ai_blocked = RwSignal::new(false);
    let side_top = RwSignal::new(0.0f64);
    // what the learner does, for familiarity and stuck detection
    let fam = StoredValue::new(Familiar::parse(get("sidekick_familiar").as_deref()));
    let limiter = StoredValue::new(stuck::Limiter::default());
    let misses = StoredValue::new(stuck::Misses::default());
    let visits = StoredValue::new(Vec::<(f64, String)>::new());
    let clicks = StoredValue::new(Vec::<(f64, f64, f64)>::new());
    let dwell = StoredValue::new(Dwell::default());
    let ptr = StoredValue::new(None::<Pt>);
    let last_dwell = StoredValue::new(0.0f64);
    let near = StoredValue::new(None::<(&'static str, f64)>);
    let tooltip = RwSignal::new(None::<(String, Pt)>);
    let tip_seq = StoredValue::new(0u32);
    let stage_at = StoredValue::new((None::<(String, usize)>, now(), false));
    let idle_fired = StoredValue::new(false);
    let modal_since = StoredValue::new(None::<f64>);
    let pane_input = StoredValue::new(0.0f64);
    let long_press = StoredValue::new(None::<(u32, Pt)>);
    let swallow_click = StoredValue::new(0.0f64);

    let mode = move || c.sidekick_mode.get();
    // On: Sidekick speaks up, explains and coaches. Quiet: in his tab, silent until clicked. Off: the tab only.
    let chatty = move || c.sidekick_mode.get_untracked() == Mode::On;
    let in_session = move || c.practice.get() != Pmode::Off;
    let in_session_u = move || c.practice.get_untracked() != Pmode::Off;
    let sprite_shown = move || visiting.get() && mode() != Mode::Off && !modal_open.get();
    let current_xy = move || at.get_untracked().unwrap_or_else(home_xy);
    let note = move |a: Area, s: Sig| { fam.update_value(|f| f.note(a, s)); put("sidekick_familiar", &fam.with_value(Familiar::to_str)) };
    let score = move |a: Area| fam.with_value(|f| f.score(a));
    let busy_ui = move || modal_open.get_untracked() || page_dialog() || c.tour_open.get_untracked().is_some();

    // ---- what Sidekick says between nudges: a short line at the tab, or in the pane's header ----
    let say_with = move |ctx: &str, n: u32, x: &str| {
        if c.sidekick_mode.get_untracked() == Mode::Off { return; }
        let text = picker.try_update_value(|p| p.pick_with(ctx, name.get_untracked().as_deref(), n, x)).unwrap_or_default();
        if text.is_empty() { return; }
        let seq = quip_seq.get_value() + 1;
        quip_seq.set_value(seq);
        last_quip.set_value(now());
        let ms = (2400.0 + text.len() as f64 * 45.0).min(5600.0);
        quip.set(text);
        set_timeout(move || { if quip_seq.get_value() == seq { quip.set(String::new()) } }, std::time::Duration::from_millis(ms as u64));
    };
    let say = move |ctx: &str, n: u32| say_with(ctx, n, "");
    let idle_after = move |p: &'static str, ms: u64| {
        set_timeout(move || { if pose.get_untracked() == p { pose.set("idle".into()) } }, std::time::Duration::from_millis(ms));
    };
    // a smaller window (or a taller wrapped bar) mustn't leave Sidekick off screen or under the bar: pull him back in
    // (a flight in progress is cut short: its keyframes were for the old window, and a trip home just ends)
    let _resize = window_event_listener(leptos::ev::resize, move |_| {
        if flying.get_untracked() {
            flight_seq.update_value(|s| *s += 1);
            if let Some(el) = sprite.get_untracked() { stop_flight(&el.into()) }
            flying.set(false);
            face_left.set(false);
            if at.get_untracked().is_none() { visiting.set(false); sk.away.set(false); pose.set("idle".into()) }
        }
        if let Some(p) = at.get_untracked() {
            let (vw, vh) = viewport();
            let q = motion::clamp_drop(p, vw, vh, top_min());
            if q != p { at.set(Some(q)) }
        }
        tick.update(|t| *t += 1)
    });
    // a scroll anywhere (the window, the course pane) places the bubbles again, once per frame
    {
        let queued = StoredValue::new(false);
        let cb = wasm_bindgen::closure::Closure::<dyn Fn()>::new(move || {
            if queued.get_value() { return; }
            queued.set_value(true);
            request_animation_frame(move || { queued.set_value(false); tick.update(|t| *t += 1) });
        });
        let _ = document().add_event_listener_with_callback_and_bool("scroll", cb.as_ref().unchecked_ref(), true);
        cb.forget(); // app-lifetime listener on the app-lifetime companion
    }

    // ---- flight: an arc from his spot to where help is needed and back (WAAPI, transform only) ----
    // Reduced motion or Quiet: a quick fade instead; a dialog: no move.
    let travel_to = move |from: Pt, to: Pt, arrive: String| {
        let seq = flight_seq.get_value() + 1;
        flight_seq.set_value(seq);
        let Some(el) = sprite.get_untracked() else { pose.set(arrive); return };
        let el: web_sys::Element = el.into();
        stop_flight(&el);
        flying.set(false);
        face_left.set(false);
        match motion::travel(c.sidekick_mode.get_untracked(), reduced_motion(), modal_open.get_untracked()) {
            Travel::Hold => {}
            _ if !untrack(sprite_shown) || motion::dist(from, to) < 2.0 => { pose.set(arrive); }
            Travel::Fade => {
                animate(&el, vec![keyframe(&translate(from, 0.0), Some(1.0), 0.0), keyframe(&translate(from, 0.0), Some(0.0), 0.5),
                    keyframe(&translate(to, 0.0), Some(0.0), 0.5), keyframe(&translate(to, 0.0), Some(1.0), 1.0)], 320.0);
                pose.set(arrive);
            }
            Travel::Fly => {
                let f = motion::plan(from, to);
                let frames = motion::frames(&f, 18).into_iter().map(|k| keyframe(&translate((k.x, k.y), k.tilt), None, k.offset)).collect();
                animate(&el, frames, f.ms);
                face_left.set(travel_view(from.0, to.0) == "view-side-left");
                flying.set(true);
                pose.set("takeoff".into());
                let ms = f.ms;
                set_timeout(move || { if flight_seq.get_value() == seq { pose.set("fly".into()) } }, std::time::Duration::from_millis(140));
                set_timeout(move || { if flight_seq.get_value() == seq { pose.set("landing".into()) } }, std::time::Duration::from_millis((ms * 0.82) as u64));
                set_timeout(move || {
                    if flight_seq.get_value() != seq { return; }
                    flying.set(false);
                    face_left.set(false);
                    pose.set(arrive);
                }, std::time::Duration::from_millis(ms as u64 + 30));
            }
        }
    };
    // out of his spot to (x, y), pointing with `arrive`
    let visit = move |(x, y, left): (f64, f64, bool), arrive: Option<String>| {
        if modal_open.get_untracked() || c.sidekick_mode.get_untracked() == Mode::Off { return; }
        let arrive = arrive.unwrap_or_else(|| (if left { "point-left" } else { "point-right" }).to_string());
        let from = current_xy();
        visiting.set(true);
        sk.away.set(true);
        at.set(Some((x, y)));
        at_scroll.set(scroll_pos());
        travel_to(from, (x, y), arrive.clone());
        set_timeout(move || { if pose.get_untracked() == arrive { pose.set("idle".into()) } }, std::time::Duration::from_millis(4200));
    };
    // back to his spot, then the flying Sidekick gives way to the one there
    let go_home = move || {
        if modal_open.get_untracked() { return; }
        let Some(from) = at.get_untracked() else { return };
        at.set(None);
        let to = home_xy();
        travel_to(from, to, "idle".into());
        let ms = if reduced_motion() || c.sidekick_mode.get_untracked() != Mode::On { 340.0 } else { motion::plan(from, to).ms + 60.0 };
        set_timeout(move || if at.get_untracked().is_none() { visiting.set(false); sk.away.set(false) }, std::time::Duration::from_millis(ms as u64));
    };
    let _follow = set_interval_with_handle(move || {
        // the open pane and the tab start under the top bar only when there is one
        let top = rect("header.top:not(.hidden)").map(|r| r.bottom().max(0.0)).unwrap_or(0.0);
        if top != side_top.get_untracked() { side_top.set(top) }
        let scrolled = (scroll_pos() - at_scroll.get_untracked()).abs();
        match tip.get_untracked() {
            // a tip follows its target a little; scrolled well away, the learner has moved on
            Some(t) => { tick.update(|t| *t += 1); if !t.pulse && scrolled > 160.0 { tip.set(None); go_home() } }
            None => if at.get_untracked().is_some() && scrolled > 40.0 { go_home() },
        }
        // a bubble or quip up: placed again as the page settles (text size, late layout, a measured size)
        if bubble.with_untracked(Option::is_some) || !quip.with_untracked(String::is_empty) { tick.update(|t| *t += 1) }
        // Sidekick out beside something stays inside the window
        if let Some(p) = at.get_untracked().filter(|_| !flying.get_untracked()) {
            let (vw, vh) = viewport();
            let q = motion::clamp_drop(p, vw, vh, top_min());
            if q != p { at.set(Some(q)) }
        }
    }, std::time::Duration::from_millis(200));
    // the turnaround views and the companion's own poses load after start-up has settled
    set_timeout(move || {
        if c.sidekick_mode.get_untracked() == Mode::Off { return; }
        let reduce = reduced_motion();
        for v in VIEWS.iter().filter(|_| !reduce).chain(EXTRA_POSES.iter()) {
            if let Ok(img) = web_sys::HtmlImageElement::new() { img.set_decoding("async"); img.set_src(&format!("assets/mascot/{v}.png")); }
        }
    }, std::time::Duration::from_millis(4000));

    let not_now = move || {
        if let Some(Bubble::Nudge { pick, .. }) = bubble.get_untracked() {
            let mut m = parse_snooze(get("sidekick_snooze").as_deref(), now());
            m.insert(format!("{}/{}", c.course.get_untracked(), pick.trigger), now() + SNOOZE_MS);
            put("sidekick_snooze", &snooze_str(&m));
        }
        bubble.set(None);
    };
    let dismiss = move || {
        if let Some(Bubble::Nudge { pick, .. }) = bubble.get_untracked() { dismissed.update(|d| d.push(pick.trigger)) }
        bubble.set(None);
    };

    // ---- tips: coach marks, hover explanations, stuck nudges ----
    let mark_seen = move |id: &str| {
        let mut seen = coach::parse_seen(get("sidekick_coach_seen").as_deref());
        if seen.insert(id.to_string()) { put("sidekick_coach_seen", &coach::seen_str(&seen)) }
    };
    let untarget = || {
        if let Ok(list) = document().query_selector_all("[data-sidekick-target]") {
            for e in (0..list.length()).filter_map(|i| list.item(i)).filter_map(|n| n.dyn_into::<web_sys::Element>().ok()) { let _ = e.remove_attribute("data-sidekick-target"); }
        }
    };
    let end_tip = move || {
        if let Some(t) = tip.get_untracked() {
            if t.kind == TipKind::Mark { mark_seen(&t.id) }
            tip.set(None);
            untarget();
            go_home();
        }
    };
    let start_tip = move |t: Tip| {
        tooltip.set(None);
        if t.kind == TipKind::Mark {
            put("sidekick_coach_last", &now().to_string());
            // a coach mark on a control explains it: its hover help won't repeat it
            if let Some(k) = t.target.as_deref().and_then(|s| s.strip_prefix("[data-help=\"")).and_then(|s| s.strip_suffix("\"]")) {
                let mut seen = coach::parse_seen(get("sidekick_help_seen").as_deref());
                if seen.insert(k.to_string()) { put("sidekick_help_seen", &coach::seen_str(&seen)) }
            }
        }
        let (id, kind) = (t.id.clone(), t.kind);
        let target = t.target.clone();
        tip.set(Some(t));
        if kind == TipKind::Mark {
            set_timeout(move || if tip.get_untracked().is_some_and(|x| x.id == id) { mark_seen(&id) }, std::time::Duration::from_millis(coach::SEEN_AFTER_MS));
        }
        at_scroll.set(scroll_pos());
        match target.as_deref().and_then(rect) {
            Some(r) => if let Some(s) = spot_beside(&r, false) {
                visit(s, Some(point_pose((s.0, s.1), (r.left(), r.top(), r.right(), r.bottom())).to_string()))
            },
            None => { pose.set("wave".into()); idle_after("wave", 1600) }
        }
    };
    let open_pane = move |starter: Option<Starter>| {
        if in_session_u() {
            c.keep.set(2);
            c.panels.update(|p| p.sidekick = crate::Ps::Open);
        } else { c.chat_open.set(true) }
        if let Some(s) = starter { sk.starter.set(Some(s)) }
    };
    let run_act = move |act: Act| {
        let t = tip.get_untracked();
        match act {
            Act::Focus => {
                let Some(t) = t else { return };
                if let Some(sel) = &t.target { focus_el(sel) }
                if t.kind == TipKind::Mark { mark_seen(&t.id) }
                tip.set(Some(Tip { pulse: true, ..t.clone() }));
                at_scroll.set(scroll_pos());
                let id = t.id.clone();
                set_timeout(move || { if tip.get_untracked().is_some_and(|x| x.id == id) { end_tip() } }, std::time::Duration::from_millis(2000));
                return;
            }
            Act::Menu => { open_pane(None); where_open.set(true) }
            Act::Press(sel) => { if in_session_u() { c.keep.set(2); c.panels.update(|p| p.sidekick = crate::Ps::Open) } click_el(sel) }
            Act::Compose(m, text) => { sk.screen_mode.set(m); open_pane(Some(Starter::Draft(text))) }
            Act::Open(s) => {
                if matches!(s, Starter::Coach) { sk.ask.set(false) }
                open_pane(Some(s))
            }
            Act::Settings(sel) => {
                if in_session_u() { c.keep.set(0); c.panels.update(|p| p.course = crate::Ps::Open) }
                c.page.set(Page::Settings);
                set_timeout(move || focus_el(sel), std::time::Duration::from_millis(500));
            }
        }
        end_tip();
    };
    // whether the AI provider mark applies: only on Settings, and only when the provider still needs a key
    Effect::new(move |_| {
        if c.page.get() == Page::Settings && !android() {
            spawn_local(async move { ai_blocked.set(api::call::<Settings>("get_settings", json!({})).await
                .is_ok_and(|s| matches!(s.provider, shared::Provider::OpenAi { .. } | shared::Provider::Anthropic { .. }) && !s.has_api_key)) });
        }
    });
    let free = move || tip.get_untracked().is_none() && bubble.get_untracked().is_none() && !busy_ui() && !flying.get_untracked();
    let coach_ready = move |m: &Mark| -> bool {
        let Some(r) = rect(m.target) else { return false };
        let (_, vh) = viewport();
        let visible = r.top() >= top_min() - SPOT_MARGIN && r.top() < vh - 140.0;
        visible && match m.id {
            "where-to" => !c.chat_open.get_untracked(),
            "streak" => c.stats.with_untracked(|s| s.as_ref().is_some_and(|s| s.streak >= 2)),
            "ai-provider" => ai_blocked.get_untracked(),
            _ => true,
        }
    };
    let _coach = set_interval_with_handle(move || {
        if !free() || !quip.get_untracked().is_empty() || in_session_u() { return; }
        let page = c.page.get_untracked();
        // asked for (an AI feature that needs a provider): shows even if seen, and skips the wait
        if let Some(id) = pending_coach.get_untracked() {
            if let Some(m) = coach::mark(id).filter(|m| coach_on(&page) == m.on && rect(m.target).is_some()) { pending_coach.set(None); start_tip(Tip::mark(m)); }
            return;
        }
        if !chatty() || typing() || video_playing() { return; }
        let seen = coach::parse_seen(get("sidekick_coach_seen").as_deref());
        let last = coach::parse_last(get("sidekick_coach_last").as_deref());
        let gap = familiar::gap_ms(score(page_area(&page)));
        if let Some(m) = coach::next(coach_on(&page), &seen, last, now(), gap, coach_ready) { start_tip(Tip::mark(m)) }
    }, std::time::Duration::from_millis(1500));

    // ---- hover help: a 5 s rest on a control, once per control per device ----
    let help_for = move |key: &str| helps.with_untracked(|h| h.iter().find(|x| x.key == key).cloned());
    let explain = move |el: web_sys::Element, key: String, prompted: bool, title: Option<String>| {
        let Some(e) = help_for(&key) else { return };
        let mut seen = coach::parse_seen(get("sidekick_help_seen").as_deref());
        if !prompted && seen.contains(&key) { return; }
        seen.insert(key.clone());
        put("sidekick_help_seen", &coach::seen_str(&seen));
        untarget();
        let _ = el.set_attribute("data-sidekick-target", "");
        last_dwell.set_value(now());
        note(Area::of_help(&key), Sig::Dwell);
        start_tip(Tip { id: format!("help:{key}"), kind: TipKind::Help, title: Some(title.unwrap_or(e.label)), text: e.text, target: Some("[data-sidekick-target]".into()), act: None, pulse: false });
    };
    let help_at = |x: f64, y: f64| -> Option<(web_sys::Element, String)> {
        let el = document().element_from_point(x as f32, y as f32)?;
        if el.closest(".sidekick-coach, .sidekick-bubble, .sidekick-tooltip").ok().flatten().is_some() { return None; }
        let h = el.closest("[data-help]").ok().flatten()?;
        let k = h.get_attribute("data-help")?;
        Some((h, k))
    };
    let _dwell = set_interval_with_handle(move || {
        let t = now();
        let Some((x, y)) = ptr.get_value() else { return };
        let calm = chatty() && free() && !typing() && !sk.away.get_untracked() && at.get_untracked().is_none();
        let hit = help_at(x, y);
        let key = hit.as_ref().map(|(_, k)| k.clone());
        match dwell.try_update_value(|d| d.sample(key.as_deref(), (x, y), t)).unwrap_or(Rest::Nothing) {
            Rest::Explain(k) if calm && t - last_dwell.get_value() >= 20_000.0 => if let Some((el, _)) = hit { explain(el, k, false, None) },
            Rest::LeftUnclicked(k) => note(Area::of_help(&k), Sig::HoverNoClick),
            _ => {}
        }
        if !calm { dwell.update_value(|d| d.reset(t)) }
        // in a session: lingering near a coaching control the learner hasn't used yet brings its coach mark
        if in_session_u() && calm && quip.get_untracked().is_empty() {
            let used = coach::parse_seen(get("sidekick_help_used").as_deref());
            let seen = coach::parse_seen(get("sidekick_coach_seen").as_deref());
            let close = coach::MARKS.iter().filter(|m| m.on == coach::On::Coach && !seen.contains(m.id))
                .filter(|m| !used.contains(m.target.trim_start_matches("[data-help=\"").trim_end_matches("\"]")))
                .find(|m| rect(m.target).is_some_and(|r| x > r.left() - 36.0 && x < r.right() + 36.0 && y > r.top() - 36.0 && y < r.bottom() + 36.0));
            match (close, near.get_value()) {
                (Some(m), Some((id, since))) if id == m.id => {
                    let last = coach::parse_last(get("sidekick_coach_last").as_deref());
                    if t - since >= 1_500.0 && familiar::may_speak(last, t, familiar::gap_ms(score(Area::Coach))) { near.set_value(None); start_tip(Tip::mark(m)) }
                }
                (Some(m), _) => near.set_value(Some((m.id, t))),
                (None, _) => near.set_value(None),
            }
        }
    }, std::time::Duration::from_millis(250));

    // ---- tooltips: a control's short label after 500 ms of hover or on keyboard focus ----
    let show_tooltip = move |el: web_sys::Element, keyboard: bool| {
        let Some(key) = el.get_attribute("data-help") else { return };
        let Some(e) = help_for(&key).filter(|e| e.tip) else { return };
        let seq = tip_seq.get_value() + 1;
        tip_seq.set_value(seq);
        set_timeout(move || {
            if tip_seq.get_value() != seq || tip.get_untracked().is_some_and(|t| t.kind == TipKind::Help) { return; }
            let hovered = el.matches(":hover").unwrap_or(false);
            let focused = document().active_element().is_some_and(|a| a == el);
            if !(hovered || (keyboard && focused)) { return; }
            // the native title would show a second tooltip: keep it aside while ours shows
            if let Some(t) = el.get_attribute("title") { let _ = el.set_attribute("data-sidekick-title", &t); let _ = el.remove_attribute("title"); }
            let _ = el.set_attribute("aria-describedby", "sidekick-tooltip");
            let (vw, vh) = viewport();
            let w = (e.label.chars().count() as f64 * 7.0 + 22.0).min(300.0);
            let spot = help::tip_spot(el_rect(&el), w, 28.0, vw, vh, &slot_rect().into_iter().collect::<Vec<_>>());
            tooltip.set(Some((e.label, spot)));
        }, std::time::Duration::from_millis(help::TIP_MS as u64));
    };
    let hide_tooltip = move || {
        tip_seq.update_value(|s| *s += 1);
        if tooltip.get_untracked().is_some() { tooltip.set(None) }
        if let Ok(list) = document().query_selector_all("[aria-describedby=\"sidekick-tooltip\"]") {
            for e in (0..list.length()).filter_map(|i| list.item(i)).filter_map(|n| n.dyn_into::<web_sys::Element>().ok()) {
                let _ = e.remove_attribute("aria-describedby");
                if let Some(t) = e.get_attribute("data-sidekick-title") { let _ = e.set_attribute("title", &t); let _ = e.remove_attribute("data-sidekick-title"); }
            }
        }
    };
    let _over = window_event_listener(leptos::ev::pointerover, move |ev| {
        if ev.pointer_type() != "mouse" { return; }
        let Some(el) = ev.target().and_then(|t| t.dyn_into::<web_sys::Element>().ok()).and_then(|e| e.closest("[data-help]").ok().flatten()) else { hide_tooltip(); return };
        if tooltip.get_untracked().is_some() { hide_tooltip() }
        show_tooltip(el, false);
    });
    let _out = window_event_listener(leptos::ev::pointerout, move |ev| {
        let into = ev.related_target().and_then(|t| t.dyn_into::<web_sys::Element>().ok()).and_then(|e| e.closest("[data-help]").ok().flatten());
        let from = ev.target().and_then(|t| t.dyn_into::<web_sys::Element>().ok()).and_then(|e| e.closest("[data-help]").ok().flatten());
        if into != from { hide_tooltip() }
        if ev.related_target().is_none() { ptr.set_value(None) }
    });
    let _focus = window_event_listener(leptos::ev::focusin, move |ev| {
        hide_tooltip();
        let Some(el) = ev.target().and_then(|t| t.dyn_into::<web_sys::Element>().ok()) else { return };
        if el.matches(":focus-visible").unwrap_or(false) { if let Ok(Some(h)) = el.closest("[data-help]") { show_tooltip(h, true) } }
    });
    let _blur = window_event_listener(leptos::ev::focusout, move |_| hide_tooltip());

    // ---- input: pointer, keys, clicks (familiarity, rage clicks, long-press help on touch) ----
    let _move = window_event_listener(leptos::ev::pointermove, move |ev| {
        last_input.set_value(now());
        idle_fired.set_value(false);
        if ev.pointer_type() == "mouse" { ptr.set_value(Some((ev.client_x() as f64, ev.client_y() as f64))) }
        if let Some((_, p)) = long_press.get_value() { if motion::dist(p, (ev.client_x() as f64, ev.client_y() as f64)) > 10.0 { long_press.set_value(None) } }
        if sleeping.get_untracked() {
            sleeping.set(false);
            if pose.get_untracked() == "sleepy" { pose.set("idle".into()) }
        }
    });
    let _down = window_event_listener(leptos::ev::pointerdown, move |ev| {
        last_input.set_value(now());
        if ev.pointer_type() != "touch" || !chatty() { return; }
        let Some(el) = ev.target().and_then(|t| t.dyn_into::<web_sys::Element>().ok()).and_then(|e| e.closest("[data-help]").ok().flatten()) else { return };
        let Some(key) = el.get_attribute("data-help") else { return };
        let p = (ev.client_x() as f64, ev.client_y() as f64);
        let seq = tip_seq.get_value() + 1;
        long_press.set_value(Some((seq, p)));
        set_timeout(move || {
            if long_press.get_value().is_some_and(|(s, _)| s == seq) && tip.get_untracked().is_none() {
                long_press.set_value(None);
                swallow_click.set_value(now());
                note(Area::of_help(&key), Sig::OpenedHelp);
                explain(el, key, true, None);
            }
        }, std::time::Duration::from_millis(help::LONG_PRESS_MS as u64));
    });
    let _up = window_event_listener(leptos::ev::pointerup, move |_| long_press.set_value(None));
    let _menu = window_event_listener(leptos::ev::contextmenu, move |ev| { if now() - swallow_click.get_value() < 900.0 { ev.prevent_default() } });
    // clicks, in the capture phase so nothing a control does first hides them
    let on_click = move |ev: web_sys::MouseEvent| {
        let t = now();
        last_input.set_value(t);
        idle_fired.set_value(false);
        if t - swallow_click.get_value() < 900.0 { ev.prevent_default(); ev.stop_propagation(); return; }
        hide_tooltip();
        dwell.update_value(|d| d.click());
        let (x, y) = (ev.client_x() as f64, ev.client_y() as f64);
        clicks.update_value(|v| { v.push((t, x, y)); if v.len() > 8 { v.remove(0); } });
        let el = ev.target().and_then(|t| t.dyn_into::<web_sys::Element>().ok());
        if el.as_ref().is_some_and(|e| e.closest(".sidekick-coach, .sidekick-bubble, .sidekick-companion").ok().flatten().is_some()) { return; }
        let help_el = el.as_ref().and_then(|e| e.closest("[data-help]").ok().flatten());
        if let Some(key) = help_el.as_ref().and_then(|h| h.get_attribute("data-help")) {
            let area = Area::of_help(&key);
            let mut used = coach::parse_seen(get("sidekick_help_used").as_deref());
            let first = used.insert(key.clone());
            if first { put("sidekick_help_used", &coach::seen_str(&used)) }
            match key.as_str() {
                "coach.hint" => note(Area::Coach, Sig::Hint),
                "side.tab" | "lesson.ask" | "top.tour" | "settings.tour" => note(area, Sig::OpenedHelp),
                _ if tip.get_untracked().is_none() && first => note(area, Sig::Used),
                _ => {}
            }
            if key == "coach.mode-ask" || key == "coach.mode-coach" { sk.quip.set(String::new()) }
            // a resubmit grades again into the same result element: read it afresh once the grading is back
            if key == "quiz.submit" {
                if let Some(r) = help_el.as_ref().and_then(|h| h.closest(".quiz").ok().flatten()).and_then(|q| q.query_selector(".result").ok().flatten()) {
                    let _ = js_sys::Reflect::set(&r, &"__sidekick".into(), &false.into());
                }
            }
        }
        // rage: four quick clicks on one spot; explain what's there (or why it's off)
        if stuck::rage(&clicks.get_value(), t) {
            clicks.set_value(vec![]);
            let target = el.as_ref().and_then(|e| e.closest("[data-help]").ok().flatten())
                .or_else(|| el.as_ref().and_then(|e| e.closest("button, a, [role=button]").ok().flatten()));
            if let Some(h) = target {
                if h.get_attribute("data-help").is_some() && chatty() && free() && limiter.with_value(|l| l.allow(Kind::Rage, t, 0.0)) {
                    limiter.update_value(|l| l.fired(Kind::Rage, t));
                    let key = h.get_attribute("data-help").unwrap_or_default();
                    note(Area::of_help(&key), Sig::Stuck);
                    let title = picker.try_update_value(|p| p.pick_with(Kind::Rage.lines(), name.get_untracked().as_deref(), 0, "")).unwrap_or_default();
                    explain(h, key, true, Some(title));
                }
            }
        }
    };
    {
        let cb = wasm_bindgen::closure::Closure::<dyn Fn(web_sys::MouseEvent)>::new(on_click);
        let _ = window().add_event_listener_with_callback_and_bool("click", cb.as_ref().unchecked_ref(), true);
        cb.forget(); // app-lifetime listener on the app-lifetime companion
    }
    let _keys = window_event_listener(leptos::ev::keydown, move |ev| {
        last_input.set_value(now());
        idle_fired.set_value(false);
        if ev.key() != "Escape" { return; }
        hide_tooltip();
        if tip.get_untracked().is_some() { end_tip() } else if bubble.get_untracked().is_some() { dismiss() }
    });
    let _wheel = window_event_listener(leptos::ev::wheel, move |_| { last_input.set_value(now()); idle_fired.set_value(false) });

    // ---- stuck: a specific, kind nudge with one best next step ----
    let fire = move |k: Kind, ctx_lines: &'static str, x: String, target: Option<String>, act: Option<(String, Act)>, area: Area| -> bool {
        let t = now();
        if !chatty() || !free() || typing() || at.get_untracked().is_some() { return false; }
        if !limiter.with_value(|l| l.allow(k, t, familiar::stuck_gap_ms(score(area)))) { return false; }
        let text = picker.try_update_value(|p| p.pick_with(ctx_lines, name.get_untracked().as_deref(), 0, &x)).unwrap_or_default();
        if text.is_empty() { return false; }
        limiter.update_value(|l| l.fired(k, t));
        note(area, Sig::Stuck);
        start_tip(Tip { id: format!("stuck:{}", ctx_lines), kind: TipKind::Stuck, title: None, text, target: target.filter(|s| rect(s).is_some()), act, pulse: false });
        true
    };
    let _stuck = set_interval_with_handle(move || {
        let t = now();
        let session = in_session_u();
        let page = c.page.get_untracked();
        let stage = c.coach.with_untracked(|s| s.as_ref().map(|s| (s.title.clone(), s.stage, s.hints_used)));
        let exercise = matches!(page, Page::Exercise(_));
        // the pane has focus (the learner is in practice): we can't see their keys, so wait twice as long
        let away = session && !document().has_focus().unwrap_or(true);
        let last = last_input.get_value().max(pane_input.get_value());
        let area = if session { Area::Coach } else { Area::Course };
        let idle = familiar::idle_ms(score(area)) * if away { 2.0 } else { 1.0 };
        if (session && stage.is_some()) || exercise {
            if t - last >= idle && !idle_fired.get_value() {
                let hints = stage.as_ref().map(|s| s.2).unwrap_or(0);
                let fired = if session {
                    let act = if hints >= 2 && !android() { ("Do it for me".to_string(), Act::Compose(Some("act"), String::new())) } else { ("Show a hint".to_string(), Act::Press("[data-help=\"coach.hint\"]")) };
                    fire(Kind::Idle, "stuck_idle", String::new(), Some("[data-help=\"coach.hint\"]".into()), Some(act), area)
                } else {
                    fire(Kind::Idle, "stuck_idle_ex", String::new(), Some("section.practice".into()), Some(("Start a coaching session".into(), Act::Press(".practice .btn.primary"))), area)
                };
                if fired { idle_fired.set_value(true) }
            }
        }
        // one stage open a long time
        if let Some((title, n, hints)) = stage.filter(|_| session) {
            let (cur, since, done) = stage_at.get_value();
            if cur.as_ref() != Some(&(title.clone(), n)) { stage_at.set_value((Some((title, n)), t, false)) }
            else if !done && t - since >= familiar::stage_ms(score(Area::Coach)) {
                note(Area::Coach, Sig::LongStage);
                let (lines, act) = if hints >= 2 && !android() { ("stuck_stage_act", ("Do it for me".to_string(), Act::Compose(Some("act"), String::new()))) }
                    else { ("stuck_stage", ("Show a hint".to_string(), Act::Press("[data-help=\"coach.hint\"]"))) };
                if fire(Kind::LongStage, lines, String::new(), Some("[data-help=\"coach.next-stage\"]".into()), Some(act), Area::Coach) { stage_at.set_value((cur, since, true)) }
            }
        }
        // a practice dialog left open
        if session && modal_since.get_value().is_some_and(|s| t - s >= 90_000.0)
            && fire(Kind::Modal, "stuck_modal", String::new(), None, Some(("Look at it with me".into(), Act::Compose(Some("look"), "What should I do in this dialog?".into()))), Area::Practice) {
            modal_since.set_value(None)
        }
        // quiz results on the page: quick-check reactions, and the same question missed twice
        if let Ok(list) = document().query_selector_all(".quiz .result") {
            for r in (0..list.length()).filter_map(|i| list.item(i)).filter_map(|n| n.dyn_into::<web_sys::Element>().ok()) {
                if js_sys::Reflect::get(&r, &"__sidekick".into()).ok().is_some_and(|v| v.is_truthy()) { continue; }
                let Ok(Some(quiz)) = r.closest(".quiz") else { continue };
                // still grading (Submit is busy): wait for the new results
                if quiz.query_selector("[data-help=\"quiz.submit\"]:disabled").ok().flatten().is_some() { continue; }
                let _ = js_sys::Reflect::set(&r, &"__sidekick".into(), &true.into());
                let missed: Vec<usize> = quiz.query_selector_all(".question").map(|qs| (0..qs.length()).filter_map(|i| qs.item(i).map(|q| (i as usize, q)))
                    .filter_map(|(i, q)| q.dyn_into::<web_sys::Element>().ok().map(|q| (i, q)))
                    .filter(|(_, q)| q.query_selector(".feedback:not(.ok)").ok().flatten().is_some()).map(|(i, _)| i).collect()).unwrap_or_default();
                let lesson = matches!(page, Page::Lesson(_));
                if missed.is_empty() { note(Area::Course, Sig::Progress) } else { for _ in &missed { note(Area::Course, Sig::QuizMiss) } }
                let again = misses.try_update_value(|m| m.graded(&page_key(&page), &missed)).flatten();
                let nudged = again.is_some_and(|q| {
                    let n = (q + 1).to_string();
                    if lesson {
                        fire(Kind::RepeatMiss, "stuck_miss", n, Some(".keypoints".into()), Some(("Show me".into(), Act::Focus)), Area::Course)
                    } else {
                        fire(Kind::RepeatMiss, "stuck_miss_check", n.clone(), None, Some(({concat!("Ask ", shared::companion_name!())}.into(), Act::Open(Starter::Ask(format!("Can you explain the idea behind question {n} without giving the answer away?"))))), Area::Course)
                    }
                });
                if !nudged && lesson && chatty() && free() { say(if missed.is_empty() { "quick_pass" } else { "quick_miss" }, 0) }
            }
        }
    }, std::time::Duration::from_millis(1000));
    // back and forth between two pages
    Effect::new(move |prev: Option<Page>| {
        let p = c.page.get();
        if prev.as_ref().is_some_and(|q| *q != p) {
            let t = now();
            visits.update_value(|v| { v.push((t, page_key(&p))); if v.len() > 10 { v.remove(0); } });
            if stuck::ping_pong(&visits.get_value(), t).is_some() {
                let act = if in_session_u() { ({concat!("Ask ", shared::companion_name!())}.to_string(), Act::Open(Starter::Draft(String::new()))) } else { ("Where to?".to_string(), Act::Menu) };
                set_timeout(move || { if fire(Kind::PingPong, "stuck_pingpong", String::new(), None, Some(act.clone()), Area::Course) { visits.set_value(vec![]) } }, std::time::Duration::from_millis(900));
            }
        }
        p
    });
    // the practice pane: field errors, error toasts, dialogs (practice.rs `practice-signal`), and new tabs
    spawn_local(async move {
        api::listen::<PaneSignal>("practice-signal", move |s| {
            pane_input.set_value(now());
            match s.kind.as_str() {
                "validation" => {
                    let field = stuck::field_of(&s.text).unwrap_or_default();
                    let ask = if field.is_empty() { "Which field is the app asking for?".to_string() } else { format!("Where is the {field} field?") };
                    fire(Kind::Validation, "stuck_validation", field, None, Some(("Show me where".into(), Act::Compose(Some("look"), ask))), Area::Practice);
                }
                "toast" => { fire(Kind::Toast, "stuck_toast", String::new(), None, Some(("Look at it".into(), Act::Compose(Some("look"), format!("What does this error mean: \"{}\"?", s.text.trim())))), Area::Practice); }
                "modal" => modal_since.set_value((!s.text.trim().is_empty()).then(now)),
                "activity" => {}
                _ => {}
            }
        }).await;
    });
    let pane_tabs = StoredValue::new(0usize);
    spawn_local(async move {
        api::listen::<PaneTabs>("practice-nav", move |n| {
            pane_input.set_value(now());
            let before = pane_tabs.get_value();
            pane_tabs.set_value(n.tabs.len());
            if n.tabs.len() > before.max(1) && chatty() { say("pane_tab", 0) }
        }).await;
    });
    // the screen helper couldn't do it: a hint, or (a setup problem) the fix in Settings
    Effect::new(move |prev: Option<usize>| {
        let n = c.screen.with(Vec::len);
        if prev.is_some_and(|p| n > p) {
            if let Some(l) = c.screen.with_untracked(|v| v.last().cloned()) {
                if stuck::helper_failed(&l.kind, &l.text) {
                    if stuck::needs_settings(&l.text) {
                        fire(Kind::Helper, "stuck_settings", String::new(), None, Some(("Open Settings".into(), Act::Settings("[data-help=\"settings.model\"]"))), Area::Practice);
                    } else {
                        fire(Kind::Helper, "stuck_helper", String::new(), Some("[data-help=\"coach.hint\"]".into()), Some(("Show a hint".into(), Act::Press("[data-help=\"coach.hint\"]"))), Area::Coach);
                    }
                }
            }
        }
        n
    });
    // stage changes: a cheer when the learner moves on
    Effect::new(move |prev: Option<Option<usize>>| {
        let cur = c.coach.with(|s| s.as_ref().map(|s| s.stage)).or_else(|| sk.coach.with(|s| s.as_ref().map(|s| s.stage)));
        if let (Some(Some(p)), Some(n)) = (prev, cur) { if n > p { note(Area::Coach, Sig::Progress); say("stage_done", 0) } }
        cur
    });
    Effect::new(move |prev: Option<bool>| {
        let a = sk.ask.get();
        if prev.is_some_and(|p| p != a) && chatty() { say(if a { "mode_ask" } else { "mode_coach" }, 0) }
        a
    });

    // ---- nudges: one per event at most, chosen by shared::pick_nudge, said at the tab ----
    let on_event = move |ev: SidekickEvent| {
        if c.sidekick_mode.get_untracked() != Mode::On || in_session_u() { return; }
        let course = c.course.get_untracked();
        if course.is_empty() { return; }
        if let SidekickEvent::LessonDone { .. } = ev { note(Area::Course, Sig::Progress) }
        if let SidekickEvent::QuizGraded { passed, .. } = ev { note(Area::Course, if passed { Sig::Progress } else { Sig::QuizMiss }) }
        spawn_local(async move {
            let Ok(cv) = api::call::<CourseView>("get_course", json!({ "course_id": course })).await else { return };
            let Ok(st) = api::call::<Stats>("get_stats", json!({ "course_id": course })).await else { return };
            let just = match &ev { SidekickEvent::LessonDone { lesson } => Some(lesson.as_str()), _ => None };
            let all_done = cv.sessions.iter().flat_map(|s| &s.lessons).all(|l| l.completed || Some(l.id.as_str()) == just);
            let needs_cert = all_done || matches!(&ev, SidekickEvent::QuizGraded { quiz, passed: true, .. } if quiz == "final");
            // an unread to-do list would read as "nothing missing": no "certificate earned" on a failed call
            let missing = if needs_cert { match fetch_missing(&course).await { Some(m) => m, None => return } } else { vec![] };
            // a bubble or a tip already open wins; nudges never queue
            let blocked = bubble.get_untracked().is_some() || tip.get_untracked().is_some() || modal_open.get_untracked() || video_playing() || typing() || in_session_u() || c.course.get_untracked() != course;
            let mut quiet = snoozed_for(&parse_snooze(get("sidekick_snooze").as_deref(), now()), &course);
            quiet.extend(dismissed.get_untracked());
            let shown = parse_shown(get("sidekick_shown").as_deref(), now());
            let x = NudgeCtx {
                event: &ev, stats: &st, outline: &cv, missing: &missing, quiet: &quiet, shown_last_hour: shown.len(),
                blocked, studied_today_shown: get("sidekick_today").as_deref() == Some(today().as_str()),
            };
            let Some(pick) = pick_nudge(&x) else {
                // no nudge: a short line and a gesture that fit the moment instead
                if blocked { return; }
                let (ctx, gesture): (&str, &str) = match &ev {
                    SidekickEvent::CourseLoaded if !greeted.get_value() => {
                        greeted.set_value(true);
                        let streak_day = get("sidekick_streak_day");
                        if st.streak >= 3 && st.active_today && streak_day.as_deref() != Some(today().as_str()) {
                            put("sidekick_streak_day", &today());
                            ("streak", "hop")
                        } else { (lines::greeting(js_sys::Date::new_0().get_hours(), st.days_since_active), "wave") }
                    }
                    SidekickEvent::LessonDone { lesson } => {
                        let session_done = cv.sessions.iter().find(|s| s.lessons.iter().any(|l| &l.id == lesson))
                            .is_some_and(|s| s.lessons.iter().all(|l| l.completed || &l.id == lesson));
                        (if session_done { "session_done" } else { "lesson_done" }, "thumbsup")
                    }
                    SidekickEvent::QuizGraded { quiz, passed: true, .. } if quiz == "final" => ("final_pass", "celebrate"),
                    SidekickEvent::QuizGraded { passed: true, .. } => ("quiz_pass", "hop"),
                    SidekickEvent::QuizGraded { passed: false, .. } => ("quiz_miss", "encourage"),
                    SidekickEvent::PageChanged { page } => {
                        // a word on a new kind of page, more often while the learner is new here
                        let ctx = match page {
                            GuidePage::Lesson => "page_lesson", GuidePage::Check | GuidePage::Final => "page_check", GuidePage::Settings => "page_settings",
                            GuidePage::Exercise => "page_exercise", GuidePage::Session => "page_session", GuidePage::Certificate => "page_certificate",
                            GuidePage::Home => return,
                        };
                        let s = score(page_area(&c.page.get_untracked()));
                        let chance = ((1.0 - s) * 100.0).max(20.0) as usize;
                        if now() - last_quip.get_value() < 25_000.0 || rng.try_update_value(|r| r.below(100)).unwrap_or(99) >= chance { return; }
                        (ctx, "wave")
                    }
                    _ => return,
                };
                let arrive = gesture.to_string();
                pose.set(arrive.clone());
                set_timeout(move || { if pose.get_untracked() == arrive { pose.set("idle".into()) } }, std::time::Duration::from_millis(3200));
                say(ctx, st.streak);
                return;
            };
            let Some(nudge) = nudges.get_untracked().into_iter().find(|n| n.trigger == pick.trigger) else { return };
            let mut shown = shown;
            shown.push(now());
            put("sidekick_shown", &shown_str(&shown));
            if pick.trigger == "studied_today" { put("sidekick_today", &today()) }
            let text = fill_nudge(&nudge.text, &pick, &cv);
            c.outline.set(Some(cv));
            quip.set(String::new());
            pose.set(nudge.pose.clone());
            idle_after("wave", 3200);
            bubble.set(Some(Bubble::Nudge { nudge, pick, text }));
        });
    };
    Effect::new(move |_| { if let Some(ev) = c.sidekick_event.get() { on_event(ev) } });
    // course loaded (start-up or a course switch)
    Effect::new(move |prev: Option<String>| {
        let id = c.outline.with(|o| o.as_ref().map(|o| o.id.clone())).unwrap_or_default();
        if !id.is_empty() && prev.as_deref() != Some(id.as_str()) {
            set_timeout(move || c.sidekick_event.set(Some(SidekickEvent::CourseLoaded)), std::time::Duration::from_millis(900));
        }
        id
    });
    // page changes: stale nudges and tips close; Sidekick says something once the page has rendered
    Effect::new(move |prev: Option<Page>| {
        let p = c.page.get();
        if prev.is_some_and(|q| q != p) {
            if bubble.get_untracked().is_some() { bubble.set(None) }
            if tip.get_untracked().is_some() { tip.set(None); untarget(); go_home() }
            where_open.set(false);
            let page = guide_page(&p);
            set_timeout(move || {
                c.sidekick_event.set(Some(SidekickEvent::PageChanged { page }));
            }, std::time::Duration::from_millis(700));
        }
        p
    });
    // entering or leaving a session clears what was up; Quiet and Off send Sidekick home and keep him there
    Effect::new(move |prev: Option<bool>| {
        let s = in_session();
        if prev.is_some_and(|p| p != s) { bubble.set(None); tip.set(None); at.set(None); visiting.set(false); sk.away.set(false) }
        s
    });
    Effect::new(move |_| { if mode() != Mode::On { tip.set(None); quip.set(String::new()); go_home() } });
    // a dialog opening closes whatever is up and stops any flight where it would have landed
    Effect::new(move |_| {
        if modal_open.get() {
            bubble.set(None);
            tip.set(None);
            quip.set(String::new());
            hide_tooltip();
            flight_seq.update_value(|s| *s += 1);
            flying.set(false);
            if let Some(el) = sprite.get_untracked() { stop_flight(&el.into()) }
        }
    });
    // the tab reserves its 40 px on course pages, so nothing sits under it
    Effect::new(move |_| {
        let tabbed = !in_session();
        if let Some(h) = document().document_element() { let _ = h.class_list().toggle_with_force("sidekick-tabbed", tabbed); }
    });

    let go = move |p: Page| { bubble.set(None); c.page.set(p) };
    let run_action = move |action: String, pick: NudgePick| {
        let cv = c.outline.get_untracked();
        if let Some(p) = cv.as_ref().and_then(|cv| action_page(&action, &pick, cv)) { go(p) } else { bubble.set(None) }
    };
    let chip = move |which: &'static str| {
        let course = c.course.get_untracked();
        spawn_local(async move {
            let Ok(cv) = api::call::<CourseView>("get_course", json!({ "course_id": course })).await else { return };
            let page = match which {
                "lesson" => next_lesson(&cv, None).map(Page::Lesson).unwrap_or(Page::Certificate),
                "check" => match fetch_missing(&course).await.as_deref().and_then(next_check) {
                    Some(id) => Page::Check(id.to_string()),
                    None => Page::Certificate,
                },
                "final" => Page::Final,
                _ => Page::Certificate,
            };
            if which == "lesson" {
                if let Some(l) = next_lesson(&cv, None).and_then(|id| find_lesson(&cv, &id).cloned()) { say_with("tab_hello", 0, &format!("{} {}", l.number, l.title)) }
            }
            c.outline.set(Some(cv));
            go(page);
        });
    };
    // "Where was I?": the next unfinished lesson
    Effect::new(move |prev: Option<u32>| { let n = sk.go_next.track(); if prev.is_some_and(|p| p != n) { chip("lesson") } n });

    // ---- the mascot's life: tricks on a tap, a glance now and then, a nap when the learner is away ----
    let play_fx = move |class: &'static str, p: &'static str, ms: u32| {
        fx.set(String::new());
        // restart the CSS animation even when the same class plays twice
        request_animation_frame(move || fx.set(class.into()));
        pose.set(p.into());
        set_timeout(move || {
            if fx.get_untracked() == class { fx.set(String::new()) }
            if pose.get_untracked() == p { pose.set("idle".into()) }
        }, std::time::Duration::from_millis(ms as u64));
    };
    let trick = move || {
        if reduced_motion() { pose.set("wave".into()); idle_after("wave", 1200); return; }
        let t = tricks::next_trick(last_trick.get_value(), rng.try_update_value(|r| r.next()).unwrap_or(0));
        last_trick.set_value(Some(t));
        play_fx(t.class(), t.pose(), t.ms());
        say(t.lines(), 0);
    };
    Effect::new(move |prev: Option<u32>| {
        let n = sk.poke.track();
        if prev.is_some_and(|p| p != n) {
            sleeping.set(false);
            match taps.try_update_value(|t| t.tap(now())).unwrap_or(Tap::First) {
                Tap::First | Tap::Again => trick(),
                Tap::Dizzy => { play_fx("t-dizzy", "dizzy", 1900); say("dizzy", 0) }
                Tap::Secret => { play_fx("t-secret", "laugh", 1900); say("secret", 0) }
                Tap::Ignore => {}
            }
        }
        n
    });
    // a peek out of the tab now and then (On, nothing else going on, never under reduced motion)
    let _life = set_interval_with_handle(move || {
        if !chatty() || reduced_motion() || in_session_u() || c.chat_open.get_untracked() || sleeping.get_untracked() || pose.get_untracked() != "idle"
            || tip.get_untracked().is_some() || bubble.get_untracked().is_some() || sk.away.get_untracked() || now() - last_input.get_value() > 120_000.0 { return; }
        if rng.try_update_value(|r| r.below(3)) != Some(0) { return; }
        let (class, p, ms) = [("t-peek", "peek", 1200), ("t-wave", "wave", 1200), ("t-hop", "hop", 650)][rng.try_update_value(|r| r.below(3)).unwrap_or(0)];
        play_fx(class, p, ms);
    }, std::time::Duration::from_millis(14_000));
    let _nap = set_interval_with_handle(move || {
        if sleeping.get_untracked() || !chatty() || pose.get_untracked() != "idle" || now() - last_input.get_value() < 240_000.0 { return; }
        sleeping.set(true);
        pose.set("sleepy".into());
        say("long_idle", 0);
    }, std::time::Duration::from_millis(15_000));
    // the tab's hello: what Sidekick can do here, a few seconds into a page (rate-limited by familiarity)
    let _hello = set_interval_with_handle(move || {
        if !chatty() || in_session_u() || c.chat_open.get_untracked() || !free() || !quip.get_untracked().is_empty() || typing() { return; }
        let t = now();
        let last = coach::parse_last(get("sidekick_coach_last").as_deref()).max(last_quip.get_value());
        if !familiar::may_speak(last, t, familiar::gap_ms(score(page_area(&c.page.get_untracked())))) || t - last_input.get_value() > 60_000.0 { return; }
        let what = match c.page.get_untracked() {
            Page::Lesson(id) => c.outline.with_untracked(|o| o.as_ref().and_then(|cv| find_lesson(cv, &id).map(|l| l.number.clone()))).unwrap_or_default(),
            _ => String::new(),
        };
        put("sidekick_coach_last", &t.to_string());
        say_with("tab_hello", 0, &what);
    }, std::time::Duration::from_millis(9_000));

    let sprite_style = move || {
        tick.track();
        let (x, y) = at.get().unwrap_or_else(home_xy);
        format!("transform: translate({x:.0}px, {y:.0}px)")
    };
    // Debug builds only: tools/ui-check/sweep.mjs puts each kind of bubble up on any screen, to check
    // where it opens: __sidekickShow("quip" | "nudge" | "mark" | "help" | "none", text, target selector or null).
    #[cfg(debug_assertions)]
    {
        let show = wasm_bindgen::closure::Closure::<dyn Fn(String, String, Option<String>)>::new(move |kind: String, text: String, target: Option<String>| {
            bubble.set(None);
            tip.set(None);
            quip.set(String::new());
            untarget();
            match kind.as_str() {
                "none" => {}
                "quip" => quip.set(text),
                "nudge" => {
                    let nudge = Nudge { id: "check".into(), trigger: "check".into(), text: text.clone(), action: Some("final".into()), label: Some("Take me there".into()), help: None, pose: "wave".into() };
                    let pick = NudgePick { trigger: "check".into(), lesson: None, session: None, check: None };
                    bubble.set(Some(Bubble::Nudge { nudge, pick, text }))
                }
                _ => {
                    let help = kind == "help";
                    let target = target.and_then(|sel| document().query_selector(&sel).ok().flatten()).map(|el| { let _ = el.set_attribute("data-sidekick-target", ""); "[data-sidekick-target]".to_string() });
                    start_tip(Tip { id: format!("check:{kind}"), kind: if help { TipKind::Help } else { TipKind::Mark }, title: Some("Checking where this opens".into()), text, target,
                        act: (!help).then(|| ("Show me".to_string(), Act::Focus)), pulse: false });
                }
            }
        });
        let _ = js_sys::Reflect::set(&window(), &"__sidekickShow".into(), show.as_ref());
        show.forget(); // debug-only test hook on the app-lifetime companion
    }
    // ---- where the bubble, the quip and a tip open (sidekick/place.rs): wholly inside the window, under the
    // top bar, beside and never over the practice pane or Sidekick's tab, never over the text-size control,
    // the page's primary buttons, the thing explained or Sidekick, and over as few other controls as possible.
    // Measured at their real size, and placed again on resize, scroll and text-size changes (`tick`).
    let phone = move || viewport().0 <= PHONE_W;
    let home_anchor = move || -> motion::Rect {
        let (x, y) = at.get().unwrap_or_else(home_xy);
        home_rect().filter(|_| at.get().is_none()).map(|r| (r.left(), r.top(), r.right(), r.bottom())).unwrap_or((x, y, x + SIZE, y + SIZE))
    };
    let sidekick_rect = move || -> Option<motion::Rect> {
        (visiting.get_untracked()).then(|| at.get_untracked()).flatten().map(|(x, y)| (x + 8.0, y + 8.0, x + SIZE - 8.0, y + SIZE - 8.0))
    };
    let fit_at = move |anchor: motion::Rect, max_w: f64, sel: &'static str, soft_too: bool, extra: Vec<motion::Rect>| -> place::Place {
        let (vw, vh) = viewport();
        let (mut hard, soft) = keep_clear(soft_too);
        // the open sidekick lies over the page: what's under it is out of sight, so only its visible part counts
        let (anchor, extra) = match rect(".sidekick-side-pane") {
            Some(sp) => {
                let clip = |r: motion::Rect| if r.0 < sp.left() { (r.0, r.1, r.2.min(sp.left()), r.3) } else { r };
                (clip(anchor), extra.into_iter().map(clip).collect())
            }
            None => (anchor, extra),
        };
        hard.extend(extra);
        hard.extend(sidekick_rect());
        let y_min = rect("header.top:not(.hidden)").map(|r| r.bottom().max(0.0) + place::GAP).unwrap_or(0.0);
        let tabbed = !in_session_u();
        let (w, h) = natural(sel).unwrap_or((max_w, if sel == ".sidekick-quip" { 48.0 } else { 140.0 }));
        if phone() {
            // clear of the session rail on the left and Sidekick's tab on the right, resting on the text-size control
            let left = rect(".srail").map(|r| r.right()).unwrap_or(0.0);
            let floor = rect(".zoom:not(.hidden)").map(|r| r.top() - place::GAP).unwrap_or(vh);
            return place::strip(h, (left, if tabbed { vw - 40.0 } else { vw }), vh, y_min, floor, &hard);
        }
        let (bl, br) = help::band(anchor, vw, slot_rect());
        let br = if tabbed { br.min(vw - 40.0) } else { br };
        place::fit(anchor, w.min(max_w), h, (bl, br), vh, y_min, &hard, &soft)
    };
    let bubble_place = Memo::new(move |_| {
        tick.track();
        if !(bubble.with(Option::is_some) && !in_session() && mode() != Mode::Off) { return place::Place::NONE }
        let a = home_anchor();
        fit_at(a, 320.0, ".sidekick-bubble", true, vec![a])
    });
    let quip_at_tab = move || !quip.get().is_empty() && !c.chat_open.get() && !in_session() && bubble.get().is_none() && tip.get().is_none() && mode() != Mode::Off && !modal_open.get();
    // a quip has no pointer events: it may lie over controls, never over the bar, the primary buttons or Sidekick
    let quip_place = Memo::new(move |_| {
        tick.track();
        if !quip_at_tab() { return place::Place::NONE }
        let a = home_anchor();
        fit_at(a, 260.0, ".sidekick-quip", false, vec![a])
    });
    // once a bubble is up it is measured: place it again at its real size on the next frame
    Effect::new(move |_| {
        bubble.track(); tip.track(); quip.track();
        request_animation_frame(move || tick.update(|t| *t += 1));
    });
    let tip_rect = move || { tick.track(); tip.get().and_then(|t| t.target).and_then(|s| rect(&s)) };
    let ring_style = move || tip_rect().map(|r| format!("transform: translate({:.0}px, {:.0}px); width: {:.0}px; height: {:.0}px",
        r.left() - 6.0, r.top() - 6.0, r.width() + 12.0, r.height() + 12.0)).unwrap_or_else(|| "display: none".into());
    let tip_place = Memo::new(move |_| {
        tick.track();
        if !tip.with(Option::is_some) { return place::Place::NONE }
        match tip_rect() {
            Some(r) => { let ring = (r.left() - 6.0, r.top() - 6.0, r.right() + 6.0, r.bottom() + 6.0); fit_at(ring, 320.0, ".sidekick-coach", true, vec![ring]) }
            None => { let a = home_anchor(); fit_at(a, 320.0, ".sidekick-coach", true, vec![a]) }
        }
    });
    let img_error = move |ev: leptos::ev::ErrorEvent| {
        let el: web_sys::HtmlImageElement = event_target(&ev);
        let fb = fallback(&pose.get_untracked());
        if el.src().ends_with(&format!("/{fb}.png")) || el.src().ends_with("/icon.png") { el.set_src("assets/icon.png") } else { el.set_src(&format!("assets/mascot/{fb}.png")) }
    };
    let where_view = move || view! {
        <div class="sidekick-where">
            <button class="sidekick-where-toggle" data-help="side.where" aria-expanded=move || where_open.get().to_string() on:click=move |_| where_open.update(|o| *o = !*o)>
                <Icon name="right"/>"Where to?"</button>
            {move || where_open.get().then(|| view! {
                <div class="sidekick-chips">
                    <button class="chip" on:click=move |_| chip("lesson")>"Next lesson"</button>
                    <button class="chip" on:click=move |_| chip("check")>"Next check"</button>
                    <button class="chip" on:click=move |_| chip("final")>"Final"</button>
                    <button class="chip" on:click=move |_| chip("certificate")>"Certificate"</button>
                </div>
            })}
        </div>
    };

    view! {
        <div class="sidekick-companion-root">
            // the collapsed sidekick: a slim tab on the right edge, Sidekick peeking out of it
            {move || (!in_session() && !c.chat_open.get()).then(|| view! {
                <div class="sidekick-tab-wrap" class:quiet=move || mode() != Mode::On>
                    <button class="sidekick-tab" data-help="side.tab" aria-label={concat!("Open ", shared::companion_name!())} aria-expanded="false" on:click=move |_| { bubble.set(None); open_pane(None) }
                        on:pointerenter=move |_| if pose.get_untracked() == "idle" && chatty() { pose.set("wave".into()); idle_after("wave", 1200) }>
                        <Icon name="left"/><span class="sidekick-tab-name">{concat!(shared::companion_name!())}</span><span class="sidekick-live" aria-hidden="true"></span>
                    </button>
                    <span class="sidekick-peek" class:away=move || sk.away.get() class:sleeping=move || sleeping.get() aria-hidden="true">
                        <span class="sidekick-halo"></span>
                        <span class=move || format!("sidekick-fx {}", fx.get())>
                            <img src=move || format!("assets/mascot/{}.png", pose.get()) alt="" width="52" height="52" on:error=img_error draggable="false"/>
                        </span>
                    </span>
                </div>
            })}
            // the open sidekick: the same Sidekick pane as a session's column, over the page
            {move || (!in_session() && c.chat_open.get()).then(|| view! {
                <div class="sidekick-side-pane" style=move || format!("top: {:.0}px", side_top.get())>
                    <pane::SidekickPane side=true extra=ViewFn::from(where_view)/>
                </div>
            })}
            <div class="sidekick-ring" class:pulse=move || tip.get().is_some_and(|t| t.pulse) style=ring_style aria-hidden="true"></div>
            // the Sidekick who flies out to point at something, then home again
            <div class="sidekick-companion" node_ref=sprite class:hidden=move || !sprite_shown() class:flying=move || flying.get() class:face-left=move || face_left.get()
                style=sprite_style aria-hidden="true">
                <span class="sidekick-float"><span class=move || format!("sidekick-fx {}", fx.get())>
                    <img src=move || format!("assets/mascot/{}.png", pose.get()) alt="" width=SIZE as u32 height=SIZE as u32 on:error=img_error draggable="false"/>
                </span></span>
                {move || (fx.get() == "t-secret").then(|| view! { <span class="sidekick-petals" aria-hidden="true"><i></i><i></i><i></i><i></i></span> })}
            </div>
            <p class="sidekick-quip" class:strip=move || quip_place.get().strip class:above=move || quip_place.get().above class:loose=move || !quip_place.get().pointed class:on=quip_at_tab style=move || quip_place.get().css() aria-live="polite">
                {move || if quip_at_tab() { quip.get() } else { String::new() }}</p>
            {move || (mode() != Mode::Off && !modal_open.get()).then(|| tip.get()).flatten().map(|t| {
                let act = t.act.clone();
                let stuck = t.kind == TipKind::Stuck;
                view! {
                    <section class="sidekick-coach" class:help=t.kind == TipKind::Help class:strip=move || tip_place.get().strip class:above=move || tip_place.get().above class:loose=move || !tip_place.get().pointed style=move || tip_place.get().css()
                        role="dialog" aria-labelledby="sidekick-coach-text" aria-describedby="sidekick-coach-text">
                        <button class="icon-btn small sidekick-x" on:click=move |_| end_tip() aria-label="Dismiss"><Icon name="close"/></button>
                        <div class="sidekick-body">
                            {t.title.map(|h| view! { <h3>{h}</h3> })}
                            <p id="sidekick-coach-text" class="sidekick-say" aria-live="polite">{crate::md::line(&t.text)}</p>
                            <div class="sidekick-actions">
                                {act.map(|(label, a)| view! { <button class="btn small primary" on:click=move |_| run_act(a.clone())>{label}</button> })}
                                <button class="btn small quiet" on:click=move |_| end_tip()>{if stuck { "Not now" } else { "Got it" }}</button>
                            </div>
                        </div>
                    </section>
                }
            })}
            {move || (!in_session() && mode() != Mode::Off).then(|| bubble.get()).flatten().map(|b| match b {
                Bubble::Nudge { nudge, pick, text } => {
                    let (action, label, help) = (nudge.action.clone(), nudge.label.clone(), nudge.help.clone());
                    view! {
                        <section class="sidekick-bubble" class:strip=move || bubble_place.get().strip class:above=move || bubble_place.get().above class:loose=move || !bubble_place.get().pointed style=move || bubble_place.get().css() role="dialog" aria-label={concat!(shared::companion_name!())} aria-live="polite">
                            <button class="icon-btn small sidekick-x" on:click=move |_| dismiss() aria-label="Close"><Icon name="close"/></button>
                            <div class="sidekick-body">
                                <p class="sidekick-say">{crate::md::line(&text)}</p>
                                <div class="sidekick-actions">
                                    {action.zip(label).map(|(a, l)| { let p = pick.clone(); view! {
                                        <button class="btn small primary" on:click=move |_| run_action(a.clone(), p.clone())>{l}</button>
                                    } })}
                                    <button class="btn small quiet" on:click=move |_| not_now()>"Not now"</button>
                                </div>
                                {help.map(|h| { let url = h.url.clone(); view! {
                                    <button class="sidekick-help" on:click=move |_| { let url = url.clone(); spawn_local(async move { let _ = api::call::<()>("open_help", json!({ "url": url })).await; }) }>
                                        {format!("Read more in {}: {}", shared::brand::HELP_SITE_NAME, h.title)}</button>
                                } })}
                            </div>
                        </section>
                    }
                }
            })}
            <div id="sidekick-tooltip" class="sidekick-tooltip" role="tooltip" class:on=move || tooltip.with(Option::is_some)
                style=move || tooltip.get().map(|(_, (x, y))| format!("transform: translate({x:.0}px, {y:.0}px)")).unwrap_or_default()>
                {move || tooltip.get().map(|(l, _)| l).unwrap_or_default()}</div>
        </div>
    }
}


// ---- destination helpers (the Where to? chips and the nudges' actions) ----
use wasm_bindgen::prelude::*;

/// Resolves a lesson the model named by id, number ("2.3") or "lesson 2.3".
pub fn find_lesson<'a>(cv: &'a CourseView, q: &str) -> Option<&'a LessonSummary> {
    let raw = q.trim();
    let n = raw.trim_start_matches(|c: char| c.is_alphabetic() || c == ' ').trim();
    if raw.is_empty() { return None; }
    cv.sessions.iter().flat_map(|s| &s.lessons).find(|l| l.id == raw).or_else(|| cv.sessions.iter().flat_map(|s| &s.lessons).find(|l| !n.is_empty() && l.number == n))
}
