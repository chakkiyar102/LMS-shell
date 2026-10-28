//! How well the learner knows each part of the app, per device (`sidekick_familiar`), from what they do:
//! hovering without clicking, resting on a control, asking for help or a hint, missing quiz questions and
//! sitting on one stage lower it; using controls on their own, finishing lessons and passing checks raise
//! it. Sidekick speaks up unprompted often while it is low (a mark every 45 s at most) and tapers off as it
//! grows: 3 minutes by the middle, then mostly silent. Pure, tested natively; sidekick.rs feeds it.
use std::collections::BTreeMap;

/// The parts of the app Sidekick tracks separately.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Area { Course, Settings, Coach, Practice }

impl Area {
    pub fn key(self) -> &'static str {
        match self { Area::Course => "course", Area::Settings => "settings", Area::Coach => "coach", Area::Practice => "practice" }
    }
    /// The area a `data-help` key belongs to, by its prefix.
    pub fn of_help(key: &str) -> Area {
        match key.split('.').next().unwrap_or_default() {
            "settings" => Area::Settings,
            "coach" => Area::Coach,
            "pane" | "rail" => Area::Practice,
            _ => Area::Course,
        }
    }
}

/// Something the learner did that says how at home they are.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Signal {
    /// Hovered a control for a while, then left without clicking it.
    HoverNoClick,
    /// Rested on a control long enough for Sidekick to explain it.
    Dwell,
    /// Opened help: Ask Sidekick, the course tour, a long-press explanation.
    OpenedHelp,
    /// Asked the coach for a hint.
    Hint,
    /// Missed a quiz or check question.
    QuizMiss,
    /// Sat on one coaching stage a long time.
    LongStage,
    /// Got stuck enough for a stuck nudge to fire.
    Stuck,
    /// Used a control without being shown it.
    Used,
    /// Finished a lesson, passed a check, moved to the next stage.
    Progress,
}

impl Signal {
    pub fn delta(self) -> f64 {
        match self {
            Signal::HoverNoClick => -0.02,
            Signal::Dwell => -0.04,
            Signal::OpenedHelp => -0.03,
            Signal::Hint => -0.05,
            Signal::QuizMiss => -0.04,
            Signal::LongStage => -0.05,
            Signal::Stuck => -0.06,
            Signal::Used => 0.02,
            Signal::Progress => 0.05,
        }
    }
}

/// Scores 0 (new here) to 1 (knows it well), per area.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Familiar(BTreeMap<String, f64>);

impl Familiar {
    /// From `sidekick_familiar`; bad JSON or values start over at 0.
    pub fn parse(v: Option<&str>) -> Self {
        let m: BTreeMap<String, f64> = v.and_then(|s| serde_json::from_str(s).ok()).unwrap_or_default();
        Familiar(m.into_iter().filter(|(_, x)| x.is_finite()).map(|(k, x)| (k, x.clamp(0.0, 1.0))).collect())
    }
    pub fn to_str(&self) -> String { serde_json::to_string(&self.0).unwrap_or_else(|_| "{}".into()) }
    pub fn score(&self, a: Area) -> f64 { self.0.get(a.key()).copied().unwrap_or(0.0) }
    pub fn note(&mut self, a: Area, s: Signal) {
        let v = (self.score(a) + s.delta()).clamp(0.0, 1.0);
        self.0.insert(a.key().to_string(), v);
    }
}

/// The least time between unprompted coach marks and nudges at familiarity `score`: 45 s for someone new,
/// rising to the old 3 minutes at 0.6, then fast to 30 minutes ("mostly silent") at 0.85 and above.
pub fn gap_ms(score: f64) -> f64 {
    let s = score.clamp(0.0, 1.0);
    if s <= 0.6 { 45_000.0 + (180_000.0 - 45_000.0) * s / 0.6 }
    else if s < 0.85 { 180_000.0 + (1_800_000.0 - 180_000.0) * (s - 0.6) / 0.25 }
    else { 1_800_000.0 }
}

/// Stuck nudges answer a real sign of trouble, so they wait less: 45 s new, at most 5 minutes.
pub fn stuck_gap_ms(score: f64) -> f64 { gap_ms(score).min(300_000.0) }

