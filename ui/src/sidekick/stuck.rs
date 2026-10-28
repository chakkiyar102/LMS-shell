//! Signs a learner is stuck, and the one best thing Sidekick offers for each. Detectors are pure (tested
//! natively); sidekick.rs feeds them input times, clicks, page visits, quiz results and practice app pane signals,
//! and shows what they return as a nudge, subject to `Limiter` and the familiarity gap.
use std::collections::BTreeMap;

/// What kind of stuck.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    /// No input for a while with a coaching stage or an exercise open.
    Idle,
    /// The same quiz question missed twice or more.
    RepeatMiss,
    /// One coaching stage open a long time without Next stage.
    LongStage,
    /// Several quick clicks on the same spot.
    Rage,
    /// Back and forth between two pages.
    PingPong,
    /// practice app showed a field error (Value is required).
    Validation,
    /// practice app showed an error toast.
    Toast,
    /// A practice dialog left open a long time.
    Modal,
    /// The screen helper couldn't do what was asked.
    Helper,
}

impl Kind {
    /// The lines context (sidekick/lines.rs) the nudge's words come from.
    pub fn lines(self) -> &'static str {
        match self {
            Kind::Idle => "stuck_idle", Kind::RepeatMiss => "stuck_miss", Kind::LongStage => "stuck_stage", Kind::Rage => "stuck_rage",
            Kind::PingPong => "stuck_pingpong", Kind::Validation => "stuck_validation", Kind::Toast => "stuck_toast",
            Kind::Modal => "stuck_modal", Kind::Helper => "stuck_helper",
        }
    }
    /// Least time before the same detector may fire again.
    pub fn cooldown_ms(self) -> f64 {
        match self {
            Kind::Rage | Kind::Validation | Kind::Toast | Kind::Helper => 120_000.0,
            Kind::PingPong | Kind::RepeatMiss => 180_000.0,
            Kind::Idle | Kind::LongStage | Kind::Modal => 300_000.0,
        }
    }
    /// Signs the learner asked for nothing: these wait for the familiarity gap. The rest answer an error
    /// they just saw and only respect their own cooldown.
    pub fn unprompted(self) -> bool { matches!(self, Kind::Idle | Kind::LongStage | Kind::PingPong | Kind::Modal) }
}

/// Rate limits for stuck nudges: each detector's own cooldown, and for unprompted ones the gap since
/// the last unprompted nudge.
#[derive(Clone, Debug, Default)]
pub struct Limiter { last: BTreeMap<Kind, f64>, last_any: Option<f64> }

impl Limiter {
    pub fn allow(&self, k: Kind, now: f64, gap: f64) -> bool {
        let own = self.last.get(&k).is_none_or(|&t| now - t >= k.cooldown_ms() || now < t);
        let any = !k.unprompted() || self.last_any.is_none_or(|t| now - t >= gap || now < t);
        own && any
    }
    pub fn fired(&mut self, k: Kind, now: f64) { self.last.insert(k, now); self.last_any = Some(now); }
}

/// Quick clicks on one spot: four or more within 1.5 s, each within 30 px of the last.
pub fn rage(clicks: &[(f64, f64, f64)], now: f64) -> bool {
    let recent: Vec<_> = clicks.iter().filter(|c| now - c.0 <= 1_500.0 && c.0 <= now).collect();
    recent.len() >= 4 && recent.windows(2).all(|w| super::motion::dist((w[0].1, w[0].2), (w[1].1, w[1].2)) <= 30.0)
}

/// Back and forth: the last five visits within 40 s alternate between the same two pages (A B A B A).
/// Returns the two pages.
pub fn ping_pong(visits: &[(f64, String)], now: f64) -> Option<(String, String)> {
    if visits.len() < 5 { return None; }
    let last = &visits[visits.len() - 5..];
    if now - last[0].0 > 40_000.0 { return None; }
    let (a, b) = (&last[0].1, &last[1].1);
    (a != b && last.iter().enumerate().all(|(i, v)| &v.1 == if i % 2 == 0 { a } else { b })).then(|| (a.clone(), b.clone()))
}

/// Misses per question (`<page>#<index>`); a question missed twice is a repeat miss.
#[derive(Clone, Debug, Default)]
pub struct Misses(BTreeMap<String, u32>);

impl Misses {
    /// Records one grading: the questions missed. Returns the first question now missed twice or more.
    pub fn graded(&mut self, page: &str, missed: &[usize]) -> Option<usize> {
        let mut hit = None;
        for i in missed {
            let n = self.0.entry(format!("{page}#{i}")).or_default();
            *n += 1;
            if *n >= 2 && hit.is_none() { hit = Some(*i) }
        }
        hit
    }
}