/// No input this long while a coaching stage or an exercise is open counts as stuck: 60 s for someone
/// new, 90 s for someone at home.
pub fn idle_ms(score: f64) -> f64 { 60_000.0 + 30_000.0 * score.clamp(0.0, 1.0) }

/// One stage open this long without Next stage: 6 minutes new, 10 at home.
pub fn stage_ms(score: f64) -> f64 { 360_000.0 + 240_000.0 * score.clamp(0.0, 1.0) }

/// Whether an unprompted bubble may show: the gap since the last one has passed (a clock that went
/// backwards never locks Sidekick out).
pub fn may_speak(last_ms: f64, now_ms: f64, gap: f64) -> bool { now_ms - last_ms >= gap || now_ms < last_ms }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gaps_start_short_and_taper_to_silence() {
        assert_eq!(gap_ms(0.0), 45_000.0);
        assert_eq!(gap_ms(0.6), 180_000.0);
        assert!(gap_ms(0.3) > 45_000.0 && gap_ms(0.3) < 180_000.0);
        assert!(gap_ms(0.7) > 180_000.0);
        assert_eq!(gap_ms(0.9), 1_800_000.0, "mostly silent once at home");
        let mut prev = 0.0;
        for i in 0..=100 { let g = gap_ms(i as f64 / 100.0); assert!(g >= prev, "monotonic"); prev = g; }
        assert!(stuck_gap_ms(1.0) <= 300_000.0 && stuck_gap_ms(0.0) == 45_000.0);
        assert!((60_000.0..=90_000.0).contains(&idle_ms(0.5)) && idle_ms(0.0) == 60_000.0 && idle_ms(1.0) == 90_000.0);
        assert!(stage_ms(0.0) < stage_ms(1.0));
    }

    #[test]
    fn signals_move_each_area_on_its_own_and_round_trip() {
        let mut f = Familiar::default();
        for _ in 0..20 { f.note(Area::Coach, Signal::Used) }
        assert!((f.score(Area::Coach) - 0.4).abs() < 1e-9);
        assert_eq!(f.score(Area::Settings), 0.0);
        f.note(Area::Coach, Signal::Hint);
        assert!(f.score(Area::Coach) < 0.4);
        for _ in 0..100 { f.note(Area::Course, Signal::Progress) }
        assert_eq!(f.score(Area::Course), 1.0, "capped");
        for _ in 0..100 { f.note(Area::Settings, Signal::Dwell) }
        assert_eq!(f.score(Area::Settings), 0.0, "floored");
        assert_eq!(Familiar::parse(Some(&f.to_str())), f);
        for bad in [None, Some(""), Some("[1]"), Some("{\"course\":\"x\"}")] { assert_eq!(Familiar::parse(bad).score(Area::Course), 0.0) }
        assert_eq!(Familiar::parse(Some("{\"course\":7}")).score(Area::Course), 1.0);
    }

    #[test]
    fn the_rate_limit_follows_familiarity() {
        // simulate a learner: unfamiliar, Sidekick may speak every 45 s; after a week of steady use, rarely
        let mut f = Familiar::default();
        let mut t = 0.0;
        let mut last = -1e12;
        let mut early = 0;
        while t < 600_000.0 { if may_speak(last, t, gap_ms(f.score(Area::Course))) { early += 1; last = t } t += 5_000.0 }
        assert!(early >= 10, "{early} in 10 minutes while new");
        for _ in 0..45 { f.note(Area::Course, Signal::Used) }
        for _ in 0..3 { f.note(Area::Course, Signal::Progress) }
        let (mut late, mut last, mut t) = (0, -1e12, 0.0);
        while t < 600_000.0 { if may_speak(last, t, gap_ms(f.score(Area::Course))) { late += 1; last = t } t += 5_000.0 }
        assert!(late <= 1, "{late} in 10 minutes once familiar ({})", f.score(Area::Course));
        assert!(may_speak(1000.0, 10.0, 1e9), "a clock that went backwards");
        assert_eq!(Area::of_help("settings.model"), Area::Settings);
        assert_eq!(Area::of_help("pane.back"), Area::Practice);
        assert_eq!(Area::of_help("coach.act"), Area::Coach);
        assert_eq!(Area::of_help("lesson.complete"), Area::Course);
    }
}