/// The field a practice validation message names, e.g. "Due date: Value is required" -> "Due date",
/// "Value is required for Due date" -> "Due date". None when it names no field.
pub fn field_of(msg: &str) -> Option<String> {
    let m = msg.trim();
    let f = if let Some((field, rest)) = m.split_once(':') {
        (rest.to_lowercase().contains("required") || rest.to_lowercase().contains("invalid")).then(|| field.trim().to_string())
    } else if let Some(i) = m.to_lowercase().find(" for ") {
        Some(m[i + 5..].trim().trim_end_matches('.').to_string())
    } else { None };
    f.filter(|f| !f.is_empty() && f.len() <= 60 && !f.eq_ignore_ascii_case("error"))
}

/// Whether a screen helper reply means it couldn't do the job.
pub fn helper_failed(kind: &str, text: &str) -> bool {
    let t = text.to_lowercase();
    kind == "error" || (kind == "done" && ["couldn't", "could not", "can't", "cannot", "unable", "wasn't able", "not able to"].iter().any(|w| t.contains(w)))
}

/// Whether a failure is about setup (key, model, provider) and the fix is in Settings.
pub fn needs_settings(text: &str) -> bool {
    let t = text.to_lowercase();
    ["api key", "model", "provider", "settings", "http 400", "http 401", "tool", "responses api", "unauthorized", "quota"].iter().any(|w| t.contains(w))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rage_clicks_are_quick_and_on_one_spot() {
        let c = |t: f64, x: f64| (t, x, 200.0);
        assert!(rage(&[c(0.0, 100.0), c(300.0, 102.0), c(600.0, 99.0), c(900.0, 101.0)], 1_000.0));
        assert!(!rage(&[c(0.0, 100.0), c(300.0, 102.0), c(600.0, 99.0)], 700.0), "three is just impatience");
        assert!(!rage(&[c(0.0, 100.0), c(600.0, 102.0), c(1_200.0, 99.0), c(1_800.0, 101.0)], 1_900.0), "spread out");
        assert!(!rage(&[c(0.0, 100.0), c(200.0, 300.0), c(400.0, 500.0), c(600.0, 700.0)], 700.0), "different places");
    }

    #[test]
    fn ping_pong_needs_five_alternating_visits_close_together() {
        let v = |t: f64, p: &str| (t, p.to_string());
        let bounce = [v(0.0, "a"), v(5e3, "b"), v(9e3, "a"), v(14e3, "b"), v(18e3, "a")];
        assert_eq!(ping_pong(&bounce, 20e3), Some(("a".into(), "b".into())));
        assert_eq!(ping_pong(&bounce, 50e3), None, "too slow");
        assert_eq!(ping_pong(&bounce[..4], 20e3), None);
        assert_eq!(ping_pong(&[v(0.0, "a"), v(1.0, "b"), v(2.0, "c"), v(3.0, "b"), v(4.0, "a")], 10.0), None);
    }

    #[test]
    fn the_same_question_missed_twice() {
        let mut m = Misses::default();
        assert_eq!(m.graded("lesson:1.2", &[0, 2]), None);
        assert_eq!(m.graded("lesson:1.2", &[1]), None);
        assert_eq!(m.graded("lesson:1.2", &[2, 1]), Some(2));
        assert_eq!(m.graded("check:s1", &[2]), None, "per page");
    }

    #[test]
    fn validation_messages_name_their_field() {
        assert_eq!(field_of("Due date: Value is required").as_deref(), Some("Due date"));
        assert_eq!(field_of("Value is required for Due date.").as_deref(), Some("Due date"));
        assert_eq!(field_of("Something went wrong"), None);
        assert_eq!(field_of("Error: invalid"), None);
        assert!(helper_failed("done", "I couldn't find the Save button.") && helper_failed("error", "boom") && !helper_failed("done", "Added the field."));
        assert!(needs_settings("HTTP 400: model does not support tools") && !needs_settings("The button was hidden"));
    }

    #[test]
    fn limits_hold_per_detector_and_across_unprompted_ones() {
        let mut l = Limiter::default();
        assert!(l.allow(Kind::Idle, 0.0, 45_000.0));
        l.fired(Kind::Idle, 0.0);
        assert!(!l.allow(Kind::Idle, 200_000.0, 45_000.0), "its own cooldown");
        assert!(l.allow(Kind::Idle, 300_000.0, 45_000.0));
        assert!(!l.allow(Kind::LongStage, 30_000.0, 45_000.0), "unprompted ones share the gap");
        assert!(l.allow(Kind::LongStage, 50_000.0, 45_000.0));
        assert!(l.allow(Kind::Validation, 1_000.0, 45_000.0), "an error on screen answers at once");
        l.fired(Kind::Validation, 1_000.0);
        assert!(!l.allow(Kind::Validation, 60_000.0, 45_000.0));
        for k in [Kind::Idle, Kind::RepeatMiss, Kind::LongStage, Kind::Rage, Kind::PingPong, Kind::Validation, Kind::Toast, Kind::Modal, Kind::Helper] {
            assert!(!crate::sidekick::lines::lines(k.lines()).is_empty(), "{k:?} has lines");
        }
    }
}
